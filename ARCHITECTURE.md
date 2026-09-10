# Architecture

Read this before adding a feature. It says where things live and, more importantly, where they *don't*.

## Layers

```
UI (React)            src/            talks to the backend only through src/lib/api.ts
─────────────────────────────────────────────────────────────────────────────────────────
Commands (Tauri)      src-tauri/src/commands/   thin: validate → call crate → map errors
─────────────────────────────────────────────────────────────────────────────────────────
Crate                 audio/  engine/  pipeline.rs  db.rs  calendar.rs  models.rs
```

Rules that keep this maintainable:

- **Commands hold no logic.** If a command is more than ~15 lines, the logic belongs in `db`, `pipeline`, `calendar` or `models`. That way the offline runner, tests and future CLIs get the same behaviour.
- **Engines are traits** (`engine/mod.rs`: `Transcriber`, `Translator`, `GrammarAnalyzer`, `SpeakerEmbedder`, `Synthesizer`). The pipeline only sees `engine::Loaded`, a snapshot of `Arc<dyn Trait>`s. Tests build `Loaded` from fakes; nothing in `pipeline.rs` knows about ONNX.
- **The pipeline emits through `EventSink`**, never through Tauri directly. `AppHandle` implements it; `CollectSink` is for tests; `DbSink` persists; `MultiSink` fans out.
- **Errors cross the boundary as `error::AppError { kind, message }`.** Use `anyhow` inside the crate; commands return `CmdResult<T>` and the `From` impls classify. The UI maps `kind` to wording in `src/lib/errors.ts`.
- **One source of truth per list:** languages in `engine/languages.rs`; models in `models.rs::required()`; POS colours in `src/lib/pos.ts`; session defaults in `src/lib/config.ts`.

## Data flow

```
cpal / file:  ─▶ audio::Mixer ─▶ 20 ms frames @16 kHz ─▶ engine::vad::Segmenter ─▶ Utterance
                                                                                   │
                       pipeline::Sentencer.push(Job{utt, role})  ◀─────────────────┘
                         ├─ speaker: local mic ⇒ "You"; else embed + SpeakerRegistry.identify
                         ├─ continues open sentence?  (SentenceOptions: gap / punctuation / cap)
                         ├─ build(): ASR on merged audio → translate → grammar tokens
                         └─ sink.segment(Segment{ id, final:false, revision })  … then final:true
                                   │
                          AppHandle (UI event)  +  DbSink (SQLite)  +  Voice (TTS, ducked)
```

Threads in a live session: **mixer** (owns the cpal input streams, sums to 20 ms frames), **vad**
(frames → utterances), **ml** (utterances → sentences), **voice** (TTS + ducking), **out** (owns
the cpal output stream), plus cpal's own callback threads. They talk over bounded crossbeam
channels; the ML thread dropping an utterance under load is logged, never fatal.

`cpal::Stream` is `!Send + !Sync`, so it can never be reachable from `AppState` (Tauri requires
managed state to be `Send + Sync`, and every command would stop compiling). The rule: **a cpal
stream is created on, and owned for the life of, the thread that uses it** — `audio::mixer` for
inputs, `audio::playback` for the output. What crosses a thread boundary is a channel endpoint or
an `Arc<Mutex<VecDeque<f32>>>`, never a stream.

## Recipes

### Add a language
1. Add a row to `engine/languages.rs::LANGUAGES` (ISO‑639‑1, NLLB code).
2. If a Piper voice exists, add it to `models.rs::tts()`. `list_languages` reports `tts: true` automatically.
3. Optional: lemma/feature heuristics in `engine/grammar.rs::lemma_hint` / `feats_hint`.
That's it — translation, POS tagging and language ID already cover it.

### Swap or add an engine (e.g. a better translator)
1. Implement the trait in a new file under `engine/`.
2. Construct it in `Engines::warm()`; add its files to `models.rs` and to `required()`.
3. No pipeline or UI changes. Run `just golden` to compare quality.

### Add a command
1. Put it in the `commands/*.rs` file for its concern (or add a file).
2. Register it in `commands/mod.rs::all_commands!`.
3. Add the typed wrapper in `src/lib/api.ts`. Components call `api.*`, never `invoke`.

