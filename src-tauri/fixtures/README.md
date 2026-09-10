# Fixtures

`*.script.json` are checked in; the generated `*.wav` / `*.expected.jsonl` are not (Git LFS or CI cache).
Regenerate with the Piper voices in your model dir:

    cargo run --bin fixture-gen -- --models "$MODEL_DIR" fixtures/ru-lesson.script.json fixtures/ru-lesson

Lines 3–4 are one sentence split by a short pause — that's the revision/merge case.
Anna and Ed use different Piper voices, so speaker separation is testable without real people.

For a real recording: export from Meet/Zoom as WAV (mono → everything is "remote"), or record
yourself and BlackHole as two channels (ch0 = you, ch1 = call) to test the role split too.
Write the `.expected.jsonl` by hand: `{"speaker","role","lang","text","start_ms","end_ms"}` per line.
