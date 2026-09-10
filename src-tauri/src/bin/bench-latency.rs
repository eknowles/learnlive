//! How long after you say a word does it appear on screen?
//!
//!   cargo run --release --bin bench-latency -- --models DIR --wav FILE [--asr tiny]
//!     [--min-silence 700] [--max-speech 12000] [--learning ru] [--native en] [--sweep]
//!
//! Audio is fed at wall-clock speed through the same two-thread arrangement a live session uses
//! (a VAD thread and an ML thread over a bounded channel), because latency only means anything in
//! real time. Every emission is timestamped and compared against the audio timeline, so the
//! headline number — `lag` — is literally "you stopped saying these words N ms ago".

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use crossbeam_channel::bounded;
use learnlive_lib::audio::{read_wav, resample::ToPipelineRate, FRAME_SAMPLES, PIPELINE_RATE};
use learnlive_lib::engine::vad::{Segmenter, SegmenterOptions};
use learnlive_lib::engine::{Engines, GrammarAnalyzer, Loaded, Synthesizer, Transcriber, Translator};
use learnlive_lib::pipeline::{EventSink, Job, Sentencer};
use learnlive_lib::types::{Levels, Segment, SessionConfig, SourceRole};
use parking_lot::Mutex;

// ---------------------------------------------------------------------------------------------
// Timing instrumentation: wrap the real engines so we can attribute lag to a stage without
// touching production code.

#[derive(Default)]
struct Stage {
    calls: usize,
    total: Duration,
    audio_secs: f32,
}

#[derive(Default)]
struct Stages {
    asr: Mutex<Stage>,
    mt: Mutex<Stage>,
}

struct TimedAsr(Arc<dyn Transcriber>, Arc<Stages>);
impl Transcriber for TimedAsr {
    fn transcribe(&self, pcm: &[f32], hint: &str) -> Result<(String, String)> {
        let t = Instant::now();
        let r = self.0.transcribe(pcm, hint);
        let mut s = self.1.asr.lock();
        s.calls += 1;
        s.total += t.elapsed();
        s.audio_secs += pcm.len() as f32 / PIPELINE_RATE as f32;
        r
    }
}

struct TimedMt(Arc<dyn Translator>, Arc<Stages>);
impl Translator for TimedMt {
    fn translate(&self, text: &str, from: &str, to: &str) -> Result<String> {
        let t = Instant::now();
        let r = self.0.translate(text, from, to);
        let mut s = self.1.mt.lock();
        s.calls += 1;
        s.total += t.elapsed();
        r
    }
}

// ---------------------------------------------------------------------------------------------
// Sink that stamps every emission against the audio clock.

struct Emission {
    at_ms: u64,
    id: String,
    ended_ms: u64,
    started_ms: u64,
    is_final: bool,
    text: String,
    target: String,
}

struct TimingSink {
    t0: Instant,
    out: Mutex<Vec<Emission>>,
}

impl EventSink for TimingSink {
    fn segment(&self, s: &Segment) {
        self.out.lock().push(Emission {
            at_ms: self.t0.elapsed().as_millis() as u64,
            id: s.id.clone(),
            started_ms: s.started_ms,
            ended_ms: s.ended_ms,
            is_final: s.is_final,
            text: s.source_text.clone(),
            target: s.target_text.clone(),
        });
    }
    fn levels(&self, _: &Levels) {}
}

// ---------------------------------------------------------------------------------------------

struct Run {
    emissions: Vec<Emission>,
    stages: Arc<Stages>,
    audio_secs: f32,
}

