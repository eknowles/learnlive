//! Headless model download for CI / scripting (no Tauri window).
//!   cargo run --bin model-fetch -- --models DIR [--asr small] [--langs ru,en]
use anyhow::Result;
use learnlive_lib::models;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let get = |k: &str, d: &str| a.iter().position(|x| x == k).map(|i| a[i + 1].clone()).unwrap_or(d.into());
    let dir = PathBuf::from(get("--models", ".models"));
    let asr = get("--asr", "small");
    let langs: Vec<String> = get("--langs", "ru,en").split(',').map(String::from).collect();
    std::fs::create_dir_all(&dir)?;

    let cfg = learnlive_lib::types::SessionConfig {
        asr_model: asr,
        learning: langs[0].clone(),
        native: langs.get(1).cloned().unwrap_or_else(|| "en".into()),
        ..Default::default()
    };
    for s in models::required(&cfg) {
        if models::is_present(&dir, &s) {
            eprintln!("ok      {}", s.id);
            continue;
        }
        eprintln!("fetch   {} (~{} MB)", s.id, s.approx_mb);
        models::ensure_headless(&dir, &s).await?;
    }
    Ok(())
}
