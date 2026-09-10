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

/// A chunk from the same speaker arriving within this gap continues the open sentence.
const CONTINUE_GAP_MS: u64 = 1500;
/// Never let one sentence grow past this; finalise and start a new one.
const MAX_SENTENCE_MS: u64 = 20_000;
/// Idle time after which an open (unfinished-looking) sentence is finalised anyway.
const IDLE_FINALISE_MS: u64 = 1800;

pub struct SessionHandle {
    running: Arc<AtomicBool>,
    mixer_cmd: Sender<MixerCommand>,
    pub speakers: Arc<Mutex<SpeakerRegistry>>,
    pub player: Arc<Player>,
    pub cfg: SessionConfig,
}

impl SessionHandle {
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = self.mixer_cmd.send(MixerCommand::Stop);
        self.player.clear();
    }
    pub fn mixer(&self) -> &Sender<MixerCommand> { &self.mixer_cmd }
}

struct Job { utt: Utterance, role: SourceRole }

/// A sentence still being spoken. Holds the *audio* so revisions re-run ASR with full context,
/// not just string-concatenate transcripts.
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

fn looks_finished(text: &str) -> bool {
    let t = text.trim_end();
    t.ends_with(['.', '!', '?', '。', '！', '？']) && !t.ends_with("...") && !t.ends_with('…')
}

pub fn start(app: AppHandle, engines: Arc<Engines>, cfg: SessionConfig, clip_dir: PathBuf) -> Result<SessionHandle> {
    engines.warm(&cfg)?;
    let mixer = Mixer::start(&cfg.sources)?;
    let player = Arc::new(Player::open()?);
    let running = Arc::new(AtomicBool::new(true));
    let speakers = Arc::new(Mutex::new(SpeakerRegistry::new(512)));
    let (job_tx, job_rx) = bounded::<Job>(16);
    std::fs::create_dir_all(&clip_dir)?;

    // ---- audio thread: frames → utterances ------------------------------------------------
    {
        let running = running.clone();
        let app = app.clone();
        let model_dir = engines.model_dir.clone();
        let frames = mixer.frames.clone();
        thread::Builder::new().name("learnlive-vad".into()).spawn(move || {
            let mut seg = match Segmenter::new(&model_dir) { Ok(s) => s, Err(e) => { error!("vad: {e}"); return; } };
            let mut role_votes = (0u32, 0u32);
            let mut last_meter = Instant::now();
            let mut speaking = false;
            while running.load(Ordering::SeqCst) {
                let Ok(MixFrame { mix, rms, dominant }) = frames.recv_timeout(Duration::from_millis(100)) else { continue };
                match dominant { Some(SourceRole::Local) => role_votes.0 += 1, Some(SourceRole::Remote) => role_votes.1 += 1, None => {} }
                match seg.push(&mix) {
                    Ok((sp, Some(utt))) => {
                        speaking = sp;
                        let role = if role_votes.0 > role_votes.1 { SourceRole::Local } else { SourceRole::Remote };
                        role_votes = (0, 0);
                        if job_tx.try_send(Job { utt, role }).is_err() { warn!("ML thread busy; dropped an utterance"); }
                    }
                    Ok((sp, None)) => speaking = sp,
                    Err(e) => error!("vad push: {e}"),
                }
                if last_meter.elapsed() > Duration::from_millis(66) {
                    last_meter = Instant::now();
                    let level = (mix.iter().map(|x| x * x).sum::<f32>() / mix.len() as f32).sqrt();
                    let _ = app.emit("levels", Levels { per_source: rms, mix: level, speech_active: speaking });
                }
            }
        })?;
    }

    // ---- ML thread: utterances → (revisable) sentences -------------------------------------
    {
        let running = running.clone();
        let app = app.clone();
        let engines = engines.clone();
        let cfg2 = cfg.clone();
        let speakers = speakers.clone();
        let player = player.clone();
        let mixer_cmd = mixer.commands.clone();
        thread::Builder::new().name("learnlive-ml".into()).spawn(move || {
            let mut open: Option<Open> = None;
            while running.load(Ordering::SeqCst) {
                match job_rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(job) => {
                        if let Err(e) = on_utterance(&app, &engines, &cfg2, &speakers, &player, &mixer_cmd, &clip_dir, &mut open, job) {
                            error!("pipeline: {e:#}");
                            let _ = app.emit("pipeline-error", e.to_string());
                        }
                    }
                    Err(_) => {
                        // Nobody spoke for a while: close whatever is open.
                        let idle = open.as_ref().map(|o| o.last_touch.elapsed() >= Duration::from_millis(IDLE_FINALISE_MS)).unwrap_or(false);
                        if idle {
                            if let Some(o) = open.take() {
                                if let Err(e) = finalise(&app, &engines, &cfg2, &player, &mixer_cmd, &clip_dir, o) { error!("finalise: {e:#}"); }
                            }
                        }
                    }
                }
            }
            if let Some(o) = open.take() { let _ = finalise(&app, &engines, &cfg2, &player, &mixer_cmd, &clip_dir, o); }
        })?;
    }

    info!("session started: learning={} native={} sources={}", cfg.learning, cfg.native, cfg.sources.len());
    Ok(SessionHandle { running, mixer_cmd: mixer.commands.clone(), speakers, player, cfg })
}