fn one_run(engines: &Arc<Engines>, cfg: &SessionConfig, pcm: &[f32], vad: SegmenterOptions) -> Result<Run> {
    let loaded = engines.load(cfg)?;
    let stages = Arc::new(Stages::default());
    let loaded = Loaded {
        asr: Arc::new(TimedAsr(loaded.asr.clone(), stages.clone())),
        translator: Arc::new(TimedMt(loaded.translator.clone(), stages.clone())),
        grammar: loaded.grammar.clone().map(|g| g as Arc<dyn GrammarAnalyzer>),
        speaker: loaded.speaker.clone(),
        tts: HashMap::<String, Arc<dyn Synthesizer>>::new(),
    };

    let t0 = Instant::now();
    let sink = Arc::new(TimingSink { t0, out: Mutex::new(vec![]) });
    let running = Arc::new(AtomicBool::new(true));
    let (tx, rx) = bounded::<Job>(16);

    // ML thread — exactly what pipeline::start spawns.
    let ml = {
        let (sink2, running2) = (sink.clone(), running.clone());
        let speakers = Arc::new(Mutex::new(learnlive_lib::engine::diarize::SpeakerRegistry::new(512)));
        let mut s = Sentencer::new(loaded, cfg.clone(), speakers, sink2, None, None);
        thread::spawn(move || {
            while running2.load(Ordering::SeqCst) {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(job) => {
                        let _ = s.push(job);
                    }
                    Err(_) => {
                        let _ = s.tick();
                    }
                }
            }
            while let Ok(job) = rx.try_recv() {
                let _ = s.push(job);
            }
            let _ = s.finalise();
        })
    };

    // Feed at wall-clock speed, like a microphone would.
    let mut seg = Segmenter::with_options(&engines.model_dir, vad)?;
    let mut dropped = 0usize;
    for (i, chunk) in pcm.chunks(FRAME_SAMPLES).enumerate() {
        let due = Duration::from_millis(i as u64 * 20);
        if let Some(wait) = due.checked_sub(t0.elapsed()) {
            thread::sleep(wait);
        }
        let mut frame = chunk.to_vec();
        frame.resize(FRAME_SAMPLES, 0.0);
        if let (_, Some(utt)) = seg.push(&frame)? {
            if tx.try_send(Job { utt, role: SourceRole::Remote }).is_err() {
                dropped += 1;
            }
        }
    }
    for utt in seg.flush() {
        let _ = tx.send(Job { utt, role: SourceRole::Remote });
    }
    running.store(false, Ordering::SeqCst);
    drop(tx);
    ml.join().map_err(|_| anyhow!("ml thread panicked"))?;
    if dropped > 0 {
        eprintln!("  warning: dropped {dropped} utterance(s) — ML thread could not keep up");
    }

    let emissions = std::mem::take(&mut *sink.out.lock());
    Ok(Run { emissions, stages, audio_secs: pcm.len() as f32 / PIPELINE_RATE as f32 })
}

fn words(s: &str) -> usize {
    s.split_whitespace().count()
}

