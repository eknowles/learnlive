use tauri::{AppHandle, Manager, State};

use crate::engine::languages::{Language, LANGUAGES};
use crate::error::{AppError, CmdResult};
use crate::types::{AudioDevice, CalendarEvent, MixerSource, SessionConfig};
use crate::{pipeline, AppState};

#[tauri::command]
pub fn list_audio_devices() -> CmdResult<Vec<AudioDevice>> {
    Ok(crate::audio::list_devices()?)
}

/// Languages with `tts` reflecting which voices we actually know how to fetch.
#[tauri::command]
pub fn list_languages(state: State<AppState>) -> Vec<Language> {
    // `grammar` is an optional model, so report whether it is actually installed rather than
    // just whether the tagger covers the language.
    let tagger = crate::models::is_present(&state.model_dir, &crate::models::pos());
    LANGUAGES
        .iter()
        .map(|l| Language { tts: crate::models::tts(l.code).is_some(), grammar: l.grammar && tagger, ..l.clone() })
        .collect()
}

/// Starts a session and a meeting row. Returns the meeting id so the UI can link/assign.
#[tauri::command]
pub async fn start_session(
    app: AppHandle,
    state: State<'_, AppState>,
    cfg: SessionConfig,
    event: Option<CalendarEvent>,
) -> CmdResult<i64> {
    if cfg.sources.is_empty() {
        return Err(AppError::invalid("Pick at least one audio source"));
    }
    if crate::engine::languages::by_code(&cfg.learning).is_none()
        || crate::engine::languages::by_code(&cfg.native).is_none()
    {
        return Err(AppError::invalid("Unknown language code"));
    }
    if let Some(old) = state.session.lock().take() {
        finish(&state, old);
    }

    let clip_dir = app.path().app_cache_dir()?.join("clips");
    let now = chrono::Utc::now().timestamp();
    let title = event
        .as_ref()
        .map(|e| e.title.clone())
        .unwrap_or_else(|| format!("Session {}", chrono::Local::now().format("%a %-d %b %H:%M")));
    let meeting_id = state.db.start_meeting(&title, event.as_ref().map(|e| e.id.as_str()), &cfg, now)?;
    if let Some(e) = &event {
        state.db.link_event(meeting_id, &e.id, &e.title, &attendees(e))?;
    }

    let (engines, db, app2) = (state.engines.clone(), state.db.clone(), app.clone());
    let handle =
        tauri::async_runtime::spawn_blocking(move || pipeline::start(app2, engines, db, meeting_id, cfg, clip_dir))
            .await??;
    *state.session.lock() = Some(handle);
    Ok(meeting_id)
}

#[tauri::command]
pub fn stop_session(state: State<AppState>) -> CmdResult<()> {
    if let Some(s) = state.session.lock().take() {
        finish(&state, s);
    }
    Ok(())
}

#[tauri::command]
pub fn update_mixer(state: State<AppState>, source: MixerSource) -> CmdResult<()> {
    let guard = state.session.lock();
    let Some(s) = guard.as_ref() else { return Ok(()) };
    let m = s.mixer();
    m.send(crate::audio::MixerCommand::SetGain { device_id: source.device_id.clone(), gain: source.gain })
        .map_err(|e| AppError::audio(e.to_string()))?;
    m.send(crate::audio::MixerCommand::SetMuted { device_id: source.device_id, muted: source.muted })
        .map_err(|e| AppError::audio(e.to_string()))?;
    Ok(())
}

/// Live-only rename (no meeting record). `history::assign_speaker` is the persistent version.
#[tauri::command]
pub fn rename_speaker(state: State<AppState>, id: u32, label: String) -> CmdResult<()> {
    if let Some(s) = state.session.lock().as_ref() {
        s.speakers.lock().rename(id, label);
    }
    Ok(())
}

pub(crate) fn attendees(e: &CalendarEvent) -> Vec<(String, Option<String>)> {
    e.attendees.iter().map(|a| (a.name.clone(), Some(a.email.clone()).filter(|s| !s.is_empty()))).collect()
}

/// Stop whatever session is running (used when the app quits).
pub(crate) fn shutdown(state: &AppState) {
    if let Some(s) = state.session.lock().take() {
        finish(state, s);
    }
}

/// Stop + close the meeting row + fold session voices into voiceprints (only if enabled, and
/// only for speakers the user assigned to a named participant). Never stores your own mic.
fn finish(state: &AppState, s: pipeline::SessionHandle) {
    s.stop();
    let _ = state.db.end_meeting(s.meeting_id, chrono::Utc::now().timestamp());
    if !state.db.remember_voices() {
        return;
    }
    let assigned: std::collections::HashMap<u32, i64> = state
        .db
        .meeting_summary(s.meeting_id)
        .ok()
        .flatten()
        .map(|m| m.participants.into_iter().filter_map(|p| p.speaker_id.map(|sid| (sid, p.id))).collect())
        .unwrap_or_default();
    for (sid, known_pid, centroid, n) in s.speakers.lock().centroids() {
        if sid == 0 || n == 0 {
            continue;
        }
        if let Some(pid) = assigned.get(&sid).copied().or(known_pid) {
            let _ = state.db.update_voiceprint(pid, &centroid, n);
        }
    }
}
