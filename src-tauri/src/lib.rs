//! LearnLive — single-binary live translation coach.
//!
//! Data flow:
//!   cpal devices ─▶ audio::mixer ─▶ 16 kHz mono ring ─▶ engine::vad ─▶ utterance
//!   ─▶ engine::asr (Whisper) ─▶ engine::diarize (speaker embedding + online clustering)
//!   ─▶ engine::translate (NLLB-200) ─▶ engine::grammar (UD POS tagger)
//!   ─▶ Tauri event `segment` ─▶ UI      (and optionally ─▶ engine::tts ─▶ speakers, ducked)

pub mod audio;
pub mod commands;
pub mod engine;
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
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let model_dir = models::model_dir(app.handle())?;
            app.manage(AppState {
                session: Mutex::new(None),
                engines: Arc::new(engine::Engines::new(model_dir.clone())),
                model_dir,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running LearnLive");
}
