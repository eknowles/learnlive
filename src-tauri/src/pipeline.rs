use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

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

/// Handle the UI holds while a session runs.
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

/// Work item handed from the audio thread to the (slower) ML thread.
struct Job {
    utt: Utterance,
    role: SourceRole,
}

pub fn start(app: AppHandle, engines: Arc<Engines>, cfg: SessionConfig, clip_dir: PathBuf) -> Result<SessionHandle> {
    engines.warm(&cfg)?;
    let mixer = Mixer::start(&cfg.sources)?;
    let player = Arc::new(Player::open()?);
    let running = Arc::new(AtomicBool::new(true));
    let speakers = Arc::new(Mutex::new(SpeakerRegistry::new(512)));
    let (job_tx, job_rx) = bounded::<Job>(16);
    std::fs::create_dir_all(&clip_dir)?;

    // ---- audio thread: frames → utterances -------------------------------------------------
    {
        let running = running.clone();
        let app = app.clone();
        let model_dir = engines.model_dir.clone();
        let frames = mixer.frames.clone();
        thread::Builder::new().name("learnlive-vad".into()).spawn(move || {
            let mut seg = match Segmenter::new(&model_dir) { Ok(s) => s, Err(e) => { error!("vad: {e}"); return; } };
            let mut role_votes = (0u32, 0u32); // (local, remote) during the current utterance
            let mut last_meter = Instant::now();
            while running.load(Ordering::SeqCst) {
                let Ok(MixFrame { mix, rms, dominant }) = frames.recv_timeout(Duration::from_millis(100)) else { continue };
                match dominant { Some(SourceRole::Local) => role_votes.0 += 1, Some(SourceRole::Remote) => role_votes.1 += 1, None => {} }

                match seg.push(&mix) {
                    Ok((speaking, Some(utt))) => {
                        let role = if role_votes.0 > role_votes.1 { SourceRole::Local } else { SourceRole::Remote };
                        role_votes = (0, 0);
                        if job_tx.try_send(Job { utt, role }).is_err() { warn!("ML thread busy; dropped an utterance"); }
                        let _ = speaking;
                    }
                    Ok(_) => {}
                    Err(e) => error!("vad push: {e}"),
                }

                if last_meter.elapsed() > Duration::from_millis(66) {
                    last_meter = Instant::now();
                    let level = (mix.iter().map(|x| x * x).sum::<f32>() / mix.len() as f32).sqrt();
                    let _ = app.emit("levels", Levels { per_source: rms, mix: level, speech_active: seg_active(&seg) });
                }
            }
        })?;
    }

    // ---- ML thread: utterance → segment ----------------------------------------------------
    {
        let running = running.clone();
        let app = app.clone();
        let engines = engines.clone();
        let cfg2 = cfg.clone();
        let speakers = speakers.clone();
        let player = player.clone();
        let mixer_cmd = mixer.commands.clone();
        thread::Builder::new().name("learnlive-ml".into()).spawn(move || {
            while running.load(Ordering::SeqCst) {
                let Ok(job) = job_rx.recv_timeout(Duration::from_millis(100)) else { continue };
                match process(&engines, &cfg2, &speakers, &job, &clip_dir) {
                    Ok(Some(segment)) => {
                        let _ = app.emit("segment", &segment);
                        if cfg2.speak_translations && job.role == SourceRole::Remote {
                            speak(&engines, &cfg2, &player, &mixer_cmd, &segment.target_lang, &segment.target_text);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => { error!("pipeline: {e:#}"); let _ = app.emit("pipeline-error", e.to_string()); }
                }
            }
        })?;
    }

    info!("session started: learning={} native={} sources={}", cfg.learning, cfg.native, cfg.sources.len());
    Ok(SessionHandle { running, mixer_cmd: mixer.commands.clone(), speakers, player, cfg })
}

fn seg_active(_s: &Segmenter) -> bool { true }

fn process(engines: &Engines, cfg: &SessionConfig, speakers: &Mutex<SpeakerRegistry>, job: &Job, clip_dir: &PathBuf) -> Result<Option<Segment>> {
    let asr = engines.asr.lock().clone().expect("asr loaded");
    let (src_lang, text) = asr.transcribe(&job.utt.pcm, &cfg.source_lang)?;
    if text.is_empty() || text.len() < 2 { return Ok(None); }

    // Who said it? Local mic = you; otherwise embed + cluster.
    let speaker: SpeakerRef = if job.role == SourceRole::Local {
        speakers.lock().local()
    } else if let Some(emb) = engines.speaker.lock().clone() {
        let e = emb.embed(&job.utt.pcm)?;
        speakers.lock().identify(&e)
    } else {
        SpeakerRef { id: 1, label: "Speaker".into(), confidence: 0.0 }
    };

    // Translation direction: anything in your learning language → native (so you understand),
    // anything else (incl. your own speech) → learning language (so you learn how to say it).
    let target_lang = if src_lang == cfg.learning { cfg.native.clone() } else { cfg.learning.clone() };
    let translator = engines.translator.lock().clone().expect("translator loaded");
    let target_text = translator.translate(&text, &src_lang, &target_lang)?;

    // Grammar on whichever side is the learning language — that's the text you study.
    let grammar = engines.grammar.lock().clone().expect("grammar loaded");
    let study_text = if target_lang == cfg.learning { &target_text } else { &text };
    let tokens = grammar.analyze(study_text, &cfg.learning).unwrap_or_default();

    let id = uuid::Uuid::new_v4().to_string();
    let clip_path = write_wav(clip_dir, &id, &job.utt.pcm).ok();

    Ok(Some(Segment {
        id,
        speaker,
        role: job.role,
        started_ms: job.utt.started_ms,
        ended_ms: job.utt.ended_ms,
        source_lang: src_lang,
        source_text: text,
        target_lang,
        target_text,
        tokens,
        clip_path: clip_path.map(|p| p.to_string_lossy().to_string()),
    }))
}

/// Read a translation aloud, ducking the remote sources while it plays.
pub fn speak(engines: &Engines, cfg: &SessionConfig, player: &Player, mixer: &Sender<MixerCommand>, lang: &str, text: &str) {
    let Some(voice) = engines.tts.lock().get(lang).cloned() else { return };
    match voice.synthesize(text) {
        Ok((pcm, rate)) => {
            let _ = mixer.send(MixerCommand::Duck(1.0 - cfg.duck_amount));
            player.play(&pcm, rate);
            let ms = (pcm.len() as f32 / rate as f32 * 1000.0) as u64;
            thread::sleep(Duration::from_millis(ms));
            let _ = mixer.send(MixerCommand::Duck(1.0));
        }
        Err(e) => warn!("tts: {e}"),
    }
}

/// Minimal 16-bit PCM WAV writer (no extra crate).
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