#[allow(clippy::too_many_arguments)]
fn on_utterance(
    app: &AppHandle, engines: &Engines, cfg: &SessionConfig, speakers: &Mutex<SpeakerRegistry>,
    player: &Player, mixer: &Sender<MixerCommand>, clip_dir: &PathBuf, open: &mut Option<Open>, job: Job,
) -> Result<()> {
    // 1. Who is this? (needed before we can decide whether it continues the open sentence)
    let speaker: SpeakerRef = if job.role == SourceRole::Local {
        speakers.lock().local()
    } else if let Some(emb) = engines.speaker.lock().clone() {
        let e = emb.embed(&job.utt.pcm)?;
        speakers.lock().identify(&e)
    } else {
        SpeakerRef { id: 1, label: "Speaker".into(), confidence: 0.0 }
    };

    // 2. Continuation?  Same speaker, short gap, previous text didn't look finished, not too long.
    let continues = open.as_ref().map(|o| {
        o.speaker.id == speaker.id
            && job.utt.started_ms.saturating_sub(o.ended_ms) <= CONTINUE_GAP_MS
            && !looks_finished(&o.last_text)
            && job.utt.ended_ms - o.started_ms <= MAX_SENTENCE_MS
    }).unwrap_or(false);

    if !continues {
        if let Some(o) = open.take() { finalise(app, engines, cfg, player, mixer, clip_dir, o)?; }
        *open = Some(Open {
            id: uuid::Uuid::new_v4().to_string(),
            speaker, role: job.role, pcm: vec![],
            started_ms: job.utt.started_ms, ended_ms: job.utt.ended_ms,
            last_text: String::new(), revision: 0, arrived_at: now_ms(), last_touch: Instant::now(),
        });
    } else if let Some(o) = open.as_mut() {
        o.revision += 1;
    }

    let o = open.as_mut().expect("open sentence");
    o.pcm.extend_from_slice(&job.utt.pcm);
    o.ended_ms = job.utt.ended_ms;
    o.last_touch = Instant::now();

    // 3. Draft: re-run ASR on the *merged* audio, translate, emit as provisional.
    let Some(seg) = build(engines, cfg, o, false, clip_dir)? else { return Ok(()) };
    o.last_text = seg.source_text.clone();
    let _ = app.emit("segment", &seg);

    // Speech that already ends a sentence needn't wait for the idle timer.
    if looks_finished(&o.last_text) {
        if let Some(o) = open.take() { finalise(app, engines, cfg, player, mixer, clip_dir, o)?; }
    }
    Ok(())
}

fn finalise(app: &AppHandle, engines: &Engines, cfg: &SessionConfig, player: &Player, mixer: &Sender<MixerCommand>, clip_dir: &PathBuf, o: Open) -> Result<()> {
    let mut o = o;
    let Some(seg) = build(engines, cfg, &mut o, true, clip_dir)? else { return Ok(()) };
    let _ = app.emit("segment", &seg);
    if cfg.speak_translations && o.role == SourceRole::Remote {
        speak(engines, cfg, player, mixer, &seg.target_lang, &seg.target_text);
    }
    Ok(())
}

/// ASR + translate (+ grammar) for the open sentence's full audio.
fn build(engines: &Engines, cfg: &SessionConfig, o: &mut Open, is_final: bool, clip_dir: &PathBuf) -> Result<Option<Segment>> {
    let asr = engines.asr.lock().clone().expect("asr loaded");
    let (src_lang, text) = asr.transcribe(&o.pcm, &cfg.source_lang)?;
    if text.trim().len() < 2 { return Ok(None); }

    let target_lang = if src_lang == cfg.learning { cfg.native.clone() } else { cfg.learning.clone() };
    let translator = engines.translator.lock().clone().expect("translator loaded");
    let target_text = translator.translate(&text, &src_lang, &target_lang)?;

    let grammar = engines.grammar.lock().clone().expect("grammar loaded");
    let study_text = if target_lang == cfg.learning { &target_text } else { &text };
    let tokens = grammar.analyze(study_text, &cfg.learning).unwrap_or_default();

    let clip_path = if is_final { write_wav(clip_dir, &o.id, &o.pcm).ok() } else { None };

    Ok(Some(Segment {
        id: o.id.clone(),
        speaker: o.speaker.clone(),
        role: o.role,
        started_ms: o.started_ms,
        ended_ms: o.ended_ms,
        source_lang: src_lang,
        source_text: text,
        target_lang,
        target_text,
        tokens,
        clip_path: clip_path.map(|p| p.to_string_lossy().to_string()),
        is_final,
        revision: o.revision,
        arrived_at: o.arrived_at,
    }))
}

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
    let data: Vec<u8> = pcm.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes()).collect();
    let mut buf = Vec::with_capacity(44 + data.len());
    buf.extend(b"RIFF"); buf.extend(((36 + data.len()) as u32).to_le_bytes()); buf.extend(b"WAVEfmt ");
    buf.extend(16u32.to_le_bytes()); buf.extend(1u16.to_le_bytes()); buf.extend(1u16.to_le_bytes());
    buf.extend(16_000u32.to_le_bytes()); buf.extend(32_000u32.to_le_bytes()); buf.extend(2u16.to_le_bytes()); buf.extend(16u16.to_le_bytes());
    buf.extend(b"data"); buf.extend((data.len() as u32).to_le_bytes()); buf.extend(data);
    std::fs::write(&path, buf)?;
    Ok(path)
}

pub fn read_wav(path: &str) -> Result<Vec<f32>> {
    let bytes = std::fs::read(path)?;
    Ok(bytes[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32).collect())
}
