//! Speaker detection: ERes2Net embeddings + online centroid clustering.
//!
//! We don't need offline pyannote-style diarization: utterances are already split by VAD,
//! so "who said this?" reduces to embedding each utterance and matching against known speakers.

use std::path::Path;

use anyhow::Result;
use parking_lot::Mutex;
use sherpa_rs::embedding_manager::EmbeddingManager;
use sherpa_rs::speaker_id::{EmbeddingExtractor, ExtractorConfig};

use super::SpeakerEmbedder;
use crate::types::SpeakerRef;

pub struct Eres2Net {
    inner: Mutex<EmbeddingExtractor>,
}

impl Eres2Net {
    pub fn load(model_dir: &Path) -> Result<Self> {
        let file = model_dir.join(crate::models::SPEAKER.dir)
            .join("3dspeaker_speech_eres2net_base_sv_zh-cn_3dspeaker_16k.onnx");
        let cfg = ExtractorConfig { model: file.to_string_lossy().into(), num_threads: Some(2), ..Default::default() };
        Ok(Self { inner: Mutex::new(EmbeddingExtractor::new(cfg)?) })
    }
}

impl SpeakerEmbedder for Eres2Net {
    fn embed(&self, pcm16k: &[f32]) -> Result<Vec<f32>> {
        Ok(self.inner.lock().compute_speaker_embedding(pcm16k.to_vec(), 16_000)?)
    }
}

/// Tracks speakers within one session.
pub struct SpeakerRegistry {
    manager: EmbeddingManager,
    labels: Vec<(u32, String)>,
    /// Cosine similarity needed to match an existing speaker. 0.55–0.7 works well for ERes2Net.
    threshold: f32,
    next_id: u32,
    /// Reserved id for the local mic ("You") so it never gets clustered with remote voices.
    local_id: u32,
}

impl SpeakerRegistry {
    pub fn new(dim: usize) -> Self {
        let mut r = Self { manager: EmbeddingManager::new(dim as i32), labels: vec![], threshold: 0.6, next_id: 1, local_id: 0 };
        r.labels.push((0, "You".into()));
        r
    }

    pub fn local(&self) -> SpeakerRef {
        SpeakerRef { id: self.local_id, label: self.label_of(self.local_id), confidence: 1.0 }
    }

    /// Match or create. Returns the speaker plus similarity to its centroid.
    pub fn identify(&mut self, embedding: &[f32]) -> SpeakerRef {
        if let Some(name) = self.manager.search(embedding, self.threshold) {
            let id: u32 = name.parse().unwrap_or(0);
            // Refine centroid so the speaker model improves over the call.
            let _ = self.manager.add(name.clone(), &mut embedding.to_vec());
            return SpeakerRef { id, label: self.label_of(id), confidence: self.threshold + 0.1 };
        }
        let id = self.next_id;
        self.next_id += 1;
        let _ = self.manager.add(id.to_string(), &mut embedding.to_vec());
        self.labels.push((id, format!("Speaker {id}")));
        SpeakerRef { id, label: self.label_of(id), confidence: 0.5 }
    }

    pub fn rename(&mut self, id: u32, label: String) {
        if let Some(l) = self.labels.iter_mut().find(|(i, _)| *i == id) { l.1 = label; }
    }

    fn label_of(&self, id: u32) -> String {
        self.labels.iter().find(|(i, _)| *i == id).map(|l| l.1.clone()).unwrap_or_else(|| format!("Speaker {id}"))
    }
}
