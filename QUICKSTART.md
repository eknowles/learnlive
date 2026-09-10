# LearnLive Quick Start

Get up and running in 5 minutes.

## 1. Install BlackHole (Mac only)

```bash
# Install BlackHole for virtual audio routing
# Download: https://existential.audio/blackhole/
# Then restart your Mac
```

## 2. Find Your BlackHole Device Index

```bash
# In a terminal
cd backend
python3 -c "import sounddevice as sd; import pprint; pprint.pprint(sd.query_devices())"

# Look for "BlackHole" in the output, note the index (e.g., 12)
```

## 3. Update Backend Config

Edit `backend/main.py` and find this line:

```python
BLACKHOLE_DEVICE_INDEX = None  # Set this to your BlackHole device index
```

Change it to:

```python
BLACKHOLE_DEVICE_INDEX = 12  # Replace 12 with your device index
```

## 4. Setup Python Backend

```bash
cd backend
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

## 5. Setup Node Frontend

```bash
cd ..  # Back to project root
npm install
```

## 6. Install Rust (if not already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install tauri-cli
```

## 7. Start Everything

**Terminal 1 - Backend:**
```bash
cd backend
source venv/bin/activate
python main.py
```

**Terminal 2 - Frontend:**
```bash
npm run tauri:dev
```

The app should open. You'll see the LearnLive window.

## 8. Use It

1. Open Google Meet in Chrome
2. In Meet settings, set Speaker output to "BlackHole 2ch"
3. Click **"Start Listening"** in LearnLive
4. Speak or play audio in Google Meet
5. Watch Russian translations appear with grammar analysis
6. Hover over Russian words to see definitions

## Troubleshooting

**"Connection error - is backend running?"**
- Make sure `python main.py` is running and shows "Application startup complete"

**"BlackHole device not found"**
- Check your device index matches in `backend/main.py`
- Restart your Mac after installing BlackHole

**Models downloading slowly**
- First run downloads ~140MB of Whisper models
- This is normal and only happens once

**Translation not working**
- Check backend logs for "Failed to load Argos translator"
- You may need to manually install: `pip install argos-translate-en-ru`

## Next: Learn More

Check out `README.md` for:
- Full feature documentation
- Project structure
- Development setup
- Contributing guidelines

Happy learning! 🚀