fn report(label: &str, run: &Run, verbose: bool) {
    let e = &run.emissions;
    if e.is_empty() {
        println!("  {label}: no output");
        return;
    }

    // lag = when it appeared − when those words finished being spoken.
    let mut lags: Vec<i64> = e.iter().map(|x| x.at_ms as i64 - x.ended_ms as i64).collect();
    lags.sort_unstable();
    let pct = |p: f32| lags[((lags.len() as f32 - 1.0) * p) as usize];

    let first = &e[0];
    let first_lag = first.at_ms as i64 - first.started_ms as i64;

    // How much text lands at once: words in each segment's first appearance.
    let mut seen: HashMap<&str, usize> = HashMap::new();
    let mut block_sizes = vec![];
    for x in e {
        let prev = seen.get(x.id.as_str()).copied().unwrap_or(0);
        let now = words(&x.text);
        if now > prev {
            block_sizes.push(now - prev);
        }
        seen.insert(&x.id, now);
    }
    let avg_block = block_sizes.iter().sum::<usize>() as f32 / block_sizes.len().max(1) as f32;
    let max_block = block_sizes.iter().copied().max().unwrap_or(0);

    let asr = run.stages.asr.lock();
    let mt = run.stages.mt.lock();
    let rtf = asr.total.as_secs_f32() / asr.audio_secs.max(0.001);

    println!("  {label}");
    println!(
        "     lag after speaking   median {:>6} ms   p90 {:>6} ms   worst {:>6} ms",
        pct(0.5),
        pct(0.9),
        lags[lags.len() - 1]
    );
    println!("     first words after    {first_lag:>6} ms  (from the moment speech started)");
    println!("     words per update     avg {avg_block:>5.1}      biggest block {max_block}");
    println!(
        "     ASR   {:>2} calls  {:>6} ms total  RTF {:.2}x  ({:.1}s audio processed)",
        asr.calls,
        asr.total.as_millis(),
        rtf,
        asr.audio_secs
    );
    println!("     MT    {:>2} calls  {:>6} ms total", mt.calls, mt.total.as_millis());
    println!("     emissions {} for {} sentence(s), {:.1}s of audio", e.len(), seen.len(), run.audio_secs);

    // The transcript a reader ends up with, for eyeballing whether shorter chunks cost accuracy.
    let mut finals: HashMap<&str, (&str, &str)> = HashMap::new();
    let mut order: Vec<&str> = vec![];
    for x in e {
        if !finals.contains_key(x.id.as_str()) {
            order.push(&x.id);
        }
        finals.insert(&x.id, (&x.text, &x.target));
    }
    println!("     transcript:");
    for id in &order {
        let (src, tgt) = finals[id];
        println!("       src: {src}");
        if !tgt.is_empty() {
            println!("       mt : {tgt}");
        }
    }

    if verbose {
        println!("     timeline:");
        for x in e {
            println!(
                "       t=+{:>6.2}s  {:<6} lag {:>5}ms  {:>2}w  {:?}",
                x.at_ms as f32 / 1000.0,
                if x.is_final { "final" } else { "draft" },
                x.at_ms as i64 - x.ended_ms as i64,
                words(&x.text),
                x.text.chars().take(58).collect::<String>()
            );
            if !x.target.is_empty() {
                println!("                          -> {:?}", x.target.chars().take(58).collect::<String>());
            }
        }
    }
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let a: Vec<String> = std::env::args().collect();
    let get = |k: &str, d: &str| a.iter().position(|x| x == k).map(|i| a[i + 1].clone()).unwrap_or(d.into());
    let has = |k: &str| a.iter().any(|x| x == k);

    let dir = PathBuf::from(get("--models", ".models"));
    let wav = get("--wav", "");
    if wav.is_empty() {
        return Err(anyhow!("usage: bench-latency --models DIR --wav FILE [--asr tiny] [--sweep] [-v]"));
    }

    let cfg = SessionConfig {
        asr_model: get("--asr", "tiny"),
        learning: get("--learning", "ru"),
        native: get("--native", "en"),
        source_lang: get("--lang", "auto"),
        speak_translations: false,
        diarize: !has("--no-diarize"),
        ..Default::default()
    };

    // Mono 16 kHz, however the file arrives.
    let w = read_wav(Path::new(&wav))?;
    let ch = w.channels as usize;
    let mono: Vec<f32> = w.samples.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect();
    let pcm = ToPipelineRate::new(w.sample_rate, 1)?.push(&mono)?;

    let engines = Arc::new(Engines::new(dir));
    println!("\n{}  ({:.1}s audio, asr={})\n", wav, pcm.len() as f32 / PIPELINE_RATE as f32, cfg.asr_model);

    let base = SegmenterOptions::default();
    if has("--sweep") {
        // The two knobs that dominate: how long a pause must be, and the no-pause ceiling.
        for (silence, max_speech) in [(700, 12_000), (500, 12_000), (300, 12_000), (300, 4_000), (200, 2_500)] {
            let opts = SegmenterOptions { min_silence_ms: silence, max_speech_ms: max_speech, ..base.clone() };
            let run = one_run(&engines, &cfg, &pcm, opts)?;
            report(&format!("min_silence={silence}ms max_speech={max_speech}ms"), &run, has("-v"));
            println!();
        }
    } else {
        // Default to whatever the app actually ships, so a bare run measures real behaviour.
        let opts = SegmenterOptions {
            min_silence_ms: get("--min-silence", &base.min_silence_ms.to_string()).parse()?,
            max_speech_ms: get("--max-speech", &base.max_speech_ms.to_string()).parse()?,
            preroll_ms: get("--preroll", &base.preroll_ms.to_string()).parse()?,
            ..base
        };
        let run = one_run(&engines, &cfg, &pcm, opts)?;
        report("run", &run, true);
    }
    Ok(())
}
