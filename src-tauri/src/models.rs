//! Model manifest + downloader. Models live in the app data dir so the binary stays small
//! (~15 MB) and the user only pays for the languages/quality they pick.
//!
//! Enable the `bundled-models` cargo feature to `include_bytes!` a fixed set instead.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::types::ModelProgress;

#[derive(Debug, Clone, Serialize)]
pub struct ModelSpec {
    pub id: &'static str,
    pub kind: &'static str,
    /// Directory (relative to model_dir) the archive expands to / files are placed in.
    pub dir: &'static str,
    pub url: String,
    pub approx_mb: u32,
    /// A model the app can run without. Download failures are logged and the feature is
    /// disabled for the session rather than blocking `prepare_models`.
    pub optional: bool,
}

/// Whisper ASR (multilingual; also gives us language ID per utterance).
pub fn asr(model: &str) -> ModelSpec {
    let (id, url, mb) = match model {
        "tiny" => (
            "whisper-tiny",
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-tiny.tar.bz2",
            110,
        ),
        "base" => (
            "whisper-base",
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.tar.bz2",
            200,
        ),
        "medium" => (
            "whisper-medium",
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-medium.tar.bz2",
            1500,
        ),
        "large-v3-turbo" => (
            "whisper-turbo",
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-turbo.tar.bz2",
            1600,
        ),
        _ => (
            "whisper-small",
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-small.tar.bz2",
            600,
        ),
    };
    ModelSpec { id, kind: "asr", dir: id, url: url.into(), approx_mb: mb, optional: false }
}

pub fn vad() -> ModelSpec {
    ModelSpec {
        id: "silero-vad",
        kind: "vad",
        dir: "silero-vad",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx".into(),
        approx_mb: 2,
        optional: false,
    }
}

