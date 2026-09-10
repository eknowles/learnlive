//! Session orchestration.
//!
//! `Sentencer` is the heart: it turns VAD utterances into revisable sentences and is pure
//! logic over the engine traits, so it's unit-tested with fakes. `start()` wires it to the
//! live mixer + Tauri; `run_offline()` wires it to a WAV file for tests, CI and demos.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use crossbeam_channel::{bounded, Sender};
use log::{error, info, warn};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};

use crate::audio::{MixFrame, Mixer, MixerCommand, Player};
use crate::engine::diarize::SpeakerRegistry;
use crate::engine::vad::{Segmenter, Utterance};
use crate::engine::Engines;
use crate::types::{Levels, Segment, SessionConfig, SourceRole, SpeakerRef};

pub const CONTINUE_GAP_MS: u64 = 1500;
pub const MAX_SENTENCE_MS: u64 = 20_000;
pub const IDLE_FINALISE_MS: u64 = 1800;

// ---------------------------------------------------------------------------------------------
// Event sink: Tauri in the app, a Vec in tests.

pub trait EventSink: Send + Sync {
    fn segment(&self, s: &Segment);
    fn levels(&self, _l: &Levels) {}
    fn error(&self, _msg: &str) {}
}

impl EventSink for AppHandle {
    fn segment(&self, s: &Segment) { let _ = self.emit("segment", s); }
    fn levels(&self, l: &Levels) { let _ = self.emit("levels", l); }
    fn error(&self, m: &str) { let _ = self.emit("pipeline-error", m); }
}

/// Collects everything; `final_only()` gives the transcript a reader would end up with.
#[derive(Default)]
pub struct CollectSink { pub all: Mutex<Vec<Segment>> }
impl EventSink for CollectSink {
    fn segment(&self, s: &Segment) { self.all.lock().push(s.clone()); }
}
/// Fan-out to several sinks (UI + database).
pub struct MultiSink(pub Vec<Arc<dyn EventSink>>);
impl EventSink for MultiSink {
    fn segment(&self, s: &Segment) { for k in &self.0 { k.segment(s); } }
    fn levels(&self, l: &Levels) { for k in &self.0 { k.levels(l); } }
    fn error(&self, m: &str) { for k in &self.0 { k.error(m); } }
}

/// Persists finalised sentences to the local history.
pub struct DbSink { pub db: Arc<crate::db::Db>, pub meeting_id: i64 }
impl EventSink for DbSink {
    fn segment(&self, s: &Segment) {
        if s.is_final { if let Err(e) = self.db.insert_segment(self.meeting_id, s) { error!("db: {e:#}"); } }
    }
}

impl CollectSink {
    pub fn final_only(&self) -> Vec<Segment> {
        let all = self.all.lock();
        let mut out: Vec<Segment> = vec![];
        for s in all.iter() {
            match out.iter_mut().find(|o| o.id == s.id) { Some(o) => *o = s.clone(), None => out.push(s.clone()) }
        }
        out.retain(|s| s.is_final);
        out
    }
}

// ---------------------------------------------------------------------------------------------

pub struct SessionHandle {
    running: Arc<AtomicBool>,
    mixer_cmd: Sender<MixerCommand>,
    pub speakers: Arc<Mutex<SpeakerRegistry>>,
    pub player: Arc<Player>,
    pub cfg: SessionConfig,
    pub meeting_id: i64,
}

impl SessionHandle {
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = self.mixer_cmd.send(MixerCommand::Stop);
        self.player.clear();
    }
    pub fn mixer(&self) -> &Sender<MixerCommand> { &self.mixer_cmd }
}

pub struct Job { pub utt: Utterance, pub role: SourceRole }

struct Open {
    id: String,
    speaker: SpeakerRef,
    role: SourceRole,
    pcm: Vec<f32>,
    started_ms: u64,
    ended_ms: u64,
    last_text: String,
    revision: u32,
    arrived_at: u64,
    last_touch: Instant,
}

fn now_ms() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }

pub fn looks_finished(text: &str) -> bool {
    let t = text.trim_end();
    t.ends_with(['.', '!', '?', '。', '！', '？']) && !t.ends_with("...") && !t.ends_with('…')
}

/// Optional audio output for spoken translations (absent in offline runs).
pub struct Voice { pub player: Arc<Player>, pub mixer: Sender<MixerCommand> }

