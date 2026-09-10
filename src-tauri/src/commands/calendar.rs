use tauri::State;

use crate::error::CmdResult;
use crate::types::{CalendarEvent, MeetingSummary};
use crate::AppState;

#[tauri::command]
pub async fn calendar_events_near_now() -> CmdResult<Vec<CalendarEvent>> {
    // EventKit may block on the permission prompt; keep it off the UI thread.
    Ok(tauri::async_runtime::spawn_blocking(|| crate::calendar::events_near(chrono::Utc::now().timestamp())).await??)
}

/// Attach (or re-attach) a calendar event to the running or a past meeting.
#[tauri::command]
pub fn link_meeting(state: State<AppState>, meeting_id: i64, event: CalendarEvent) -> CmdResult<MeetingSummary> {
    state.db.link_event(meeting_id, &event.id, &event.title, &super::session::attendees(&event))?;
    state.db.meeting_summary(meeting_id)?.ok_or_else(|| crate::error::AppError::invalid("meeting not found"))
}
