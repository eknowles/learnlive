//! Supported languages and the code mappings each engine needs.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Language {
    pub code: &'static str, // ISO-639-1 used across the app + Whisper
    pub name: &'static str,
    pub native: &'static str,
    pub nllb: &'static str, // NLLB-200 / FLORES code
    pub tts: bool,          // do we ship a Piper voice
    pub grammar: bool,      // does the UD POS model cover it
}

pub const LANGUAGES: &[Language] = &[
    Language { code: "en", name: "English", native: "English", nllb: "eng_Latn", tts: true, grammar: true },
    Language { code: "ru", name: "Russian", native: "Русский", nllb: "rus_Cyrl", tts: true, grammar: true },
    Language {
        code: "uk", name: "Ukrainian", native: "Українська", nllb: "ukr_Cyrl", tts: true, grammar: true
    },
    Language { code: "es", name: "Spanish", native: "Español", nllb: "spa_Latn", tts: true, grammar: true },
    Language { code: "fr", name: "French", native: "Français", nllb: "fra_Latn", tts: true, grammar: true },
    Language { code: "de", name: "German", native: "Deutsch", nllb: "deu_Latn", tts: true, grammar: true },
    Language { code: "it", name: "Italian", native: "Italiano", nllb: "ita_Latn", tts: true, grammar: true },
    Language { code: "pt", name: "Portuguese", native: "Português", nllb: "por_Latn", tts: true, grammar: true },
    Language { code: "pl", name: "Polish", native: "Polski", nllb: "pol_Latn", tts: true, grammar: true },
    Language { code: "tr", name: "Turkish", native: "Türkçe", nllb: "tur_Latn", tts: true, grammar: true },
    Language { code: "ar", name: "Arabic", native: "العربية", nllb: "arb_Arab", tts: true, grammar: true },
    Language { code: "zh", name: "Chinese", native: "中文", nllb: "zho_Hans", tts: true, grammar: true },
    Language { code: "ja", name: "Japanese", native: "日本語", nllb: "jpn_Jpan", tts: false, grammar: true },
    Language { code: "ko", name: "Korean", native: "한국어", nllb: "kor_Hang", tts: false, grammar: true },
    Language { code: "hi", name: "Hindi", native: "हिन्दी", nllb: "hin_Deva", tts: false, grammar: true },
    Language { code: "nl", name: "Dutch", native: "Nederlands", nllb: "nld_Latn", tts: false, grammar: true },
];

pub fn by_code(code: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|l| l.code == code)
}
