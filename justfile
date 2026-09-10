# LearnLive — `just` to list, `just <recipe>` to run. Everything assumes `devbox shell`.

set dotenv-load := true
set shell := ["bash", "-euo", "pipefail", "-c"]

manifest := "src-tauri/Cargo.toml"
models   := env_var_or_default("LEARNLIVE_MODELS", ".models")

default:
    @just --list --unsorted

# ---- setup ----------------------------------------------------------------------------------

# One-time: node deps + tauri CLI (+ cargo-audit for `just audit`)
setup:
    npm ci
    cargo install tauri-cli --version "^2" --locked
    cargo install cargo-audit --locked || true

# Download models for a language pair (default ru,en, Whisper small)
models langs="ru,en" asr="small":
    cargo run --manifest-path {{manifest}} --bin model-fetch -- --models "{{models}}" --asr {{asr}} --langs {{langs}}

# ---- run ------------------------------------------------------------------------------------

# App with hot reload
dev:
    npx tauri dev

# Run the live app against a recording instead of a call (paced at real time)
demo wav:
    LEARNLIVE_DEMO_SOURCE="file:{{wav}}" npx tauri dev

# Production bundle → src-tauri/target/release/bundle/
build:
    npx tauri build

# ---- test -----------------------------------------------------------------------------------

# Everything that needs no models (what CI runs on every push)
test: test-ui test-rust lint

test-ui:
    npx tsc --noEmit
    npx vitest run

test-rust:
    cargo test --manifest-path {{manifest}}

lint:
    cargo clippy --manifest-path {{manifest}} --all-targets -- -D warnings
    cargo fmt --manifest-path {{manifest}} --check

fmt:
    cargo fmt --manifest-path {{manifest}}
    npx prettier --write "src/**/*.{ts,tsx,css}" 2>/dev/null || true

# Regenerate synthetic fixtures from fixtures/*.script.json (needs `just models`)
fixtures:
    #!/usr/bin/env bash
    set -euo pipefail
    for s in src-tauri/fixtures/*.script.json; do
      stem="${s%.script.json}"
      cargo run --manifest-path {{manifest}} --bin fixture-gen -- --models "{{models}}" "$s" "$stem"
    done

# Golden tests with real models (needs `just models` and `just fixtures`)
golden:
    LEARNLIVE_MODELS="{{models}}" cargo test --manifest-path {{manifest}} --test golden -- --ignored --nocapture

# ---- housekeeping ---------------------------------------------------------------------------

# Where the app keeps things on this machine
paths:
    @echo "models:  {{models}}"
    @echo "app data: ~/Library/Application Support/dev.eknowles.learnlive"
    @echo "clips:    ~/Library/Caches/dev.eknowles.learnlive/clips"

# Open the history database
db:
    sqlite3 "$HOME/Library/Application Support/dev.eknowles.learnlive/learnlive.sqlite"

audit:
    cargo audit --file src-tauri/Cargo.lock || true
    npm audit --omit=dev || true

clean:
    rm -rf dist node_modules src-tauri/target src-tauri/fixtures/*.wav src-tauri/fixtures/*.expected.jsonl

# Tag-free release flow: merge the release-please PR on GitHub; this just shows what's pending
release-status:
    gh pr list --label "autorelease: pending" --state open
