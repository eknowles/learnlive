//! Piper voices through sherpa-onnx VITS.

use std::path::Path;

use anyhow::Result;
use parking_lot::Mutex;
use sherpa_rs::tts::{VitsTts, VitsTtsConfig};

use super::Synthesizer;

pub struct Piper {
    inner: Mutex<VitsTts>,
}

impl Piper {
    /// Returns None if we don't have a voice for this language (UI shows "no voice").
    pub fn load(model_dir: &Path, lang: &str) -> Result<Option<Self>> {
        let Some(spec) = crate::models::tts(lang) else { return Ok(None) };
        let dir = model_dir.join(spec.dir);
        if !dir.exists() { return Ok(None); }
        let onnx = std::fs::read_dir(&dir)?.filter_map(|e| e.ok()).map(|e| e.path())
            .find(|p| p.extension().map(|x| x == "onnx").unwrap_or(false));
        let Some(onnx) = onnx else { return Ok(None) };
        let cfg = VitsTtsConfig {
            model: onnx.to_string_lossy().into(),
            tokens: dir.join("tokens.txt").to_string_lossy().into(),
            data_dir: dir.join("espeak-ng-data").to_string_lossy().into(),
            length_scale: 1.0,
            ..Default::default()
        };
        Ok(Some(Self { inner: Mutex::new(VitsTts::new(cfg)) }))
    }
}

impl Synthesizer for Piper {
    fn synthesize(&self, text: &str) -> Result<(Vec<f32>, u32)> {
        let audio = self.inner.lock().create(text, 0, 1.0)?;
        Ok((audio.samples, audio.sample_rate))
    }
}
