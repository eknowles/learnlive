//! Generate a test fixture from a script using the Piper voices already in the model dir:
//! deterministic audio with known text, speakers and timings — no real call needed.
//!
//!   cargo run --bin fixture-gen -- --models ~/Library/Application\ Support/dev.eknowles.learnlive/models \
//!       fixtures/ru-lesson.script.json fixtures/ru-lesson
//!
//! Writes <out>.wav (16 kHz, mono or stereo) and <out>.expected.jsonl.
//! Script format:  [{"speaker":"Anna","voice":"ru","role":"remote","text":"Привет!","pause_ms":800}, ...]
//! With any "local" lines the WAV is stereo: ch0 = local (you), ch1 = remote.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use learnlive_lib::audio::{resample::ToPipelineRate, write_wav16};
use learnlive_lib::engine::{tts::Piper, Synthesizer};

#[derive(Deserialize)]
struct Line {
    speaker: String,
    voice: String,
    #[serde(default = "remote")]
    role: String,
    text: String,
    #[serde(default = "gap")]
    pause_ms: u64,
}
fn remote() -> String {
    "remote".into()
}
fn gap() -> u64 {
    900
}

#[derive(Serialize)]
struct Expected {
    speaker: String,
    role: String,
    lang: String,
    text: String,
    start_ms: u64,
    end_ms: u64,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mi = args.iter().position(|a| a == "--models").ok_or_else(|| anyhow!("--models <dir> required"))?;
    let models = PathBuf::from(&args[mi + 1]);
    let rest: Vec<&String> =
        args.iter().enumerate().filter(|(i, _)| *i != 0 && *i != mi && *i != mi + 1).map(|(_, a)| a).collect();
    let (script, out) =
        (rest.first().ok_or_else(|| anyhow!("script path"))?, rest.get(1).ok_or_else(|| anyhow!("out stem"))?);
    let lines: Vec<Line> = serde_json::from_str(&std::fs::read_to_string(script)?)?;

    let stereo = lines.iter().any(|l| l.role == "local");
    let mut voices = std::collections::HashMap::<String, Piper>::new();
    let mut lanes: [Vec<f32>; 2] = [vec![], vec![]];
    let mut expected = vec![];
    let mut clock_ms = 500u64;
    lanes[0].resize(8000, 0.0);
    lanes[1].resize(8000, 0.0);

    for l in &lines {
        if !voices.contains_key(&l.voice) {
            let v = Piper::load(&models, &l.voice)?
                .with_context(|| format!("no Piper voice for '{}' in {}", l.voice, models.display()))?;
            voices.insert(l.voice.clone(), v);
        }
        let (pcm, rate) = voices[&l.voice].synthesize(&l.text)?;
        let mut rs = ToPipelineRate::new(rate, 1)?;
        let pcm16 = rs.push(&pcm)?;
        let lane = if l.role == "local" { 0 } else { 1 };
        let start = lanes[lane].len().max(lanes[1 - lane].len());
        for x in lanes.iter_mut() {
            x.resize(start, 0.0);
        }
        lanes[lane].extend_from_slice(&pcm16);
        lanes[1 - lane].resize(lanes[lane].len(), 0.0);
        let dur_ms = pcm16.len() as u64 * 1000 / 16_000;
        expected.push(Expected {
            speaker: l.speaker.clone(),
            role: l.role.clone(),
            lang: l.voice.clone(),
            text: l.text.clone(),
            start_ms: clock_ms,
            end_ms: clock_ms + dur_ms,
        });
        clock_ms += dur_ms + l.pause_ms;
        let pad = (l.pause_ms * 16) as usize;
        for x in lanes.iter_mut() {
            let n = x.len();
            x.resize(n + pad, 0.0);
        }
    }

    let n = lanes[0].len().max(lanes[1].len());
    for x in lanes.iter_mut() {
        x.resize(n, 0.0);
    }
    let wav = PathBuf::from(format!("{out}.wav"));
    if stereo {
        let inter: Vec<f32> = (0..n).flat_map(|i| [lanes[0][i], lanes[1][i]]).collect();
        write_wav16(&wav, 16_000, 2, &inter)?;
    } else {
        write_wav16(&wav, 16_000, 1, &lanes[1])?;
    }
    let jsonl: String = expected.iter().map(|e| serde_json::to_string(e).unwrap()).collect::<Vec<_>>().join("\n");
    std::fs::write(format!("{out}.expected.jsonl"), jsonl + "\n")?;
    eprintln!("wrote {} ({} lines, {:.1} s)", wav.display(), expected.len(), n as f32 / 16_000.0);
    Ok(())
}
