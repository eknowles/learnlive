# LearnLive

Live translation and grammar analysis app for practicing Russian in Google Meet sessions.

**Live English → Russian translation with interactive word analysis, pronunciation, and grammar highlighting.**

## Features

- 🎤 **Real-time transcription** - Captures audio from Google Meet
- 🌐 **Instant translation** - English to Russian with Argos Translate
- 📖 **Grammar analysis** - Part of speech tagging and morphological analysis
- 🎨 **Interactive UI** - Hover over Russian words to see:
  - English equivalent
  - Dictionary form (lemma)
  - Part of speech (noun, verb, adjective, etc.)
  - Grammatical information (case, tense, etc.)
- 🔊 **Pronunciation** - Click words to hear them spoken
- 🎯 **Color-coded grammar** - Visual highlighting of different word types

## Tech Stack

### Frontend
- **Tauri** - Lightweight desktop app framework
- **React 18** - UI components
- **TypeScript** - Type safety
- **Vite** - Fast dev server

### Backend
- **FastAPI** - Python web framework
- **Faster-Whisper** - Fast speech-to-text (OpenAI Whisper)
- **Argos Translate** - Offline translation
- **pymorphy2** - Russian morphological analysis
- **python-sounddevice** - Audio capture

## Setup

### Prerequisites

- **Node.js** 18+ (for Tauri/React)
- **Rust** (for Tauri) - Install from [rustup.rs](https://rustup.rs/)
- **Python** 3.10+ (for backend)
- **BlackHole** - Virtual audio device for capturing system audio
  - Download: https://existential.audio/blackhole/
  - Install and restart after installation

### Backend Setup

1. Create Python environment:
```bash
cd backend
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
```

2. Install dependencies:
```bash
pip install -r requirements.txt
```

3. Find your BlackHole device index:
```bash
python -c "import sounddevice as sd; import pprint; pprint.pprint(sd.query_devices())"
```

Look for something like "BlackHole 2ch" and note its index number.

4. Update `backend/main.py` with your device index:
```python
BLACKHOLE_DEVICE_INDEX = 12  # Change this to your BlackHole index
```

5. Test the backend:
```bash
python main.py
```

Should see:
```
INFO:     Application startup complete
WebSocket endpoint: ws://127.0.0.1:8000/ws/transcribe
```

### Frontend Setup

1. Install Node dependencies:
```bash
npm install
```

2. Install Tauri CLI:
```bash
cargo install tauri-cli
```

3. Development server (in new terminal):
```bash
npm run tauri:dev
```

This launches both the Vite dev server (port 5173) and the Tauri app.

4. Build for production:
```bash
npm run tauri:build
```

Creates `.dmg` in `src-tauri/target/release/bundle/macos/`

## Usage

### Step 1: Configure Google Meet Audio

1. Open Google Meet in Chrome
2. Go to Settings → Audio
3. Set **Speaker output** to "BlackHole 2ch"
4. Keep Meet window open

### Step 2: Start the Backend

```bash
cd backend
source venv/bin/activate
python main.py
```

Leave this running in the background.

### Step 3: Launch LearnLive App

```bash
npm run tauri:dev
```

Or use the built `.dmg` if running production build.

### Step 4: Start Listening

1. Click **"Start Listening"** in the app
2. Speak in Google Meet (or have the speaker talk)
3. Watch translations appear with grammar analysis

## How It Works

```
Google Meet Audio
    ↓
BlackHole (virtual audio device)
    ↓
Python Backend
├─ Whisper (transcribe English audio)
├─ Argos Translate (translate EN → RU)
├─ pymorphy2 (analyze Russian grammar)
└─ WebSocket (send to frontend)
    ↓
Tauri App (React frontend)
├─ Display sentences
├─ Highlight words by POS
└─ Show grammar on hover
```

## Interactive Features

### Hover on Russian Word
Shows a popup with:
- **Lemma** - Dictionary form of the word
- **POS** - Part of speech (NOUN, VERB, etc.)
- **English** - Translation of that word
- **Grammar** - Case, tense, aspect, etc.

### Click Word
Plays pronunciation (coming in next version)

### Color Legend
- 🔵 Blue - Nouns
- 🔴 Red - Verbs
- 🟢 Green - Adjectives
- 🟡 Yellow - Adverbs
- 🟣 Purple - Prepositions

## Project Structure

```
learnlive/
├── src/                      # React frontend
│   ├── components/
│   │   ├── TranslationDisplay.tsx
│   │   ├── ControlPanel.tsx
│   │   └── *.css
│   ├── App.tsx
│   ├── main.tsx
│   └── index.css
├── src-tauri/                # Tauri/Rust
│   ├── src/
│   │   └── main.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── backend/                  # Python FastAPI
│   ├── main.py
│   └── requirements.txt
├── index.html
├── package.json
├── vite.config.ts
└── README.md
```

## Development

### Frontend Development

Hot reload is enabled. Changes to React files auto-refresh:

```bash
npm run tauri:dev
```

### Backend Development

Restart the Python process to reload:

```bash
# Terminal 1
cd backend
python main.py

# Terminal 2 (in another tab)
npm run tauri:dev
```

### Adding Features

**Add a new React component:**
1. Create `src/components/MyComponent.tsx`
2. Import and use in `src/App.tsx`

**Add a new Python endpoint:**
1. Add route to `backend/main.py`
2. Call via fetch in React component

**Add a new ML model:**
1. Add import to `backend/main.py`
2. Initialize in startup
3. Use in `process_audio_buffer()` function

## Troubleshooting

### "BlackHole device not found"
- Make sure BlackHole is installed and Mac was restarted
- Run `python -c "import sounddevice as sd; print(sd.query_devices())"` to list devices
- Update `BLACKHOLE_DEVICE_INDEX` in `backend/main.py`

### "Failed to connect to backend"
- Make sure `python main.py` is running in the backend directory
- Check that you're running on port 8000
- Look for error messages in the terminal

### "Whisper models downloading forever"
- Whisper downloads ~140MB on first run
- Make sure you have internet connection
- Models cache in `~/.cache/huggingface/`

### "Argos Translate package install fails"
- Try: `argostranslate-package-en-ru` (may need manual package install)
- Or use HuggingFace models as fallback

### App crashes on startup
- Check browser console for errors (F12 in dev mode)
- Check Python backend logs
- Try: `npm install` and `cargo update`

## Next Steps

### Planned Features

- [ ] Audio playback of translations
- [ ] Slow-motion playback (0.8x speed)
- [ ] Flashcard generation from translations
- [ ] Grammar statistics dashboard
- [ ] User dictionary (save custom translations)
- [ ] Settings panel (language pair selection)
- [ ] Dark mode
- [ ] Support for other language pairs

### Known Limitations

- **Latency**: 2-5 seconds between speech and translation (normal for Whisper)
- **Accuracy**: Depends on audio quality and speaker clarity
- **Languages**: Currently English ↔ Russian only
- **Requires separate Python process** - Not bundled into single executable (yet)

## Contributing

Found a bug or have a feature request? Open an issue!

## License

MIT

## Resources

- [Tauri Docs](https://tauri.app/)
- [Whisper GitHub](https://github.com/openai/whisper)
- [Argos Translate](https://www.argosopentech.com/)
- [pymorphy2 Docs](https://pymorphy2.readthedocs.io/)
- [FastAPI](https://fastapi.tiangolo.com/)

---

Made for Russian learners. Happy learning! 🚀