/// Turns utterances into revisable sentences. Not thread-safe by design: one per ML thread.
pub struct Sentencer {
    engines: Arc<Engines>,
    cfg: SessionConfig,
    speakers: Arc<Mutex<SpeakerRegistry>>,
    sink: Arc<dyn EventSink>,
    voice: Option<Voice>,
    clip_dir: Option<PathBuf>,
    open: Option<Open>,
    /// Injectable clock so tests can simulate idle without sleeping.
    now: Box<dyn Fn() -> Instant + Send>,
}

impl Sentencer {
    pub fn new(engines: Arc<Engines>, cfg: SessionConfig, speakers: Arc<Mutex<SpeakerRegistry>>, sink: Arc<dyn EventSink>, voice: Option<Voice>, clip_dir: Option<PathBuf>) -> Self {
        Self { engines, cfg, speakers, sink, voice, clip_dir, open: None, now: Box::new(Instant::now) }
    }

    #[cfg(test)]
    fn with_clock(mut self, f: impl Fn() -> Instant + Send + 'static) -> Self { self.now = Box::new(f); self }

    pub fn push(&mut self, job: Job) -> Result<()> {
        let speaker: SpeakerRef = if job.role == SourceRole::Local {
            self.speakers.lock().local()
        } else if let Some(emb) = self.engines.speaker.lock().clone() {
            let e = emb.embed(&job.utt.pcm)?;
            self.speakers.lock().identify(&e)
        } else {
            SpeakerRef { id: 1, label: "Speaker".into(), confidence: 0.0 }
        };

        let continues = self.open.as_ref().map(|o| {
            o.speaker.id == speaker.id
                && job.utt.started_ms.saturating_sub(o.ended_ms) <= CONTINUE_GAP_MS
                && !looks_finished(&o.last_text)
                && job.utt.ended_ms.saturating_sub(o.started_ms) <= MAX_SENTENCE_MS
        }).unwrap_or(false);

        if !continues {
            self.finalise()?;
            self.open = Some(Open {
                id: uuid::Uuid::new_v4().to_string(), speaker, role: job.role, pcm: vec![],
                started_ms: job.utt.started_ms, ended_ms: job.utt.ended_ms,
                last_text: String::new(), revision: 0, arrived_at: now_ms(), last_touch: (self.now)(),
            });
        } else if let Some(o) = self.open.as_mut() {
            o.revision += 1;
        }

        let o = self.open.as_mut().expect("open");
        o.pcm.extend_from_slice(&job.utt.pcm);
        o.ended_ms = job.utt.ended_ms;
        o.last_touch = (self.now)();

        if let Some(seg) = self.build(false)? {
            self.open.as_mut().unwrap().last_text = seg.source_text.clone();
            self.sink.segment(&seg);
            if looks_finished(&seg.source_text) { self.finalise()?; }
        }
        Ok(())
    }

    /// Call periodically; finalises an open sentence nobody has added to for a while.
    pub fn tick(&mut self) -> Result<()> {
        let idle = self.open.as_ref().map(|o| (self.now)().duration_since(o.last_touch) >= Duration::from_millis(IDLE_FINALISE_MS)).unwrap_or(false);
        if idle { self.finalise()?; }
        Ok(())
    }

    pub fn finalise(&mut self) -> Result<()> {
        if self.open.is_none() { return Ok(()); }
        let seg = self.build(true)?;
        let o = self.open.take().unwrap();
        if let Some(seg) = seg {
            self.sink.segment(&seg);
            if self.cfg.speak_translations && o.role == SourceRole::Remote {
                if let Some(v) = &self.voice { speak(&self.engines, &self.cfg, &v.player, &v.mixer, &seg.target_lang, &seg.target_text); }
            }
        }
        Ok(())
    }

    fn build(&self, is_final: bool) -> Result<Option<Segment>> {
        let o = self.open.as_ref().expect("open");
        let cfg = &self.cfg;
        let asr = self.engines.asr.lock().clone().expect("asr loaded");
        let (src_lang, text) = asr.transcribe(&o.pcm, &cfg.source_lang)?;
        if text.trim().len() < 2 { return Ok(None); }

        let target_lang = if src_lang == cfg.learning { cfg.native.clone() } else { cfg.learning.clone() };
        let translator = self.engines.translator.lock().clone().expect("translator loaded");
        let target_text = translator.translate(&text, &src_lang, &target_lang)?;

        let study_text = if target_lang == cfg.learning { &target_text } else { &text };
        let tokens = self.engines.grammar.lock().clone().map(|g| g.analyze(study_text, &cfg.learning).unwrap_or_default()).unwrap_or_default();

        let clip_path = match (&self.clip_dir, is_final) {
            (Some(d), true) => write_wav(d, &o.id, &o.pcm).ok().map(|p| p.to_string_lossy().to_string()),
            _ => None,
        };

        Ok(Some(Segment {
            id: o.id.clone(), speaker: o.speaker.clone(), role: o.role,
            started_ms: o.started_ms, ended_ms: o.ended_ms,
            source_lang: src_lang, source_text: text, target_lang, target_text, tokens, clip_path,
            is_final, revision: o.revision, arrived_at: o.arrived_at,
        }))
    }
}

// ---------------------------------------------------------------------------------------------
// Live session

pub fn start(app: AppHandle, engines: Arc<Engines>, db: Arc<crate::db::Db>, meeting_id: i64, cfg: SessionConfig, clip_dir: PathBuf) -> Result<SessionHandle> {
    engines.warm(&cfg)?;
    let mixer = Mixer::start(&cfg.sources)?;
    let player = Arc::new(Player::open()?);
    let running = Arc::new(AtomicBool::new(true));
    let mut registry = SpeakerRegistry::new(512);
    if db.remember_voices() { registry.preload(&db.voiceprints()?); }
    let speakers = Arc::new(Mutex::new(registry));
    let (job_tx, job_rx) = bounded::<Job>(16);
    std::fs::create_dir_all(&clip_dir)?;
    let sink: Arc<dyn EventSink> = Arc::new(MultiSink(vec![Arc::new(app.clone()), Arc::new(DbSink { db, meeting_id })]));

    {
        let running = running.clone();
        let sink = sink.clone();
        let model_dir = engines.model_dir.clone();
        let frames = mixer.frames.clone();
        thread::Builder::new().name("learnlive-vad".into()).spawn(move || {
            let mut seg = match Segmenter::new(&model_dir) { Ok(s) => s, Err(e) => { error!("vad: {e}"); return; } };
            let mut votes = (0u32, 0u32);
            let mut last_meter = Instant::now();
            let mut speaking = false;
            while running.load(Ordering::SeqCst) {
                let Ok(MixFrame { mix, rms, dominant }) = frames.recv_timeout(Duration::from_millis(100)) else { continue };
                match dominant { Some(SourceRole::Local) => votes.0 += 1, Some(SourceRole::Remote) => votes.1 += 1, None => {} }
                match seg.push(&mix) {
                    Ok((sp, Some(utt))) => {
                        speaking = sp;
                        let role = if votes.0 > votes.1 { SourceRole::Local } else { SourceRole::Remote };
                        votes = (0, 0);
                        if job_tx.try_send(Job { utt, role }).is_err() { warn!("ML thread busy; dropped an utterance"); }
                    }
                    Ok((sp, None)) => speaking = sp,
                    Err(e) => error!("vad push: {e}"),
                }
                if last_meter.elapsed() > Duration::from_millis(66) {
                    last_meter = Instant::now();
                    let level = (mix.iter().map(|x| x * x).sum::<f32>() / mix.len() as f32).sqrt();
                    sink.levels(&Levels { per_source: rms, mix: level, speech_active: speaking });
                }
            }
        })?;
    }

    {
        let running = running.clone();
        let voice = Voice { player: player.clone(), mixer: mixer.commands.clone() };
        let mut s = Sentencer::new(engines.clone(), cfg.clone(), speakers.clone(), sink.clone(), Some(voice), Some(clip_dir));
        let sink = sink.clone();
        thread::Builder::new().name("learnlive-ml".into()).spawn(move || {
            while running.load(Ordering::SeqCst) {
                let r = match job_rx.recv_timeout(Duration::from_millis(200)) { Ok(job) => s.push(job), Err(_) => s.tick() };
                if let Err(e) = r { error!("pipeline: {e:#}"); sink.error(&e.to_string()); }
            }
            let _ = s.finalise();
        })?;
    }

    info!("session started: learning={} native={} sources={}", cfg.learning, cfg.native, cfg.sources.len());
    Ok(SessionHandle { running, mixer_cmd: mixer.commands.clone(), speakers, player, cfg, meeting_id })
}

// ---------------------------------------------------------------------------------------------
// Offline run: WAV → segments, as fast as the models allow. Mono = remote speech.
// Stereo = channel 0 is you (local), channel 1 is the call (remote) — record it that way and
// the role split gets tested too.

pub fn run_offline(engines: Arc<Engines>, cfg: &SessionConfig, wav_path: &std::path::Path) -> Result<Vec<Segment>> {
    engines.warm(cfg)?;
    let wav = crate::audio::read_wav(wav_path)?;
    let sink = Arc::new(CollectSink::default());
    let speakers = Arc::new(Mutex::new(SpeakerRegistry::new(512)));
    let mut s = Sentencer::new(engines.clone(), cfg.clone(), speakers, sink.clone(), None, None);

    // Split channels, resample each, then walk them in lockstep through the VAD like the mixer would.
    let ch = wav.channels as usize;
    let mut lanes: Vec<Vec<f32>> = (0..ch.min(2)).map(|c| {
        let mut rs = crate::audio::resample::ToPipelineRate::new(wav.sample_rate, 1).unwrap();
        let mono: Vec<f32> = wav.samples.iter().skip(c).step_by(ch).copied().collect();
        rs.push(&mono).unwrap_or_default()
    }).collect();
    if ch > 2 { warn!("using first two channels of a {ch}-channel file"); }
    let n = lanes.iter().map(|l| l.len()).max().unwrap_or(0);
    for l in lanes.iter_mut() { l.resize(n, 0.0); }

    let mut seg = Segmenter::new(&engines.model_dir)?;
    let mut votes = (0u32, 0u32);
    let frame = crate::audio::FRAME_SAMPLES;
    for i in (0..n).step_by(frame) {
        let end = (i + frame).min(n);
        let mut mix = vec![0.0f32; end - i];
        let mut rms = vec![0.0f32; lanes.len()];
        for (li, l) in lanes.iter().enumerate() {
            for (m, x) in mix.iter_mut().zip(&l[i..end]) { *m += x; rms[li] += x * x; }
        }
        if lanes.len() == 2 {
            if rms[0] > rms[1] * 1.5 && rms[0] > 1e-4 { votes.0 += 1 } else if rms[1] > 1e-4 { votes.1 += 1 }
        }
        if let (_, Some(utt)) = seg.push(&mix)? {
            let role = if lanes.len() == 2 && votes.0 > votes.1 { SourceRole::Local } else { SourceRole::Remote };
            votes = (0, 0);
            s.push(Job { utt, role })?;
        }
    }
    s.finalise()?;
    Ok(sink.final_only())
}

// ---------------------------------------------------------------------------------------------

pub fn speak(engines: &Engines, cfg: &SessionConfig, player: &Player, mixer: &Sender<MixerCommand>, lang: &str, text: &str) {
    let Some(voice) = engines.tts.lock().get(lang).cloned() else { return };
    match voice.synthesize(text) {
        Ok((pcm, rate)) => {
            let _ = mixer.send(MixerCommand::Duck(1.0 - cfg.duck_amount));
            player.play(&pcm, rate);
            thread::sleep(Duration::from_millis((pcm.len() as f32 / rate as f32 * 1000.0) as u64));
            let _ = mixer.send(MixerCommand::Duck(1.0));
        }
        Err(e) => warn!("tts: {e}"),
    }
}

fn write_wav(dir: &PathBuf, id: &str, pcm: &[f32]) -> Result<PathBuf> {
    let path = dir.join(format!("{id}.wav"));
    crate::audio::write_wav16(&path, 16_000, 1, pcm)?;
    Ok(path)
}

pub fn read_wav(path: &str) -> Result<Vec<f32>> {
    Ok(crate::audio::read_wav(std::path::Path::new(path))?.samples)
}

// =============================================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{GrammarAnalyzer, SpeakerEmbedder, Transcriber, Translator};

