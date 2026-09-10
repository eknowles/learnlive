//! NLLB-200 (distilled 600M, int8 ONNX) via ort. Any of 200 languages in any direction.
//!
//! Greedy decoding against the *plain* decoder. The merged (`_merged`) export declares every
//! `past_key_values.*` entry as a graph input and ORT requires all of them to be bound, so it
//! cannot be driven without maintaining a real KV cache. Re-running the full prefix each step is
//! fine at <=128 tokens; see ARCHITECTURE.md "known debt" for the cache work.

use std::path::Path;

use anyhow::{anyhow, Result};
use ndarray::{Array2, Array3, Axis};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use parking_lot::Mutex;
use tokenizers::Tokenizer;

use super::{languages, ort_err, Translator};

pub struct Nllb {
    encoder: Mutex<Session>,
    decoder: Mutex<Session>,
    tok: Tokenizer,
    max_new_tokens: usize,
}

impl Nllb {
    pub fn load(model_dir: &Path) -> Result<Self> {
        let dir = model_dir.join(crate::models::translate().dir);
        let build = |f: &str| -> Result<Session> {
            Session::builder()
                .map_err(ort_err)?
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(ort_err)?
                .with_intra_threads(4)
                .map_err(ort_err)?
                .commit_from_file(dir.join(f))
                .map_err(ort_err)
        };
        Ok(Self {
            encoder: Mutex::new(build("encoder_model_quantized.onnx")?),
            decoder: Mutex::new(build("decoder_model_quantized.onnx")?),
            tok: Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| anyhow!("{e}"))?,
            max_new_tokens: 128,
        })
    }

    fn lang_token(&self, code: &str) -> Result<u32> {
        let nllb = languages::by_code(code).map(|l| l.nllb).ok_or_else(|| anyhow!("unsupported language {code}"))?;
        self.tok.token_to_id(nllb).ok_or_else(|| anyhow!("tokenizer lacks {nllb}"))
    }
}

/// Split on terminal punctuation followed by whitespace.
///
/// NLLB-200 is trained on single sentences: hand it two and it translates the first, emits EOS
/// and silently drops the rest. Splitting is the difference between losing half of what somebody
/// said and not. Abbreviations ("Dr. Smith") do get split, which costs a little fluency — much
/// cheaper than dropping a clause.
fn split_sentences(text: &str) -> Vec<&str> {
    const ENDERS: [char; 7] = ['.', '!', '?', '\u{3002}', '\u{ff01}', '\u{ff1f}', '\u{2026}'];
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if !ENDERS.contains(&c) {
            continue;
        }
        // Swallow a run of terminal marks so "?!" and "..." stay with their sentence.
        let mut end = i + c.len_utf8();
        while let Some(&(j, n)) = chars.peek() {
            if ENDERS.contains(&n) {
                end = j + n.len_utf8();
                chars.next();
            } else {
                break;
            }
        }
        // Only a boundary if whitespace (or the end of the string) follows.
        if text[end..].is_empty() || text[end..].starts_with(char::is_whitespace) {
            let s = text[start..end].trim();
            if !s.is_empty() {
                out.push(s);
            }
            start = end;
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        out.push(tail);
    }
    if out.is_empty() {
        out.push(text.trim());
    }
    out
}

impl Translator for Nllb {
    fn translate(&self, text: &str, from: &str, to: &str) -> Result<String> {
        if from == to || text.trim().is_empty() {
            return Ok(text.to_string());
        }
        let sentences = split_sentences(text);
        if sentences.len() > 1 {
            let mut parts = Vec::with_capacity(sentences.len());
            for s in sentences {
                parts.push(self.translate_one(s, from, to)?);
            }
            return Ok(parts.join(" "));
        }
        self.translate_one(text.trim(), from, to)
    }
}

