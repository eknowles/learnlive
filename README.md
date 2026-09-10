# LearnLive

Live translation, speaker detection and grammar coaching for video calls — one binary, fully offline.

Join a Google Meet / Zoom / Teams call, press **Start listening**, and every utterance shows up as:

- who said it (speakers are told apart by voice; your own mic is always "You")
- the sentence in the language you're learning, in large serif type, with each word underlined by part of speech
- the gloss in your native language underneath
- hover any word for lemma, part of speech and case/tense; **Hear** / **Slower** / **Replay original** per line
- optionally, the translation read aloud while the call audio is ducked underneath it
- lines are provisional while someone is still talking, then rewritten in place once the sentence lands — words that moved or changed form flash briefly, so free-word-order languages like Russian don't leave you with a wrong first draft
- each line shows arrival time and a live "3 min ago"; scrolling up pauses auto-follow, with a "N new · back to now" button to return

Works in either direction: speech in your learning language is translated *to* your native language so you can follow; anything else (including what you say) is translated *into* the learning language so you see how you could have said it.

## Meetings, people and history

- **Calendar link.** On Start, LearnLive looks at your calendar (macOS EventKit, read‑only, permission prompt on first use) for a meeting in progress or about to start and attaches the session to it. Attach later from the Meeting panel if it guessed wrong or you started early.
- **Speakers → people.** With an invite linked, each detected speaker gets a dropdown of attendees. Pick "Anna" and every line from that voice is relabelled, now and in history.
- **Remembered voices** (off by default, History → checkbox). When on, the voice of anyone you've *named* is averaged into a local voiceprint and they're recognised from their first sentence next time. Stored only in the local SQLite file; turning it off deletes all of them; per‑person forget is a command away.
- **History.** Every finalised sentence is written to `learnlive.sqlite` in the app data dir, with FTS5 search across all meetings in both languages ("книгу", "book", prefix*). Open any meeting and you get the same reading surface with hover cards and clip replay.

Tauri has no calendar plugin; `src-tauri/src/calendar.rs` is a small `objc2` bridge to EventKit and is macOS‑only for now (Windows/Linux would need the Google Calendar API).

## Languages

English, Russian, Ukrainian, Spanish, French, German, Italian, Portuguese, Polish, Turkish, Arabic, Chinese, Japanese, Korean, Hindi, Dutch. Any pair, any direction. Adding one is a row in `src-tauri/src/engine/languages.rs` (+ a Piper voice in `models.rs` if you want it spoken).

## How it's built

Everything is Rust inside a Tauri 2 app. No Python, no separate server.

```
cpal (BlackHole, mic, …)
  └─ audio::mixer      per-source gain / mute / duck, resample → 16 kHz mono, 20 ms frames
       └─ engine::vad        Silero VAD → utterances                       (sherpa-onnx)
            ├─ engine::asr        Whisper, multilingual + language ID       (sherpa-onnx)
            ├─ engine::diarize    ERes2Net speaker embeddings + online clustering (sherpa-onnx)
            ├─ engine::translate  NLLB-200 distilled 600M, int8             (ort / ONNX Runtime)
            ├─ engine::grammar    XLM-R Universal Dependencies POS tagger   (ort)
            └─ engine::tts        Piper voices, ducked over the call         (sherpa-onnx)
                 └─ Tauri events → React transcript
```

The binary is ~15 MB. Models (~1.2 GB for a typical setup, more for the large Whisper) are downloaded into the app data dir on first run for the languages you pick, with a progress list in the UI. Turn on the `bundled-models` cargo feature if you'd rather ship them inside the app.

## Build

```bash
devbox shell   # Rust, Node, just, ffmpeg from devbox.json
just setup     # npm ci + tauri CLI
just dev       # hot reload
just build     # → src-tauri/target/release/bundle/{macos,dmg}/
```

Without devbox: Rust stable, Node 20, `cargo install tauri-cli --version "^2"`, then the `npm run tauri …` scripts. See CONTRIBUTING.md for the release flow (conventional commits → release-please).

First `cargo build` pulls prebuilt sherpa-onnx and ONNX Runtime binaries (the `download-binaries` features), so it needs network once.

### How revision works

