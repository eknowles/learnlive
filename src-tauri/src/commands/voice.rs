use std::sync::Arc;

use tauri::State;

use crate::error::{AppError, CmdResult};
use crate::AppState;

fn player_for(state: &State<AppState>) -> CmdResult<Arc<crate::audio::Player>> {
    match state.session.lock().as_ref() {
        Some(s) => Ok(s.player.clone()),
        None => Ok(Arc::new(crate::audio::Player::open()?)),
    }
}

/// Speak arbitrary text (word hover ▶, "say it slower", etc.). Works without a running session.
#[tauri::command]
pub async fn speak(state: State<'_, AppState>, text: String, lang: String, speed: Option<f32>) -> CmdResult<()> {
    let engines = state.engines.clone();
    let player = player_for(&state)?;
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
        let rate = (rate as f32 * speed.unwrap_or(1.0).clamp(0.5, 1.5)) as u32;
        player.play(&pcm, rate);
        std::thread::sleep(std::time::Duration::from_millis((pcm.len() as f32 / rate as f32 * 1000.0) as u64));
        Ok(())
    }).await?.map_err(|e| AppError::models(format!("{e:#}")))
}

/// Replay the original audio of a segment.
#[tauri::command]
pub async fn play_clip(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    let player = player_for(&state)?;
    let pcm = crate::pipeline::read_wav(&path)?;
    player.play(&pcm, 16_000);
    Ok(())
}
