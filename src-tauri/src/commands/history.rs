use tauri::State;

use crate::error::{AppError, CmdResult};
use crate::types::{MeetingSummary, SearchHit, Segment};
use crate::AppState;

/// "Speaker 2 is Anna". Renames live labels too.
#[tauri::command]
pub fn assign_speaker(
    state: State<AppState>,
    meeting_id: i64,
    speaker_id: u32,
    name: String,
    email: Option<String>,
) -> CmdResult<MeetingSummary> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::invalid("Name can't be empty"));
    }
    state.db.assign_speaker(meeting_id, speaker_id, &name, email.as_deref())?;
    if let Some(s) = state.session.lock().as_ref() {
        if s.meeting_id == meeting_id {
            s.speakers.lock().rename(speaker_id, name);
        }
    }
    state.db.meeting_summary(meeting_id)?.ok_or_else(|| AppError::invalid("meeting not found"))
}

#[tauri::command]
pub fn list_meetings(state: State<AppState>, limit: Option<i64>) -> CmdResult<Vec<MeetingSummary>> {
    Ok(state.db.list_meetings(limit.unwrap_or(200))?)
}

#[tauri::command]
pub fn get_meeting(state: State<AppState>, id: i64) -> CmdResult<Option<(MeetingSummary, Vec<Segment>)>> {
    Ok(state.db.meeting(id)?)
}

#[tauri::command]
pub fn search_history(state: State<AppState>, query: String, limit: Option<i64>) -> CmdResult<Vec<SearchHit>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    Ok(state.db.search(&query, limit.unwrap_or(50))?)
}

#[tauri::command]
pub fn delete_meeting(state: State<AppState>, id: i64) -> CmdResult<()> {
    for clip in state.db.delete_meeting(id)? {
        let _ = std::fs::remove_file(clip);
    }
    Ok(())
}

#[tauri::command]
pub fn get_remember_voices(state: State<AppState>) -> bool {
    state.db.remember_voices()
}

#[tauri::command]
pub fn set_remember_voices(state: State<AppState>, on: bool) -> CmdResult<()> {
    Ok(state.db.set_remember_voices(on)?)
}

#[tauri::command]
pub fn forget_voice(state: State<AppState>, participant_id: i64) -> CmdResult<()> {
    Ok(state.db.forget_voice(participant_id)?)
}
