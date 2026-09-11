# Changelog

## [0.2.2](https://github.com/eknowles/learnlive/compare/v0.2.1...v0.2.2) (2026-09-11)


### Features

* **settings:** let the reading type be configured, and reset ([5894e44](https://github.com/eknowles/learnlive/commit/5894e441fcc20b3f8a620da7448eb1a1bcaaf7c6))
* **study:** translate a single word on demand ([2eb4064](https://github.com/eknowles/learnlive/commit/2eb4064c963a0a8ee69d337cb582a8880b3ffe02))
* **ui:** rebuild the conversation log around pairing and hovering ([5cfba54](https://github.com/eknowles/learnlive/commit/5cfba5417a70c01008c2787d3fd3d1404cedd4a1))


### Fixes

* **asr:** drop Whisper's non-speech annotations from the transcript ([ccd1347](https://github.com/eknowles/learnlive/commit/ccd1347e9a70daddda5322f563e7c0d78e769902))
* **audio:** stop the pipeline transcribing its own voice ([21bfe65](https://github.com/eknowles/learnlive/commit/21bfe65085d67e05bce8e78fb0076258daab5f2f))
* **build:** bundle onnxruntime/sherpa-onnx dylibs and stop linking Nix's libiconv ([893f2ad](https://github.com/eknowles/learnlive/commit/893f2adff8966aa69a0a2e36b88276467c75bf2a))
* **build:** take the bundled dylibs out of the compile-time config ([cdd2fbc](https://github.com/eknowles/learnlive/commit/cdd2fbc829081a879ac075b7f2a0fb0dacc8b5ab))
* **ci:** cache the native library downloads alongside the target dir ([ac64767](https://github.com/eknowles/learnlive/commit/ac64767f22ea5a0dfc28cd3f28cd4fca1ed04339))
* **diarize:** don't register a speaker until ASR confirms there were words ([0d2aee8](https://github.com/eknowles/learnlive/commit/0d2aee8647a64205dfa03ba9242be47af1ae4f1d))
* **ui:** give every speaker a colour WebKit actually renders ([980d46a](https://github.com/eknowles/learnlive/commit/980d46a15a86bffc4392d11afcee7ea41f21d48c))

## [0.2.1](https://github.com/eknowles/learnlive/compare/v0.2.0...v0.2.1) (2026-09-10)


### Features

* add source language selector to override Whisper auto-detection ([327420c](https://github.com/eknowles/learnlive/commit/327420c0bd8a0a193326bc4ad87b5ccc4f84e00d))
* get the app compiling, running and responsive on macOS ([e8127db](https://github.com/eknowles/learnlive/commit/e8127db63f8a7f29f76ff9d0eca6b1cebbced5c0))
* redesign UI as native macOS app with split-view shell ([934de48](https://github.com/eknowles/learnlive/commit/934de48d8ce830f6c2c7991834896df4dd1af6f5))


### Fixes

* **audio:** pace file playback against an absolute deadline ([cb22d73](https://github.com/eknowles/learnlive/commit/cb22d73278f2763cf28525c1e51f88e39db4fe0f))
* **deps:** use @vitejs/plugin-react 6 so npm ci resolves under Vite 8 ([8cf66ef](https://github.com/eknowles/learnlive/commit/8cf66ef912c4595650a48100dee9d212dba94437))