/// Speaker embedding model (3D-Speaker ERes2Net, language-independent).
pub fn speaker() -> ModelSpec {
    ModelSpec { id: "speaker-eres2net", kind: "speaker", dir: "speaker-eres2net", url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/3dspeaker_speech_eres2net_base_sv_zh-cn_3dspeaker_16k.onnx".into(), approx_mb: 26, optional: false }
}

/// NLLB-200 distilled 600M exported to ONNX (int8). One model, 200 languages, any direction.
pub fn translate() -> ModelSpec {
    ModelSpec {
        id: "nllb-200-600m",
        kind: "translate",
        dir: "nllb-200-600m",
        url: "https://huggingface.co/Xenova/nllb-200-distilled-600M/resolve/main/onnx/".into(),
        approx_mb: 700,
        optional: false,
    }
}

/// UD POS tagger for the word-level grammar cards.
///
/// **Currently unavailable.** The referenced repo (`wietsedv/xlm-roberta-base-ft-udpos28-all`)
/// does not exist — that family is published per language (`-en`, `-ru`, …) and as PyTorch
/// weights only. There is no multilingual UPOS model with an ONNX export on the Hub, so this
/// has to be exported and hosted before the feature can work; see ARCHITECTURE.md.
///
/// Marked `optional` so its absence costs you the hover cards and nothing else.
pub fn pos() -> ModelSpec {
    ModelSpec {
        id: "xlmr-udpos",
        kind: "grammar",
        dir: "xlmr-udpos",
        url: "https://huggingface.co/wietsedv/xlm-roberta-base-ft-udpos28-all/resolve/main/".into(),
        approx_mb: 280,
        optional: true,
    }
}

/// TTS voices per language (Piper VITS, via sherpa-onnx). Add more from the sherpa-onnx tts-models release.
pub fn tts(lang: &str) -> Option<ModelSpec> {
    let base = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/";
    let (id, file) = match lang {
        "ru" => ("tts-ru", "vits-piper-ru_RU-irina-medium.tar.bz2"),
        "en" => ("tts-en", "vits-piper-en_GB-alan-medium.tar.bz2"),
        "es" => ("tts-es", "vits-piper-es_ES-davefx-medium.tar.bz2"),
        "fr" => ("tts-fr", "vits-piper-fr_FR-upmc-medium.tar.bz2"),
        "de" => ("tts-de", "vits-piper-de_DE-thorsten-medium.tar.bz2"),
        "it" => ("tts-it", "vits-piper-it_IT-riccardo-x_low.tar.bz2"),
        "pt" => ("tts-pt", "vits-piper-pt_BR-faber-medium.tar.bz2"),
        "pl" => ("tts-pl", "vits-piper-pl_PL-darkman-medium.tar.bz2"),
        "uk" => ("tts-uk", "vits-piper-uk_UA-ukrainian_tts-medium.tar.bz2"),
        "tr" => ("tts-tr", "vits-piper-tr_TR-fettah-medium.tar.bz2"),
        "zh" => ("tts-zh", "vits-piper-zh_CN-huayan-medium.tar.bz2"),
        "ar" => ("tts-ar", "vits-piper-ar_JO-kareem-medium.tar.bz2"),
        _ => return None,
    };
    // A missing voice just means this language is not spoken aloud; `Piper::load`
    // already returns `None` gracefully.
    Some(ModelSpec { id, kind: "tts", dir: id, url: format!("{base}{file}"), approx_mb: 70, optional: true })
}

impl ModelSpec {
    /// Concrete URLs to fetch. HF repos need individual files; GitHub releases are one archive.
    pub fn files(&self) -> Vec<String> {
        match self.kind {
            "translate" => ["encoder_model_quantized.onnx", "decoder_model_quantized.onnx"]
                .iter()
                .map(|f| format!("{}{}", self.url, f))
                .chain(
                    ["tokenizer.json", "config.json"]
                        .iter()
                        .map(|f| format!("{}{}", self.url.trim_end_matches("onnx/"), f)),
                )
                .collect(),
            "grammar" => ["onnx/model_quantized.onnx", "tokenizer.json", "config.json"]
                .iter()
                .map(|f| format!("{}{}", self.url, f))
                .collect(),
            _ => vec![self.url.clone()],
        }
    }
}

/// Every model a config needs — the single source of truth used by the UI status, the
/// downloader, and the headless fetcher.
pub fn required(cfg: &crate::types::SessionConfig) -> Vec<ModelSpec> {
    // `pos()` is deliberately absent: its source repo does not exist, so listing it would put a
    // 280 MB download in the UI's estimate that can only ever 401. `list_languages` still probes
    // the directory, so dropping a tagger in by hand re-enables the grammar cards.
    let mut v = vec![vad(), asr(&cfg.asr_model), translate()];
    if cfg.diarize {
        v.push(speaker());
    }
    if cfg.speak_translations {
        for l in [&cfg.learning, &cfg.native] {
            if let Some(t) = tts(l) {
                v.push(t);
            }
        }
    }
    v
}

pub fn model_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = app.path().app_data_dir()?.join("models");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Written into a model directory only once every file has been fetched and extracted.
/// Without it a half-finished download would look "present" forever and could never be
/// repaired from the UI — the directory is simply non-empty.
const MARKER: &str = ".complete";

pub fn is_present(model_dir: &Path, spec: &ModelSpec) -> bool {
    model_dir.join(spec.dir).join(MARKER).exists()
}

/// Download (and extract, for .tar.bz2) one model, emitting `model-progress` events.
pub async fn ensure(app: &AppHandle, model_dir: &Path, spec: &ModelSpec) -> Result<PathBuf> {
    let app = app.clone();
    let id = spec.id.to_string();
    ensure_with(model_dir, spec, move |bytes, total, done| {
        let _ = app.emit("model-progress", ModelProgress { model: id.clone(), bytes, total, done });
    })
    .await
}

/// Same, without Tauri (CI, scripts, tests).
pub async fn ensure_headless(model_dir: &Path, spec: &ModelSpec) -> Result<PathBuf> {
    ensure_with(model_dir, spec, |_, _, _| {}).await
}

async fn ensure_with(
    model_dir: &Path,
    spec: &ModelSpec,
    mut progress: impl FnMut(u64, Option<u64>, bool),
) -> Result<PathBuf> {
    let target = model_dir.join(spec.dir);
    if is_present(model_dir, spec) {
        return Ok(target);
    }
    // Anything already here is from an interrupted attempt; start clean rather than
    // leaving a mix of old and new files.
    if target.exists() {
        std::fs::remove_dir_all(&target).with_context(|| format!("clearing partial download {}", target.display()))?;
    }
    std::fs::create_dir_all(&target)?;

    // HF repos: fetch the individual files we need rather than an archive.
    let files: Vec<String> = spec.files();

    let client = reqwest::Client::new();
    let mut fetched = 0u64;
    for url in files {
        let fname = url.rsplit('/').next().unwrap().to_string();
        let dest = target.join(&fname);
        let resp = client.get(&url).send().await.with_context(|| format!("GET {url}"))?;
        if resp.status().is_client_error() {
            anyhow::bail!(
                "{url} returned {}. The model source has moved or never existed — retrying will not help.",
                resp.status()
            );
        }
        let resp = resp.error_for_status().with_context(|| format!("GET {url}"))?;
        let total = resp.content_length();
        let mut stream = resp.bytes_stream();
        let mut file = tokio::fs::File::create(&dest).await?;
        let mut bytes = 0u64;
        use tokio::io::AsyncWriteExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            bytes += chunk.len() as u64;
            progress(bytes, total, false);
        }
        file.flush().await?;

        if fname.ends_with(".tar.bz2") {
            extract_tar_bz2(&dest, &target)?;
            std::fs::remove_file(&dest)?;
        }
        fetched += bytes;
    }
    std::fs::write(target.join(MARKER), spec.id)?;
    progress(fetched, Some(fetched), true);
    Ok(target)
}

/// Shell out to `tar` (present on macOS/Linux; on Windows 10+ via bsdtar) to avoid pulling in bzip2 crates.
fn extract_tar_bz2(archive: &Path, into: &Path) -> Result<()> {
    let status = std::process::Command::new("tar")
        .args(["-xjf", archive.to_str().unwrap(), "-C", into.to_str().unwrap(), "--strip-components=1"])
        .status()?;
    if !status.success() {
        return Err(anyhow!("tar failed extracting {}", archive.display()));
    }
    Ok(())
}