impl Nllb {
    fn translate_one(&self, text: &str, from: &str, to: &str) -> Result<String> {
        let src_lang = self.lang_token(from)?;
        let tgt_lang = self.lang_token(to)?;
        let eos = self.tok.token_to_id("</s>").ok_or_else(|| anyhow!("no </s>"))?;
        let pad = self.tok.token_to_id("<pad>").unwrap_or(1);

        // NLLB input format: [src_lang] tokens </s>
        let enc = self.tok.encode(text, false).map_err(|e| anyhow!("{e}"))?;
        let mut ids: Vec<i64> = vec![src_lang as i64];
        ids.extend(enc.get_ids().iter().map(|&i| i as i64));
        ids.push(eos as i64);
        let len = ids.len();
        let input_ids = Array2::from_shape_vec((1, len), ids)?;
        let attn = Array2::<i64>::ones((1, len));

        let enc_out: Array3<f32> = {
            let mut s = self.encoder.lock();
            let out = s.run(ort::inputs![
                "input_ids" => Tensor::from_array(input_ids)?,
                "attention_mask" => Tensor::from_array(attn.clone())?
            ])?;
            out["last_hidden_state"].try_extract_array::<f32>()?.to_owned().into_dimensionality()?
        };

        // Decoder: starts with </s> [tgt_lang]; greedy loop. (No KV cache reuse here for simplicity —
        // re-running the full prefix each step is fine at <=128 tokens on Apple Silicon.)
        let mut out_ids: Vec<i64> = vec![eos as i64, tgt_lang as i64];
        let mut dec = self.decoder.lock();
        for _ in 0..self.max_new_tokens {
            let dec_ids = Array2::from_shape_vec((1, out_ids.len()), out_ids.clone())?;
            let res = dec.run(ort::inputs![
                "input_ids" => Tensor::from_array(dec_ids)?,
                "encoder_attention_mask" => Tensor::from_array(attn.clone())?,
                "encoder_hidden_states" => Tensor::from_array(enc_out.clone())?
            ])?;
            let logits = res["logits"].try_extract_array::<f32>()?;
            let last = logits.index_axis(Axis(1), out_ids.len() - 1);
            let next = last.iter().enumerate().fold((0usize, f32::MIN), |b, (i, &v)| if v > b.1 { (i, v) } else { b }).0
                as i64;
            if next == eos as i64 || next == pad as i64 {
                break;
            }
            out_ids.push(next);
        }
        let toks: Vec<u32> = out_ids[2..].iter().map(|&i| i as u32).collect();
        Ok(self.tok.decode(&toks, true).map_err(|e| anyhow!("{e}"))?.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::split_sentences;

    #[test]
    fn splits_only_at_real_boundaries() {
        assert_eq!(split_sentences("One sentence only"), ["One sentence only"]);
        assert_eq!(split_sentences("First. Second."), ["First.", "Second."]);
        assert_eq!(split_sentences("Really?! Yes... Fine."), ["Really?!", "Yes...", "Fine."]);
        // No whitespace after the mark: a decimal or an ellipsis mid-word is not a boundary.
        assert_eq!(split_sentences("It cost 3.50 today."), ["It cost 3.50 today."]);
        assert_eq!(split_sentences("a.b.c"), ["a.b.c"]);
        // CJK terminals, and text with no terminal punctuation at all.
        assert_eq!(split_sentences("你好。 再见。"), ["你好。", "再见。"]);
        assert_eq!(split_sentences("никаких точек"), ["никаких точек"]);
        assert_eq!(split_sentences("   "), ["   ".trim()]);
    }

    #[test]
    fn nothing_is_dropped() {
        // The whole point: every non-space character survives the round trip.
        for t in ["A. B. C.", "Привет, как дела? Хорошо!", "One sentence only", "Trailing space. "] {
            let joined: String = split_sentences(t).concat();
            let strip = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
            assert_eq!(strip(&joined), strip(t), "content lost splitting {t:?}");
        }
    }
}