    /// Fake ASR: "hears" a script keyed by audio length, so merged audio yields merged text.
    struct FakeAsr;
    impl Transcriber for FakeAsr {
        fn transcribe(&self, pcm: &[f32], _: &str) -> Result<(String, String)> {
            // Each 1000 "samples" is one word of a fixed sentence; a full stop only on the 4th.
            let words = ["я", "читаю", "книгу", "дома."];
            let n = (pcm.len() / 1000).clamp(1, 4);
            Ok(("ru".into(), words[..n].join(" ")))
        }
    }
    struct FakeMt;
    impl Translator for FakeMt { fn translate(&self, t: &str, _: &str, _: &str) -> Result<String> { Ok(format!("[{t}]")) } }
    struct NoGrammar;
    impl GrammarAnalyzer for NoGrammar { fn analyze(&self, _: &str, _: &str) -> Result<Vec<crate::types::Token>> { Ok(vec![]) } }
    /// Speaker = sign of the first sample. Two "voices" for tests.
    struct FakeEmb;
    impl SpeakerEmbedder for FakeEmb {
        fn embed(&self, pcm: &[f32]) -> Result<Vec<f32>> {
            let s = if pcm.first().copied().unwrap_or(1.0) >= 0.0 { 1.0 } else { -1.0 };
            let mut v = vec![0.0; 512]; v[0] = s; v[1] = 0.1; Ok(v)
        }
    }

