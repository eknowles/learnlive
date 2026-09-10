//! Universal Dependencies POS tagging with XLM-R (token classification, ~30 languages).
//! Lemmas + morphological features come from a light rule layer per language; a full UD parser
//! is on the roadmap (see README) — POS alone already drives the colour-coded hover UI.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use ndarray::{Array2, Axis};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use parking_lot::Mutex;
use tokenizers::Tokenizer;

use super::GrammarAnalyzer;
use crate::types::Token;

pub struct UdPos {
    session: Mutex<Session>,
    tok: Tokenizer,
    labels: Vec<String>,
}

impl UdPos {
    pub fn load(model_dir: &Path) -> Result<Self> {
        let dir = model_dir.join(crate::models::pos().dir);
        let cfg: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("config.json"))?)?;
        let id2label = cfg["id2label"].as_object().ok_or_else(|| anyhow!("config.json missing id2label"))?;
        let mut labels = vec![String::new(); id2label.len()];
        for (k, v) in id2label { labels[k.parse::<usize>()?] = v.as_str().unwrap_or("X").to_string(); }
        Ok(Self {
            session: Mutex::new(Session::builder()?.with_optimization_level(GraphOptimizationLevel::Level3)?.commit_from_file(dir.join("model_quantized.onnx"))?),
            tok: Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| anyhow!("{e}"))?,
            labels,
        })
    }
}

impl GrammarAnalyzer for UdPos {
    fn analyze(&self, text: &str, lang: &str) -> Result<Vec<Token>> {
        let enc = self.tok.encode(text, true).map_err(|e| anyhow!("{e}"))?;
        let ids: Vec<i64> = enc.get_ids().iter().map(|&i| i as i64).collect();
        let n = ids.len();
        let out = {
            let mut s = self.session.lock();
            let r = s.run(ort::inputs![
                "input_ids" => Tensor::from_array(Array2::from_shape_vec((1, n), ids)?)?,
                "attention_mask" => Tensor::from_array(Array2::<i64>::ones((1, n)))?
            ]?)?;
            r["logits"].try_extract_tensor::<f32>()?.to_owned()
        };

        // Collapse sub-word pieces to words: first piece's tag wins.
        let mut tokens: Vec<Token> = vec![];
        let mut last_word: Option<u32> = None;
        for (i, wid) in enc.get_word_ids().iter().enumerate() {
            let Some(w) = wid else { continue };
            if last_word == Some(*w) { continue; }
            last_word = Some(*w);
            let (start, end) = enc.get_offsets()[i];
            let (s, e) = enc.get_offsets().iter().zip(enc.get_word_ids()).filter(|(_, x)| **x == Some(*w))
                .fold((start, end), |(a, b), ((s, e), _)| (a.min(*s), b.max(*e)));
            let word = &text[s..e];
            let row = out.index_axis(Axis(0), 0);
            let row = row.index_axis(Axis(0), i);
            let best = row.iter().enumerate().fold((0, f32::MIN), |b, (j, &v)| if v > b.1 { (j, v) } else { b }).0;
            let pos = self.labels.get(best).cloned().unwrap_or_else(|| "X".into());
            tokens.push(Token {
                text: word.to_string(),
                lemma: lemma_hint(word, &pos, lang),
                pos,
                feats: feats_hint(word, lang),
                aligned_to: None,
            });
        }
        Ok(tokens)
    }
}

/// Cheap lemma heuristics until a real morphological analyser lands. Never wrong-looking:
/// falls back to the surface form in lowercase.
fn lemma_hint(word: &str, pos: &str, lang: &str) -> String {
    let w = word.to_lowercase();
    match (lang, pos) {
        ("ru", "NOUN") => strip_any(&w, &["ами", "ями", "ах", "ях", "ов", "ев", "ей", "ой", "ей", "ом", "ем", "ам", "ям", "ы", "и", "у", "ю", "а", "я", "е"]),
        ("ru", "VERB") => strip_any(&w, &["ешь", "ете", "ишь", "ите", "ет", "ит", "ем", "им", "ут", "ют", "ат", "ят", "ла", "ло", "ли", "л", "ю", "у"]),
        ("es", "VERB") | ("pt", "VERB") | ("it", "VERB") => strip_any(&w, &["amos", "emos", "imos", "aste", "iste", "as", "es", "an", "en", "a", "e", "o"]),
        ("en", "VERB") => strip_any(&w, &["ing", "ed", "es", "s"]),
        _ => w,
    }
}

fn strip_any(w: &str, suffixes: &[&str]) -> String {
    for s in suffixes {
        if w.len() > s.len() + 2 && w.ends_with(s) {
            return w[..w.len() - s.len()].to_string();
        }
    }
    w.to_string()
}

fn feats_hint(word: &str, lang: &str) -> BTreeMap<String, String> {
    let mut f = BTreeMap::new();
    let w = word.to_lowercase();
    if lang == "ru" {
        // Rough Russian case hints from endings — labelled "hint" in the UI.
        let case = if w.ends_with("ами") || w.ends_with("ями") || w.ends_with("ом") || w.ends_with("ем") || w.ends_with("ой") { Some("Ins") }
            else if w.ends_with("ах") || w.ends_with("ях") { Some("Loc") }
            else if w.ends_with("ов") || w.ends_with("ев") || w.ends_with("ей") { Some("Gen") }
            else if w.ends_with("ам") || w.ends_with("ям") { Some("Dat") }
            else if w.ends_with("у") || w.ends_with("ю") { Some("Acc") }
            else { None };
        if let Some(c) = case { f.insert("Case".into(), c.into()); f.insert("Confidence".into(), "hint".into()); }
    }
    f
}
