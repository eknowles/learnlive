# LearnLive

Live translation, speaker detection and grammar coaching for video calls — one binary, fully offline.

Join a Google Meet / Zoom / Teams call, press **Start listening**, and every utterance shows up as:

- who said it (speakers are told apart by voice; your own mic is always "You")
- the sentence in the language you're learning, in large serif type, with each word underlined by part of speech
- the gloss in your native language underneath
- hover any word for lemma, part of speech and case/tense; **Hear** / **Slower** / **Replay original** per line
- optionally, the translation read aloud while the call audio is ducked underneath it

Works in either direction: speech in your learning language is translated *to* your native language so you can follow; anything else (including what you say) is translated *into* the learning language so you see how you could have said it.

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

Prereqs: Rust stable, Node 18+, and on macOS Xcode command line tools. `cargo install tauri-cli --version "^2"`.

```bash
npm install
npm run tauri dev      # dev with hot reload
npm run tauri build    # → src-tauri/target/release/bundle/{macos,dmg}/LearnLive.app
```

First `cargo build` pulls prebuilt sherpa-onnx and ONNX Runtime binaries (the `download-binaries` features), so it needs network once.

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
  components/Transcript.tsx  the reading surface
  components/Word.tsx        hover card + pronunciation
  lib/api.ts                 typed wrapper over Tauri commands/events
src-tauri/src/
  audio/                     capture, mixer, resample, playback
  engine/                    asr, diarize, translate, grammar, tts, vad, languages
  pipeline.rs                session orchestration (audio thread + ML thread)
  models.rs                  manifest + downloader
  commands.rs                Tauri command surface
```

## Status / roadmap

This is a first end-to-end cut and has **not yet been compiled on a Mac** — expect to iterate on `sherpa-rs` / `ort` API names for the exact crate versions you land on. Things I'd do next, in order:

1. Compile, fix bindings, smoke-test with a YouTube video routed through BlackHole.
2. Streaming partials (`Partial` type exists, not wired) so text appears while someone is still talking.
3. Real morphology: lemmas/case/tense are heuristic today (`grammar.rs`). Swap in a UD parser ONNX export (e.g. Trankit/Stanza) or per-language analysers.
4. KV-cache in the NLLB decoder for faster long sentences; beam search for quality.
5. Word alignment (source ↔ translation) so hovering a translated word highlights the original.
6. Save sessions; export flashcards (Anki) from clicked words.
7. Per-speaker language memory (skip language ID once a speaker's language is known).

## License

MIT
