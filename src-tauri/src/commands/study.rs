//! Word lookups for the reading surface.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::engine::{translate::Nllb, Translator};
use crate::error::{AppError, CmdResult};
use crate::AppState;

/// One word, glossed.
#[derive(Clone, Serialize)]
pub struct WordGloss {
    pub word: String,
    pub translation: String,
    /// Byte range of this word's counterpart in the other language's sentence.
    ///
    /// Always `None` today: `grammar.rs` never populated `aligned_to` and no aligner exists, so
    /// the popover falls back to translating the word on its own. The field is here — and the
    /// UI already reads it — so that a real aligner slots in behind this command without the
    /// reading surface changing at all.
    pub aligned_to: Option<(usize, usize)>,
}

/// Translate a single word for the hover popover.
///
/// A word out of context is a worse translation than the sentence it came from, which is why
/// `sentence` is taken now even though only an aligner will read it. The results are cached:
/// hovering the same word twice in a call must not cost two decodes.
#[tauri::command]
pub async fn translate_word(
    state: State<'_, AppState>,
    word: String,
    sentence: String,
    from: String,
    to: String,
) -> CmdResult<WordGloss> {
    // Reserved for the aligner — placing a word needs the sentence it came from. Taking it now
    // means the aligner will not change this command's shape.
    let _ = &sentence;

    let word = word.trim().to_string();
    if word.is_empty() {
        return Err(AppError::invalid("No word to look up"));
    }
    if from == to {
        return Ok(WordGloss { translation: word.clone(), word, aligned_to: None });
    }

    let key = format!("{from}|{to}|{}", word.to_lowercase());
    if let Some(hit) = state.glossary.lock().get(&key).cloned() {
        return Ok(WordGloss { word, translation: hit, aligned_to: None });
    }

    let engines = state.engines.clone();
    let (w, f, t) = (word.clone(), from.clone(), to.clone());
    let translation = tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<String> {
        // The session may not have loaded anything yet — the reading surface works on history
        // too, with no models warm.
        let translator: Arc<dyn Translator> = match engines.translator.lock().clone() {
            Some(t) => t,
            None => {
                let loaded: Arc<dyn Translator> = Arc::new(Nllb::load(&engines.model_dir)?);
                *engines.translator.lock() = Some(loaded.clone());
                loaded
            }
        };
        translator.translate(&w, &f, &t)
    })
    .await?
    .map_err(|e| AppError::models(format!("{e:#}")))?;

    state.glossary.lock().insert(key, translation.clone());
    Ok(WordGloss { word, translation, aligned_to: None })
}
