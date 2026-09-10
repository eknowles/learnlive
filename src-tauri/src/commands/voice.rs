use std::sync::Arc;

use tauri::State;

use crate::error::{AppError, CmdResult};
use crate::pipeline::{play_gated, EchoGate};
use crate::AppState;

/// The session's speakers and its echo gate. Anything we play has to hold that gate or the
/// pipeline hears it, transcribes it and — for call-side audio — speaks it back. With no
/// session running there is no pipeline to protect, so a throwaway gate will do.
fn output_for(state: &State<AppState>) -> CmdResult<(Arc<crate::audio::Player>, Arc<EchoGate>)> {
    match state.session.lock().as_ref() {
        Some(s) => Ok((s.player.clone(), s.gate.clone())),
        None => Ok((Arc::new(crate::audio::Player::open()?), Arc::new(EchoGate::default()))),
    }
}

/// Speak arbitrary text (word hover ▶, "say it slower", etc.). Works without a running session.
#[tauri::command]
pub async fn speak(state: State<'_, AppState>, text: String, lang: String, speed: Option<f32>) -> CmdResult<()> {
    let engines = state.engines.clone();
    let (player, gate) = output_for(&state)?;
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<()> {
        let voice = {
            let mut tts = engines.tts.lock();
            if !tts.contains_key(&lang) {
                if let Some(v) = crate::engine::tts::Piper::load(&engines.model_dir, &lang)? {
                    tts.insert(lang.clone(), Arc::new(v));
                }
            }
            tts.get(&lang).cloned()
        };
        let Some(voice) = voice else { anyhow::bail!("No voice installed for {lang}") };
        let (pcm, rate) = voice.synthesize(&text)?;
        let rate = (rate as f32 * speed.unwrap_or(1.0).clamp(0.5, 1.5)) as u32;
        play_gated(&gate, &player, &pcm, rate);
        Ok(())
    })
    .await?
    .map_err(|e| AppError::models(format!("{e:#}")))
}

/// Replay the original audio of a segment. Blocking on its own thread: `play_gated` waits for
/// the clip to finish so the gate covers all of it.
#[tauri::command]
pub async fn play_clip(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    let (player, gate) = output_for(&state)?;
    let pcm = crate::pipeline::read_wav(&path)?;
    tauri::async_runtime::spawn_blocking(move || play_gated(&gate, &player, &pcm, 16_000)).await?;
    Ok(())
}
