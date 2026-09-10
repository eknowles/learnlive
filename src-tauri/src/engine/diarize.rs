//! Speaker detection: ERes2Net embeddings + online centroid clustering.
//!
//! We don't need offline pyannote-style diarization: utterances are already split by VAD,
//! so "who said this?" reduces to embedding each utterance and matching against known speakers.

use std::path::Path;

use anyhow::{anyhow, Result};
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
        let file = model_dir
            .join(crate::models::speaker().dir)
            .join("3dspeaker_speech_eres2net_base_sv_zh-cn_3dspeaker_16k.onnx");
        let cfg = ExtractorConfig { model: file.to_string_lossy().into(), num_threads: Some(2), ..Default::default() };
        Ok(Self { inner: Mutex::new(EmbeddingExtractor::new(cfg).map_err(|e| anyhow!("speaker model: {e}"))?) })
    }
}

impl SpeakerEmbedder for Eres2Net {
    fn embed(&self, pcm16k: &[f32]) -> Result<Vec<f32>> {
        self.inner.lock().compute_speaker_embedding(pcm16k.to_vec(), 16_000).map_err(|e| anyhow!("embed: {e}"))
    }
}

/// Tracks speakers within one session.
pub struct SpeakerRegistry {
    manager: EmbeddingManager,
    labels: Vec<(u32, String)>,
    /// Running mean per speaker so we can persist a voiceprint at the end of a session.
    centroids: std::collections::HashMap<u32, (Vec<f32>, u32)>,
    /// Speakers seeded from stored voiceprints: speaker id → participant id.
    known: std::collections::HashMap<u32, i64>,
    /// Cosine similarity needed to match an existing speaker. 0.55–0.7 works well for ERes2Net.
    threshold: f32,
    next_id: u32,
    /// Reserved id for the local mic ("You") so it never gets clustered with remote voices.
    local_id: u32,
}

impl SpeakerRegistry {
    pub fn new(dim: usize) -> Self {
        let mut r = Self {
            manager: EmbeddingManager::new(dim as i32),
            labels: vec![],
            centroids: Default::default(),
            known: Default::default(),
            threshold: 0.6,
            next_id: 1,
            local_id: 0,
        };
        r.labels.push((0, "You".into()));
        r
    }

    /// Seed with remembered voices so a known person is labelled from their first sentence.
    pub fn preload(&mut self, voices: &[(i64, String, Vec<f32>)]) {
        for (pid, name, emb) in voices {
            let id = self.next_id;
            self.next_id += 1;
            let _ = self.manager.add(id.to_string(), &mut emb.clone());
            self.labels.push((id, name.clone()));
            self.known.insert(id, *pid);
        }
    }

    /// (speaker id, participant id if it came from a voiceprint, centroid, sample count)
    pub fn centroids(&self) -> Vec<(u32, Option<i64>, Vec<f32>, u32)> {
        self.centroids.iter().map(|(id, (c, n))| (*id, self.known.get(id).copied(), c.clone(), *n)).collect()
    }

    fn fold(&mut self, id: u32, e: &[f32]) {
        let (c, n) = self.centroids.entry(id).or_insert_with(|| (vec![0.0; e.len()], 0));
        *n += 1;
        for (a, b) in c.iter_mut().zip(e) {
            *a += (b - *a) / *n as f32;
        }
    }

    pub fn local(&self) -> SpeakerRef {
        SpeakerRef { id: self.local_id, label: self.label_of(self.local_id), confidence: 1.0 }
    }

    /// Cosine similarity two utterances need before they count as the same voice.
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// The speaker this embedding belongs to, or `None` for a voice we have not heard before.
    ///
    /// Deliberately read-only. Registering a voice is [`enroll`](Self::enroll), and the two are
    /// kept apart so the caller can wait until ASR has confirmed the audio contained words:
    /// everything that clears the VAD gets embedded — coughs, keyboard clatter, line noise —
    /// and none of it matches a real person, so matching and registering in one step minted a
    /// fresh "Speaker N" for every throat-clear.
    pub fn matching(&mut self, embedding: &[f32]) -> Option<SpeakerRef> {
        let id: u32 = self.manager.search(embedding, self.threshold)?.parse().unwrap_or(0);
        let known = self.known.contains_key(&id);
        Some(SpeakerRef { id, label: self.label_of(id), confidence: if known { 0.9 } else { self.threshold + 0.1 } })
    }

    /// Register a voice that matched nobody, and return the speaker it became.
    pub fn enroll(&mut self, embedding: &[f32]) -> SpeakerRef {
        let id = self.next_id;
        self.next_id += 1;
        let _ = self.manager.add(id.to_string(), &mut embedding.to_vec());
        self.fold(id, embedding);
        self.labels.push((id, format!("Speaker {id}")));
        SpeakerRef { id, label: self.label_of(id), confidence: 0.5 }
    }

    /// Fold another utterance into a known speaker, so our model of them sharpens over the call.
    pub fn reinforce(&mut self, id: u32, embedding: &[f32]) {
        let _ = self.manager.add(id.to_string(), &mut embedding.to_vec());
        self.fold(id, embedding);
    }

    pub fn rename(&mut self, id: u32, label: String) {
        if let Some(l) = self.labels.iter_mut().find(|(i, _)| *i == id) {
            l.1 = label;
        }
    }

    fn label_of(&self, id: u32) -> String {
        self.labels.iter().find(|(i, _)| *i == id).map(|l| l.1.clone()).unwrap_or_else(|| format!("Speaker {id}"))
    }
}

/// Cosine similarity, for comparing two voices the registry has not seen yet: until one of them
/// is enrolled there is no centroid to search against.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let (mut dot, mut na, mut nb) = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    dot / (na.sqrt() * nb.sqrt()).max(f32::EPSILON)
}
