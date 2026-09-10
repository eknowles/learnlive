# Contributing

```bash
devbox shell        # toolchain (Rust, Node, just, ffmpeg…) from devbox.json
just setup          # npm ci + tauri CLI
just dev            # run the app
just test           # what CI runs on every push
just models         # ~1 GB of models for golden tests / fixtures
just fixtures       # synth test audio from fixtures/*.script.json
just golden         # real-model tests
```

## Commits and releases

Squash-merge PRs with a [conventional commit](https://www.conventionalcommits.org/) title.
[release-please](https://github.com/googleapis/release-please) keeps a "chore(main): release x.y.z" PR
open with the changelog; merging it tags the release, bumps `package.json`, `src-tauri/Cargo.toml`
and `src-tauri/tauri.conf.json` together, and `release-please.yml` builds and attaches the `.dmg`.

| prefix | effect |
|---|---|
| `fix:` | patch |
| `feat:` | minor (while < 1.0, also minor) |
| `feat!:` / `BREAKING CHANGE:` | minor while < 1.0, major after |
| anything else | no release, hidden from changelog |