    fn engines() -> Arc<Engines> {
        let e = Engines::new(std::env::temp_dir());
        *e.asr.lock() = Some(Arc::new(FakeAsr));
        *e.translator.lock() = Some(Arc::new(FakeMt));
        *e.grammar.lock() = Some(Arc::new(NoGrammar));
        *e.speaker.lock() = Some(Arc::new(FakeEmb));
        Arc::new(e)
    }
    fn cfg() -> SessionConfig { SessionConfig { speak_translations: false, ..Default::default() } }
    fn utt(words: usize, start: u64, voice: f32) -> Utterance {
        let mut pcm = vec![0.01; words * 1000]; pcm[0] = voice;
        Utterance { pcm, started_ms: start, ended_ms: start + words as u64 * 500 }
    }
    fn sut() -> (Sentencer, Arc<CollectSink>) {
        let sink = Arc::new(CollectSink::default());
        let s = Sentencer::new(engines(), cfg(), Arc::new(Mutex::new(SpeakerRegistry::new(512))), sink.clone(), None, None);
        (s, sink)
    }

    #[test]
    fn continuation_merges_and_revises_in_place() {
        let (mut s, sink) = sut();
        s.push(Job { utt: utt(2, 0, 1.0), role: SourceRole::Remote }).unwrap();     // "я читаю"
        s.push(Job { utt: utt(2, 1500, 1.0), role: SourceRole::Remote }).unwrap();  // + "книгу дома."
        let all = sink.all.lock();
        assert_eq!(all.len(), 3, "draft, revised draft, final");
        assert!(all.iter().all(|x| x.id == all[0].id), "same sentence id throughout");
        assert_eq!(all[0].source_text, "я читаю"); assert!(!all[0].is_final);
        assert_eq!(all[1].source_text, "я читаю книгу дома."); assert_eq!(all[1].revision, 1);
        assert!(all[2].is_final, "terminal punctuation finalises immediately");
        assert_eq!(all[2].target_text, "[я читаю книгу дома.]");
    }

