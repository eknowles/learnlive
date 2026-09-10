//! LearnLive — single-binary live translation coach.
//!
//! Data flow:
//!   cpal devices ─▶ audio::mixer ─▶ 16 kHz mono ring ─▶ engine::vad ─▶ utterance
//!   ─▶ engine::asr (Whisper) ─▶ engine::diarize (speaker embedding + online clustering)
//!   ─▶ engine::translate (NLLB-200) ─▶ engine::grammar (UD POS tagger)
//!   ─▶ Tauri event `segment` ─▶ UI      (and optionally ─▶ engine::tts ─▶ speakers, ducked)

pub mod audio;
pub mod calendar;
pub mod db;
pub mod commands;
pub mod engine;
pub mod eval;
pub mod models;
pub mod pipeline;
pub mod types;

use std::sync::Arc;
use parking_lot::Mutex;
use tauri::Manager;

/// Global app state handed to every command.
pub struct AppState {
    pub session: Mutex<Option<pipeline::SessionHandle>>,
    pub engines: Arc<engine::Engines>,
    pub model_dir: std::path::PathBuf,
    pub db: Arc<db::Db>,
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let model_dir = models::model_dir(app.handle())?;
            let data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data)?;
            let db = Arc::new(db::Db::open(&data.join("learnlive.sqlite"))?);
            app.manage(AppState {
                session: Mutex::new(None),
                engines: Arc::new(engine::Engines::new(model_dir.clone())),
                model_dir,
                db,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_audio_devices,
            commands::list_languages,
            commands::model_status,
            commands::prepare_models,
            commands::start_session,
            commands::stop_session,
            commands::update_mixer,
            commands::rename_speaker,
            commands::speak,
            commands::play_clip,
            commands::calendar_events_near_now,
            commands::link_meeting,
            commands::assign_speaker,
            commands::list_meetings,
            commands::get_meeting,
            commands::search_history,
            commands::delete_meeting,
            commands::get_remember_voices,
            commands::set_remember_voices,
            commands::forget_voice,
        ])
        .run(tauri::generate_context!())
        .expect("error while running LearnLive");
}