### Add a UI panel
Components under `src/components/` are presentational: props in, `api.*` calls out. Session state lives in `hooks/useSession.ts`, transcript state in `hooks/useTranscript.ts`. New cross‑cutting state → a new hook, not more `useState` in `App.tsx`.

### Add a persisted field
1. Extend `types::Segment` / `MeetingSummary` (Rust) and `src/lib/types.ts` (TS) together.
2. Add a migration step in `db.rs` (append to `SCHEMA`; SQLite `CREATE … IF NOT EXISTS` is idempotent, use `ALTER TABLE` guarded by a `PRAGMA user_version` bump for real migrations).
3. Update `row_to_segment` / `insert_segment`, and the `db.rs` tests.

### Make it feel faster
Latency is dominated by two things, in this order:
1. **When the VAD closes an utterance** — nothing reaches the screen before that. `SegmenterOptions`
   (`engine/vad.rs`): `min_silence_ms` is the floor on "how long after I stop talking", and
   `max_speech_ms` is the worst case if you never pause.
2. **Translation.** NLLB is ~10x the cost of Whisper-tiny per utterance (measure it: the bench
   reports per-stage totals). ASR runs at ~0.04x real time; MT at ~0.4x.

Change a default only with a `just bench-sweep` before/after in the commit message.

### Tune sentence revision
`pipeline::SentenceOptions`. Unit tests in `pipeline.rs` document the intended behaviour; change a default only with a test that shows why.

## Testing map

| What | Where | Needs |
|---|---|---|
| Mixer maths, resampler | `audio/*.rs` tests | – |
| Sentence revision rules | `pipeline.rs` tests (fake `Loaded`) | – |
| SQLite, FTS, voiceprints | `db.rs` tests (in‑memory) | – |
| WER / chrF | `eval.rs` tests | – |
| Token diff, time helpers | `src/__tests__` (vitest) | – |
| End‑to‑end on audio | `tests/golden.rs` | models + fixtures |
| Latency (audio → words) | `just bench <wav>` | models |

## Known debt (in order of pain)

1. **The grammar/POS feature is off.** `models.rs::pos()` points at
   `wietsedv/xlm-roberta-base-ft-udpos28-all`, which does not exist — that family is published
   per language (`-en`, `-ru`, …) and as PyTorch weights only, and no multilingual UPOS model on
   the Hub has an ONNX export. So `pos()` is excluded from `required()`, the model is marked
   `optional`, `Engines::warm` logs and continues when it will not load, and segments carry no
   tokens (`Transcript.tsx` falls back to plain text). To turn it back on: export
   `wietsedv/xlm-roberta-base-ft-udpos28-<lang>` with `optimum` to quantized ONNX, host it,
   make `pos()` take a language, and put it back in `required()`. `list_languages` already
   reports `grammar` from what is on disk, so a hand-placed tagger re-enables the cards.
2. Lemmas and case/tense are heuristics (`grammar.rs`). Replace with a UD parser export; the `Token` shape already has room.
3. **sherpa-onnx aborts the process on bad input** — `exit(-1)`, not an error you can catch.
   Whisper does it for a language outside the model's table (`"auto"` is *not* a code; empty
   means auto-detect), and there are similar paths in the VAD and speaker models. Validate
   before handing anything to sherpa; see `asr.rs::pin_language`.
4. NLLB-200 is a sentence-level model: given two sentences it translates the first, emits EOS and
   drops the rest. `translate.rs::split_sentences` splits before translating, at the cost of
   splitting abbreviations too.
5. NLLB decoding has no KV cache (re-runs the prefix each step), so it uses the plain
   `decoder_model_quantized.onnx` rather than the `_merged` export — the merged graph declares
   every `past_key_values.*` as a required input and cannot be driven without a real cache.
   Fine for sentences, slow for monologues.
6. Pressing Stop drops whatever the VAD is still holding. `run_offline` calls
   `Segmenter::flush`; the live session does not, because the ML thread is already exiting.
7. Clips are raw 16 kHz WAV (~2 MB/min). Opus + a retention setting would be sensible before history grows.
8. Calendar is macOS‑only. `calendar.rs` has the stub for a Google Calendar route.
