//! Tauri command surface, one file per concern. Commands are thin: validate, call into the
//! crate, map errors. Logic lives in `pipeline`, `db`, `calendar`, `models`.

pub mod calendar;
pub mod history;
pub mod models;
pub mod session;
pub mod voice;

/// Everything `tauri::generate_handler!` needs, in one place so lib.rs stays short.
#[macro_export]
macro_rules! all_commands {
    () => {
        tauri::generate_handler![
            $crate::commands::session::list_audio_devices,
            $crate::commands::session::list_languages,
            $crate::commands::session::start_session,
            $crate::commands::session::stop_session,
            $crate::commands::session::update_mixer,
            $crate::commands::session::rename_speaker,
            $crate::commands::models::model_status,
            $crate::commands::models::prepare_models,
            $crate::commands::voice::speak,
            $crate::commands::voice::play_clip,
            $crate::commands::calendar::calendar_events_near_now,
            $crate::commands::calendar::link_meeting,
            $crate::commands::history::assign_speaker,
            $crate::commands::history::list_meetings,
            $crate::commands::history::get_meeting,
            $crate::commands::history::search_history,
            $crate::commands::history::delete_meeting,
            $crate::commands::history::get_remember_voices,
            $crate::commands::history::set_remember_voices,
            $crate::commands::history::forget_voice,
        ]
    };
}
