use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::engine::languages::{Language, LANGUAGES};
use crate::models;
use crate::types::{AudioDevice, CalendarEvent, MeetingSummary, MixerSource, SearchHit, Segment, SessionConfig};
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

/// Starts a session and a meeting row. Returns the meeting id so the UI can link/assign.
#[tauri::command]
pub async fn start_session(app: AppHandle, state: State<'_, AppState>, cfg: SessionConfig, event: Option<CalendarEvent>) -> CmdResult<i64> {
    if cfg.sources.is_empty() { return Err("Pick at least one audio source".into()); }
    if let Some(old) = state.session.lock().take() { finish(&state, old); }
    let clip_dir = app.path().app_cache_dir().map_err(err)?.join("clips");
    let now = chrono::Utc::now().timestamp();
    let title = event.as_ref().map(|e| e.title.clone()).unwrap_or_else(|| format!("Session {}", chrono::Local::now().format("%a %-d %b %H:%M")));
    let meeting_id = state.db.start_meeting(&title, event.as_ref().map(|e| e.id.as_str()), &cfg, now).map_err(err)?;
    if let Some(e) = &event {
        let att: Vec<(String, Option<String>)> = e.attendees.iter().map(|a| (a.name.clone(), Some(a.email.clone()).filter(|s| !s.is_empty()))).collect();
        state.db.link_event(meeting_id, &e.id, &e.title, &att).map_err(err)?;
    }
    let (engines, db, app2) = (state.engines.clone(), state.db.clone(), app.clone());
    let handle = tauri::async_runtime::spawn_blocking(move || pipeline::start(app2, engines, db, meeting_id, cfg, clip_dir))
        .await.map_err(err)?.map_err(err)?;
    *state.session.lock() = Some(handle);
    Ok(meeting_id)
}

#[tauri::command]
pub fn stop_session(state: State<AppState>) -> CmdResult<()> {
    if let Some(s) = state.session.lock().take() { finish(&state, s); }
    Ok(())
}

/// Stop + close the meeting row + fold session voices into voiceprints (only if enabled, and
/// only for speakers the user assigned to a named participant).
fn finish(state: &State<AppState>, s: pipeline::SessionHandle) {
    s.stop();
    let _ = state.db.end_meeting(s.meeting_id, chrono::Utc::now().timestamp());
    if state.db.remember_voices() {
        let assigned: std::collections::HashMap<u32, i64> = state.db.list_meetings(i64::MAX).ok()
            .and_then(|ms| ms.into_iter().find(|m| m.id == s.meeting_id))
            .map(|m| m.participants.into_iter().filter_map(|p| p.speaker_id.map(|sid| (sid, p.id))).collect())
            .unwrap_or_default();
        for (sid, known_pid, centroid, n) in s.speakers.lock().centroids() {
            if sid == 0 || n == 0 { continue; } // never store your own mic; nothing to store
            if let Some(pid) = assigned.get(&sid).copied().or(known_pid) {
                let _ = state.db.update_voiceprint(pid, &centroid, n);
            }
        }
    }
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

// ---- calendar & history ---------------------------------------------------------------------

#[tauri::command]
pub async fn calendar_events_near_now() -> CmdResult<Vec<CalendarEvent>> {
    // EventKit may block on the permission prompt; keep it off the UI thread.
    tauri::async_runtime::spawn_blocking(|| crate::calendar::events_near(chrono::Utc::now().timestamp()))
        .await.map_err(err)?.map_err(err)
}

/// Attach (or re-attach) a calendar event to the running or a past meeting.
#[tauri::command]
pub fn link_meeting(state: State<AppState>, meeting_id: i64, event: CalendarEvent) -> CmdResult<MeetingSummary> {
    let att: Vec<(String, Option<String>)> = event.attendees.iter().map(|a| (a.name.clone(), Some(a.email.clone()).filter(|s| !s.is_empty()))).collect();
    state.db.link_event(meeting_id, &event.id, &event.title, &att).map_err(err)?;
    summary(&state, meeting_id)
}

/// "Speaker 2 is Anna". Renames live labels too.
#[tauri::command]
pub fn assign_speaker(state: State<AppState>, meeting_id: i64, speaker_id: u32, name: String, email: Option<String>) -> CmdResult<MeetingSummary> {
    state.db.assign_speaker(meeting_id, speaker_id, &name, email.as_deref()).map_err(err)?;
    if let Some(s) = state.session.lock().as_ref() { if s.meeting_id == meeting_id { s.speakers.lock().rename(speaker_id, name); } }
    summary(&state, meeting_id)
}

fn summary(state: &State<AppState>, meeting_id: i64) -> CmdResult<MeetingSummary> {
    state.db.list_meetings(i64::MAX).map_err(err)?.into_iter().find(|m| m.id == meeting_id).ok_or_else(|| "meeting not found".into())
}

#[tauri::command]
pub fn list_meetings(state: State<AppState>, limit: Option<i64>) -> CmdResult<Vec<MeetingSummary>> {
    state.db.list_meetings(limit.unwrap_or(200)).map_err(err)
}

#[tauri::command]
pub fn get_meeting(state: State<AppState>, id: i64) -> CmdResult<Option<(MeetingSummary, Vec<Segment>)>> {
    state.db.meeting(id).map_err(err)
}

#[tauri::command]
pub fn search_history(state: State<AppState>, query: String, limit: Option<i64>) -> CmdResult<Vec<SearchHit>> {
    if query.trim().is_empty() { return Ok(vec![]); }
    state.db.search(&query, limit.unwrap_or(50)).map_err(err)
}

#[tauri::command]
pub fn delete_meeting(state: State<AppState>, id: i64) -> CmdResult<()> {
    for clip in state.db.delete_meeting(id).map_err(err)? { let _ = std::fs::remove_file(clip); }
    Ok(())
}

#[tauri::command]
pub fn get_remember_voices(state: State<AppState>) -> bool { state.db.remember_voices() }

#[tauri::command]
pub fn set_remember_voices(state: State<AppState>, on: bool) -> CmdResult<()> { state.db.set_remember_voices(on).map_err(err) }

#[tauri::command]
pub fn forget_voice(state: State<AppState>, participant_id: i64) -> CmdResult<()> { state.db.forget_voice(participant_id).map_err(err) }
