//! NLLB-200 (distilled 600M, int8 ONNX) via ort. Any of 200 languages in any direction.
//! Greedy decoding with the merged decoder (KV cache) — good enough for conversational sentences.

use std::path::Path;

use anyhow::{anyhow, Result};
use ndarray::{Array2, Array3, Axis};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use parking_lot::Mutex;
use tokenizers::Tokenizer;

use super::{languages, Translator};

pub struct Nllb {
    encoder: Mutex<Session>,
    decoder: Mutex<Session>,
    tok: Tokenizer,
    max_new_tokens: usize,
}

impl Nllb {
    pub fn load(model_dir: &Path) -> Result<Self> {
        let dir = model_dir.join(crate::models::TRANSLATE.dir);
        let build = |f: &str| -> Result<Session> {
            Ok(Session::builder()?
                .with_optimization_level(GraphOptimizationLevel::Level3)?
                .with_intra_threads(4)?
                .commit_from_file(dir.join(f))?)
        };
        Ok(Self {
            encoder: Mutex::new(build("encoder_model_quantized.onnx")?),
            decoder: Mutex::new(build("decoder_model_merged_quantized.onnx")?),
            tok: Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| anyhow!("{e}"))?,
            max_new_tokens: 128,
        })
    }

    fn lang_token(&self, code: &str) -> Result<u32> {
        let nllb = languages::by_code(code).map(|l| l.nllb).ok_or_else(|| anyhow!("unsupported language {code}"))?;
        self.tok.token_to_id(nllb).ok_or_else(|| anyhow!("tokenizer lacks {nllb}"))
    }
}

impl Translator for Nllb {
    fn translate(&self, text: &str, from: &str, to: &str) -> Result<String> {
        if from == to || text.trim().is_empty() {
            return Ok(text.to_string());
        }
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
            ]?)?;
            out["last_hidden_state"].try_extract_tensor::<f32>()?.to_owned().into_dimensionality()?
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
                "encoder_hidden_states" => Tensor::from_array(enc_out.clone())?,
                "use_cache_branch" => Tensor::from_array(ndarray::arr1(&[false]))?
            ]?)?;
            let logits = res["logits"].try_extract_tensor::<f32>()?;
            let last = logits.index_axis(Axis(1), out_ids.len() - 1);
            let next = last.iter().enumerate().fold((0usize, f32::MIN), |b, (i, &v)| if v > b.1 { (i, v) } else { b }).0 as i64;
            if next == eos as i64 || next == pad as i64 { break; }
            out_ids.push(next);
        }
        let toks: Vec<u32> = out_ids[2..].iter().map(|&i| i as u32).collect();
        Ok(self.tok.decode(&toks, true).map_err(|e| anyhow!("{e}"))?.trim().to_string())
    }
}
