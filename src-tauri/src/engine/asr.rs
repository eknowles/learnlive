//! Whisper (multilingual) through sherpa-onnx. Gives text + detected language per utterance.

use std::path::Path;

use anyhow::Result;
use parking_lot::Mutex;
use sherpa_rs::whisper::{WhisperConfig, WhisperRecognizer};

use super::Transcriber;

pub struct WhisperOnnx {
    // sherpa's recognizer is not Sync; serialize calls. ASR is the bottleneck anyway.
    inner: Mutex<WhisperRecognizer>,
}

impl WhisperOnnx {
    pub fn load(model_dir: &Path, size: &str) -> Result<Self> {
        let dir = model_dir.join(crate::models::asr(size).dir);
        let stem = |s: &str| dir.join(format!("{size}-{s}.int8.onnx")).to_string_lossy().to_string();
        let cfg = WhisperConfig {
            encoder: stem("encoder"),
            decoder: stem("decoder"),
            tokens: dir.join(format!("{size}-tokens.txt")).to_string_lossy().into(),
            language: "auto".into(),
            task: "transcribe".into(),
            num_threads: Some(4),
            ..Default::default()
        };
        Ok(Self { inner: Mutex::new(WhisperRecognizer::new(cfg)?) })
    }
}

impl Transcriber for WhisperOnnx {
    fn transcribe(&self, pcm16k: &[f32], lang_hint: &str) -> Result<(String, String)> {
        let mut rec = self.inner.lock();
        let res = rec.transcribe(16_000, pcm16k.to_vec());
        let lang = if lang_hint == "auto" {
            if res.lang.is_empty() { "en".to_string() } else { res.lang.clone() }
        } else {
            lang_hint.to_string()
        };
        Ok((lang, res.text.trim().to_string()))
    }
}
