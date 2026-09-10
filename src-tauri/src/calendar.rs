//! Calendar access. macOS: EventKit through objc2 (read-only). Other platforms: empty.
//! Tauri has no calendar plugin — this is the ~100 lines that stand in for one.

use anyhow::Result;

use crate::types::CalendarEvent;

/// Events overlapping [start, end] (unix seconds), all calendars the user has granted.
pub fn events_between(start: i64, end: i64) -> Result<Vec<CalendarEvent>> {
    imp::events_between(start, end)
}

/// Events that could be "the meeting this session belongs to": in progress now, or starting
/// within the next 15 minutes, or ended in the last 10.
pub fn events_near(now: i64) -> Result<Vec<CalendarEvent>> {
    let mut v = events_between(now - 10 * 60, now + 15 * 60)?;
    // Prefer the one that's actually in progress; then soonest start.
    v.sort_by_key(|e| (!(e.start <= now && e.end >= now), (e.start - now).abs()));
    Ok(v)
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::Bool;
    use objc2_event_kit::{EKAuthorizationStatus, EKEntityType, EKEvent, EKEventStore, EKParticipant};
    use objc2_foundation::{NSArray, NSDate, NSError, NSString};

    fn store() -> Retained<EKEventStore> {
        unsafe { EKEventStore::new() }
    }

    /// Ask once; the OS remembers. Blocks up to 30 s for the user to answer the prompt.
    fn ensure_access(store: &EKEventStore) -> Result<()> {
        let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
        if status == EKAuthorizationStatus::FullAccess || status == EKAuthorizationStatus::Authorized {
            return Ok(());
        }
        let (tx, rx) = mpsc::channel::<bool>();
        let block = RcBlock::new(move |granted: Bool, _err: *mut NSError| { let _ = tx.send(granted.as_bool()); });
        unsafe {
            // macOS 14+: full access; older: entity access. Both resolve through the same callback.
            store.requestFullAccessToEventsWithCompletion(&block);
        }
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(true) => Ok(()),
            Ok(false) => anyhow::bail!("Calendar access was declined. Allow LearnLive in System Settings → Privacy & Security → Calendars."),
            Err(_) => anyhow::bail!("Calendar permission prompt timed out"),
        }
    }

    pub fn events_between(start: i64, end: i64) -> Result<Vec<CalendarEvent>> {
        let store = store();
        ensure_access(&store)?;
        let events: Retained<NSArray<EKEvent>> = unsafe {
            let s = NSDate::dateWithTimeIntervalSince1970(start as f64);
            let e = NSDate::dateWithTimeIntervalSince1970(end as f64);
            let pred = store.predicateForEventsWithStartDate_endDate_calendars(&s, &e, None);
            store.eventsMatchingPredicate(&pred)
        };
        let mut out = vec![];
        for ev in events.iter() {
            unsafe {
                let attendees = ev.attendees().map(|a| {
                    a.iter().map(|p: &EKParticipant| {
                        let name = p.name().map(|n| n.to_string()).unwrap_or_default();
                        // EKParticipant.URL is mailto:someone@example.com
                        let email = p.URL().absoluteString().map(|u| u.to_string()).unwrap_or_default();
                        let email = email.strip_prefix("mailto:").unwrap_or(&email).to_string();
                        crate::types::Attendee { name: if name.is_empty() { email.clone() } else { name }, email }
                    }).collect::<Vec<_>>()
                }).unwrap_or_default();
                let url = ev.URL().and_then(|u| u.absoluteString()).map(|s| s.to_string())
                    .or_else(|| ev.notes().and_then(|n| first_meeting_link(&n.to_string())));
                out.push(CalendarEvent {
                    id: ev.eventIdentifier().map(|s| s.to_string()).unwrap_or_default(),
                    title: ev.title().to_string(),
                    start: ev.startDate().timeIntervalSince1970() as i64,
                    end: ev.endDate().timeIntervalSince1970() as i64,
                    attendees,
                    url,
                    calendar: ev.calendar().map(|c| c.title().to_string()).unwrap_or_default(),
                });
            }
        }
        // Keep the type checker honest about NSString usage in this module.
        let _ = NSString::from_str("");
        Ok(out)
    }

    fn first_meeting_link(notes: &str) -> Option<String> {
        notes.split_whitespace().find(|w| w.contains("meet.google.com") || w.contains("zoom.us/j") || w.contains("teams.microsoft.com")).map(|s| s.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != ':' && c != '.' && c != '?' && c != '=' && c != '-' && c != '_').to_string())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;
    pub fn events_between(_start: i64, _end: i64) -> Result<Vec<CalendarEvent>> {
        Ok(vec![]) // TODO: Google Calendar API route for Windows/Linux
    }
}
