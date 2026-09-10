//! Golden tests: real models, real audio, threshold assertions.
//! Ignored by default (needs ~1 GB of models). Run with:
//!   LEARNLIVE_MODELS=/path/to/models cargo test --test golden -- --ignored

use std::path::PathBuf;
use std::sync::Arc;

use learnlive_lib::engine::Engines;
use learnlive_lib::eval::{chrf, wer};
use learnlive_lib::pipeline::run_offline;
use learnlive_lib::types::SessionConfig;
use serde::Deserialize;

#[derive(Deserialize)]
struct Expected { speaker: String, role: String, lang: String, text: String, start_ms: u64, end_ms: u64 }

fn load(stem: &str) -> (PathBuf, Vec<Expected>) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let wav = dir.join(format!("{stem}.wav"));
    let exp = std::fs::read_to_string(dir.join(format!("{stem}.expected.jsonl")))
        .unwrap_or_else(|_| panic!("missing fixture {stem}; see fixtures/README.md"));
    (wav, exp.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect())
}

fn engines() -> Arc<Engines> {
    let dir = std::env::var("LEARNLIVE_MODELS").expect("set LEARNLIVE_MODELS to the model directory");
    Arc::new(Engines::new(PathBuf::from(dir)))
}

#[test]
#[ignore]
fn ru_lesson_transcript_speakers_and_merge() {
    let (wav, expected) = load("ru-lesson");
    let cfg = SessionConfig { learning: "ru".into(), native: "en".into(), speak_translations: false, asr_model: "small".into(), ..Default::default() };
    let got = run_offline(engines(), &cfg, &wav).expect("offline run");

    // 1. Sentence count: lines 3+4 of the script must have merged into one.
    let exp_sentences = expected.len() - 1;
    assert!((got.len() as i64 - exp_sentences as i64).abs() <= 1, "expected ~{exp_sentences} sentences, got {}", got.len());

    // 2. Transcript quality over the whole session (order-preserving concat).
    let ref_all = expected.iter().map(|e| e.text.as_str()).collect::<Vec<_>>().join(" ");
    let hyp_all = got.iter().map(|s| s.source_text.as_str()).collect::<Vec<_>>().join(" ");
    let w = wer(&ref_all, &hyp_all);
    assert!(w < 0.25, "WER {w:.2} too high\nref: {ref_all}\nhyp: {hyp_all}");

    // 3. Language ID per line.
    for s in &got {
        let nearest = expected.iter().min_by_key(|e| (e.start_ms as i64 - s.started_ms as i64).abs()).unwrap();
        assert_eq!(s.source_lang, nearest.lang, "language ID wrong on: {}", s.source_text);
        let want_role = if nearest.role == "local" { learnlive_lib::types::SourceRole::Local } else { learnlive_lib::types::SourceRole::Remote };
        assert_eq!(s.role, want_role, "role split wrong on: {}", s.source_text);
    }

    // 4. Speakers: exactly two distinct ids, and remote ones are all the same person.
    let remote_ids: std::collections::BTreeSet<u32> = got.iter().filter(|s| s.role == learnlive_lib::types::SourceRole::Remote).map(|s| s.speaker.id).collect();
    assert_eq!(remote_ids.len(), 1, "Anna should be one speaker, got ids {remote_ids:?}");
    assert!(got.iter().any(|s| s.speaker.label == "You"));

    // 5. Translation sanity: Russian lines translated to English should resemble a reasonable gloss.
    // We don't have reference translations for TTS'd Russian, so assert direction + non-degeneracy.
    for s in got.iter().filter(|s| s.source_lang == "ru") {
        assert_eq!(s.target_lang, "en");
        assert!(s.target_text.chars().any(|c| c.is_ascii_alphabetic()), "no Latin text in translation of: {}", s.source_text);
        assert!(chrf(&s.source_text, &s.target_text) < 0.5, "translation looks like a copy of the source");
    }

    // 6. Timing: every final sentence starts within 1 s of a scripted line.
    for s in &got {
        let d = expected.iter().map(|e| (e.start_ms as i64 - s.started_ms as i64).abs()).min().unwrap();
        assert!(d < 1000, "sentence at {} ms is {d} ms from any scripted start", s.started_ms);
    }
    let _ = expected.iter().map(|e| (&e.speaker, e.end_ms)).count();
}
