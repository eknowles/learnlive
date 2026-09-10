use serde::Serialize;
use tauri::{AppHandle, State};

use crate::error::CmdResult;
use crate::models;
use crate::types::SessionConfig;
use crate::AppState;

#[derive(Serialize)]
pub struct ModelStatus {
    pub id: String,
    pub kind: String,
    pub present: bool,
    pub approx_mb: u32,
}

/// Which models this config needs, and whether they're on disk.
#[tauri::command]
pub fn model_status(state: State<AppState>, cfg: SessionConfig) -> Vec<ModelStatus> {
    models::required(&cfg)
        .into_iter()
        .map(|s| ModelStatus {
            id: s.id.into(),
            kind: s.kind.into(),
            present: models::is_present(&state.model_dir, &s),
            approx_mb: s.approx_mb,
        })
        .collect()
}

/// Download anything missing. Emits `model-progress` events.
#[tauri::command]
pub async fn prepare_models(app: AppHandle, state: State<'_, AppState>, cfg: SessionConfig) -> CmdResult<()> {
    for spec in models::required(&cfg) {
        match models::ensure(&app, &state.model_dir, &spec).await {
            Ok(_) => {}
            // An optional model failing costs one feature, not the session.
            Err(e) if spec.optional => log::warn!("optional model {} unavailable: {e:#}", spec.id),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