    #[test]
    fn long_gap_starts_a_new_sentence() {
        let (mut s, sink) = sut();
        s.push(Job { utt: utt(2, 0, 1.0), role: SourceRole::Remote }).unwrap();
        s.push(Job { utt: utt(2, 5000, 1.0), role: SourceRole::Remote }).unwrap(); // gap 4 s > 1.5 s
        let f = sink.final_only();
        assert_eq!(f.len(), 1, "first sentence finalised by the new one; second still open");
        assert_eq!(f[0].source_text, "я читаю");
        s.finalise().unwrap();
        assert_eq!(sink.final_only().len(), 2);
    }

    #[test]
    fn speaker_change_never_merges() {
        let (mut s, sink) = sut();
        s.push(Job { utt: utt(2, 0, 1.0), role: SourceRole::Remote }).unwrap();
        s.push(Job { utt: utt(2, 1200, -1.0), role: SourceRole::Remote }).unwrap(); // different voice
        s.finalise().unwrap();
        let f = sink.final_only();
        assert_eq!(f.len(), 2);
        assert_ne!(f[0].speaker.id, f[1].speaker.id);
        assert_eq!(f[1].speaker.label, "Speaker 2");
    }

    #[test]
    fn local_mic_is_you_and_translates_into_learning_language() {
        let (mut s, sink) = sut();
        s.push(Job { utt: utt(2, 0, 1.0), role: SourceRole::Local }).unwrap();
        s.finalise().unwrap();
        let f = sink.final_only();
        assert_eq!(f[0].speaker.label, "You");
        assert_eq!(f[0].role, SourceRole::Local);
    }

    #[test]
    fn idle_tick_finalises() {
        let (mut s, sink) = sut();
        let base = Instant::now();
        let offset = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let o2 = offset.clone();
        s = s.with_clock(move || base + Duration::from_millis(o2.load(Ordering::SeqCst)));
        s.push(Job { utt: utt(2, 0, 1.0), role: SourceRole::Remote }).unwrap();
        s.tick().unwrap();
        assert!(sink.final_only().is_empty(), "not idle yet");
        offset.store(IDLE_FINALISE_MS + 1, Ordering::SeqCst);
        s.tick().unwrap();
        assert_eq!(sink.final_only().len(), 1);
    }

    #[test]
    fn punctuation_heuristic() {
        assert!(looks_finished("Готово."));
        assert!(looks_finished("Правда?"));
        assert!(!looks_finished("и потом..."));
        assert!(!looks_finished("а ещё"));
    }
}