`pipeline.rs` keeps one *open* sentence per session. A new VAD chunk **continues** it when it's the same speaker, arrived within 1.5 s, the previous transcript didn't end in terminal punctuation, and the total is under 20 s. Continuation re-runs Whisper on the *merged audio* (not string-concat) and re-translates, emitting the same `id` with `final: false` and `revision + 1`. The sentence finalises on terminal punctuation, a speaker change, 1.8 s of idle, or the length cap; only then does TTS speak it and the clip get written. The UI upserts by id and diffs tokens against the previous revision.

### Getting call audio in (macOS)

1. Install [BlackHole 2ch](https://existential.audio/blackhole/).
2. Audio MIDI Setup → create a **Multi-Output Device** containing your headphones *and* BlackHole, set it as system output. You keep hearing the call; LearnLive captures the BlackHole leg.
3. In LearnLive, *Call audio* → BlackHole 2ch, *Your microphone* → your mic.

Windows: WASAPI loopback devices show up automatically. Linux: PulseAudio/PipeWire "Monitor of …" sources.

## Project layout

```
src/                       React UI (TypeScript)
  components/Setup.tsx       languages, devices, model download, start/stop
  components/Mixer.tsx       live per-source gain, mute, meters
  components/Speakers.tsx    detected speakers, rename
  components/Meeting.tsx     pick / show the linked calendar event
  components/History.tsx     past meetings, open, search, remember-voices toggle
  components/Transcript.tsx  the reading surface
  components/Word.tsx        hover card + pronunciation
  lib/api.ts                 typed wrapper over Tauri commands/events
src-tauri/src/
  audio/                     capture, mixer, resample, playback, file_source
  calendar.rs                EventKit bridge (macOS) — no Tauri plugin exists for calendars
  db.rs                      SQLite: meetings, participants, segments + FTS5, voiceprints
  engine/                    asr, diarize, translate, grammar, tts, vad, languages
  pipeline.rs                session orchestration (audio thread + ML thread)
  models.rs                  manifest + downloader
  commands.rs                Tauri command surface
```

## Testing

Three tiers, all in `.github/workflows/ci.yml` (macOS Apple Silicon runners):

| Tier | Command | Needs | When |
|---|---|---|---|
| Unit | `cargo test` + `npm test` | nothing | every push |
| Golden | `LEARNLIVE_MODELS=… cargo test --test golden -- --ignored` | models + fixtures | nightly, or PR label `golden` |
| Bundle | `npx tauri build --ci` | Rust + Node | every push |

**Unit** tests mock the engine traits and exercise the parts that are actually ours: mixer gain/mute/duck, resampler, the sentence-revision rules (`pipeline.rs` tests), the token diff and time helpers in the UI.

**Golden** tests run `pipeline::run_offline()` on a WAV and assert thresholds, not strings: WER < 0.25, correct language ID and you/them role per line, exactly one remote speaker id, the split-sentence fixture merged, timings within 1 s. Fixtures are *generated* from `fixtures/*.script.json` with the same Piper voices the app ships, so ground truth is exact and nothing private is in the repo:

```bash
cargo run --bin model-fetch -- --models .models --asr small --langs ru,en
cargo run --bin fixture-gen -- --models .models fixtures/ru-lesson.script.json fixtures/ru-lesson
LEARNLIVE_MODELS=.models cargo test --test golden -- --ignored --nocapture
```

**A real meeting recording works too.** Export it as WAV and write an `.expected.jsonl` by hand (see `fixtures/README.md`). Mono makes everything "remote"; a stereo file with you on the left and the call on the right also tests the role split. You can also run the *live* app against a file: pick a source id of `file:/path/to/call.wav` and it's fed in at real-time pace through the normal mixer.

## Status / roadmap

This is a first end-to-end cut and has **not yet been compiled on a Mac** — expect to iterate on `sherpa-rs` / `ort` API names for the exact crate versions you land on. Things I'd do next, in order:

1. Compile, fix bindings, smoke-test with a YouTube video routed through BlackHole.
2. Token-level streaming within an utterance (currently a line first appears when the VAD closes a chunk, ~1 s after a pause).
3. Real morphology: lemmas/case/tense are heuristic today (`grammar.rs`). Swap in a UD parser ONNX export (e.g. Trankit/Stanza) or per-language analysers.
4. KV-cache in the NLLB decoder for faster long sentences; beam search for quality.
5. Word alignment (source ↔ translation) so hovering a translated word highlights the original.
6. Save sessions; export flashcards (Anki) from clicked words.
7. Per-speaker language memory (skip language ID once a speaker's language is known).

## License

MIT
