use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::engine::languages::{Language, LANGUAGES};
use crate::models;
use crate::types::{AudioDevice, MixerSource, SessionConfig};
use crate::{pipeline, AppState};

type CmdResult<T> = Result<T, String>;
fn err<E: std::fmt::Display>(e: E) -> String { format!("{e:#}") }

#[tauri::command]
pub fn list_audio_devices() -> CmdResult<Vec<AudioDevice>> {
    crate::audio::list_devices().map_err(err)
}

#[tauri::command]
pub fn list_languages() -> Vec<Language> {
    LANGUAGES.to_vec()
}

#[derive(Serialize)]
pub struct ModelStatus { pub id: String, pub kind: String, pub present: bool, pub approx_mb: u32 }

/// Which models this config needs, and whether they're on disk.
#[tauri::command]
pub fn model_status(state: State<AppState>, cfg: SessionConfig) -> Vec<ModelStatus> {
    required(&cfg).into_iter().map(|s| ModelStatus {
        id: s.id.into(), kind: s.kind.into(), present: models::is_present(&state.model_dir, &s), approx_mb: s.approx_mb,
    }).collect()
}

fn required(cfg: &SessionConfig) -> Vec<models::ModelSpec> {
    let mut v = vec![models::VAD, models::asr(&cfg.asr_model), models::TRANSLATE, models::POS];
    if cfg.diarize { v.push(models::SPEAKER); }
    if cfg.speak_translations {
        for l in [&cfg.learning, &cfg.native] { if let Some(t) = models::tts(l) { v.push(t); } }
    }
    v
}

/// Download anything missing. Emits `model-progress` events.
#[tauri::command]
pub async fn prepare_models(app: AppHandle, cfg: SessionConfig) -> CmdResult<()> {
    let dir = models::model_dir(&app).map_err(err)?;
    for spec in required(&cfg) {
        models::ensure(&app, &dir, &spec).await.map_err(err)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn start_session(app: AppHandle, state: State<'_, AppState>, cfg: SessionConfig) -> CmdResult<()> {
    if cfg.sources.is_empty() { return Err("Pick at least one audio source".into()); }
    if let Some(old) = state.session.lock().take() { old.stop(); }
    let clip_dir = app.path().app_cache_dir().map_err(err)?.join("clips");
    let engines = state.engines.clone();
    let app2 = app.clone();
    // Engine warm-up can take seconds; keep the UI thread free.
    let handle = tauri::async_runtime::spawn_blocking(move || pipeline::start(app2, engines, cfg, clip_dir))
        .await.map_err(err)?.map_err(err)?;
    *state.session.lock() = Some(handle);
    Ok(())
}

#[tauri::command]
pub fn stop_session(state: State<AppState>) -> CmdResult<()> {
    if let Some(s) = state.session.lock().take() { s.stop(); }
    Ok(())
}

#[tauri::command]
pub fn update_mixer(state: State<AppState>, source: MixerSource) -> CmdResult<()> {
    let guard = state.session.lock();
    let Some(s) = guard.as_ref() else { return Ok(()) };
    let m = s.mixer();
    m.send(crate::audio::MixerCommand::SetGain { device_id: source.device_id.clone(), gain: source.gain }).map_err(err)?;
    m.send(crate::audio::MixerCommand::SetMuted { device_id: source.device_id, muted: source.muted }).map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn rename_speaker(state: State<AppState>, id: u32, label: String) -> CmdResult<()> {
    if let Some(s) = state.session.lock().as_ref() { s.speakers.lock().rename(id, label); }
    Ok(())
}

/// Speak arbitrary text (word hover ▶, "say it slower", etc.). Works without a running session.
#[tauri::command]
pub async fn speak(state: State<'_, AppState>, text: String, lang: String, speed: Option<f32>) -> CmdResult<()> {
    let engines = state.engines.clone();
    let player: Arc<crate::audio::Player> = match state.session.lock().as_ref() {
        Some(s) => s.player.clone(),
        None => Arc::new(crate::audio::Player::open().map_err(err)?),
    };
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<()> {
        let voice = {
            let mut tts = engines.tts.lock();
            if !tts.contains_key(&lang) {
                if let Some(v) = crate::engine::tts::Piper::load(&engines.model_dir, &lang)? { tts.insert(lang.clone(), Arc::new(v)); }
            }
            tts.get(&lang).cloned()
        };
        let Some(voice) = voice else { anyhow::bail!("No voice installed for {lang}") };
        let (pcm, rate) = voice.synthesize(&text)?;
        // Slow-down by playing at a lower nominal rate (simple, pitch drops slightly — acceptable for study).
        let rate = (rate as f32 * speed.unwrap_or(1.0)) as u32;
        player.play(&pcm, rate);
        std::thread::sleep(std::time::Duration::from_millis((pcm.len() as f32 / rate as f32 * 1000.0) as u64));
        Ok(())
    }).await.map_err(err)?.map_err(err)
}

/// Replay the original audio of a segment.
#[tauri::command]
pub async fn play_clip(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    let player = match state.session.lock().as_ref() {
        Some(s) => s.player.clone(),
        None => Arc::new(crate::audio::Player::open().map_err(err)?),
    };
    let pcm = pipeline::read_wav(&path).map_err(err)?;
    player.play(&pcm, 16_000);
    Ok(())
}
