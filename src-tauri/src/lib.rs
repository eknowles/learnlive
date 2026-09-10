//! LearnLive — single-binary live translation coach.
//!
//! Data flow:
//!   cpal devices ─▶ audio::mixer ─▶ 16 kHz mono ring ─▶ engine::vad ─▶ utterance
//!   ─▶ engine::asr (Whisper) ─▶ engine::diarize (speaker embedding + online clustering)
//!   ─▶ engine::translate (NLLB-200) ─▶ engine::grammar (UD POS tagger)
//!   ─▶ Tauri event `segment` ─▶ UI      (and optionally ─▶ engine::tts ─▶ speakers, ducked)

pub mod audio;
pub mod calendar;
pub mod commands;
pub mod db;
pub mod engine;
pub mod error;
pub mod eval;
pub mod models;
pub mod pipeline;
pub mod types;

use parking_lot::Mutex;
use std::sync::Arc;
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
        .invoke_handler(all_commands!())
        .run(tauri::generate_context!())
        .expect("error while running LearnLive");
}
