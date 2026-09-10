"""
LearnLive Backend - Real-time audio transcription and translation for Russian learning

This backend:
1. Captures audio from BlackHole (system audio)
2. Transcribes with Whisper
3. Translates English → Russian with Argos Translate
4. Analyzes Russian text with pymorphy2 for grammar info
5. Streams results to frontend via WebSocket
6. Provides TTS for pronunciation

Setup:
    pip install fastapi uvicorn faster-whisper argos-translate pymorphy2 python-sounddevice numpy

Run:
    python main.py
"""

import asyncio
import json
import logging
from datetime import datetime
from typing import Optional
import numpy as np
from fastapi import FastAPI, WebSocket
from fastapi.middleware.cors import CORSMiddleware
from contextlib import asynccontextmanager

# Audio processing
import sounddevice as sd

# ML Models
from faster_whisper import WhisperModel
import argostranslate.package
import pymorphy2

# Setup logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

# Initialize models (will download on first run)
logger.info("Loading models...")
whisper_model = WhisperModel("base", device="cpu", compute_type="int8")
morph = pymorphy2.MorphAnalyzer(lang='ru')

# Load translation language pair
try:
    argostranslate.package.update_package_index()
    available_packages = argostranslate.package.get_available_packages()
    en_to_ru = next(p for p in available_packages if p.from_code == "en" and p.to_code == "ru")
    argostranslate.package.install_from_path(en_to_ru.download())
    en = argostranslate.package.get_language("en")
    ru = en.get_translation("ru")
except Exception as e:
    logger.warning(f"Failed to load Argos translator: {e}. Will need to install manually.")
    ru = None

logger.info("Models loaded successfully!")

# Global state
class AudioCapture:
    def __init__(self):
        self.is_capturing = False
        self.audio_buffer = []
        self.sample_rate = 16000
        self.block_size = 4096
        
    async def capture_from_blackhole(self):
        """Capture audio from BlackHole device"""
        # Note: You'll need to find the correct device index for your BlackHole installation
        # Run: python -m sounddevice to list devices
        BLACKHOLE_DEVICE_INDEX = None  # Set this to your BlackHole device index
        
        if BLACKHOLE_DEVICE_INDEX is None:
            logger.warning("BlackHole device not configured. List available devices with:")
            logger.warning("  python -c \"import sounddevice as sd; print(sd.query_devices())\"")
            return
        
        logger.info(f"Starting audio capture from device {BLACKHOLE_DEVICE_INDEX}")
        
        def audio_callback(indata, frames, time_info, status):
            if status:
                logger.warning(f"Audio status: {status}")
            self.audio_buffer.append(indata.copy())
        
        try:
            with sd.InputStream(
                device=BLACKHOLE_DEVICE_INDEX,
                samplerate=self.sample_rate,
                channels=1,
                blocksize=self.block_size,
                callback=audio_callback,
            ):
                while self.is_capturing:
                    await asyncio.sleep(0.1)
        except Exception as e:
            logger.error(f"Audio capture error: {e}")

async def process_audio_buffer(audio_data: np.ndarray) -> dict:
    """Transcribe and translate audio"""
    try:
        # Transcribe with Whisper
        segments, _ = whisper_model.transcribe(audio_data, language="en")
        
        results = []
        for segment in segments:
            en_text = segment.text.strip()
            
            # Translate to Russian
            if ru:
                ru_text = ru.translate(en_text)
            else:
                ru_text = en_text  # Fallback
            
            # Analyze Russian text
            words = ru_text.split()
            analysis = []
            
            for word in words:
                parsed = morph.parse(word)[0]
                grammemes = parsed.tag.grammemes if parsed.tag else ()
                
                analysis.append({
                    'word': word,
                    'lemma': parsed.normal_form,
                    'pos': parsed.tag.POS if parsed.tag else 'UNKNOWN',
                    'case': 'nomn' in grammemes and 'nominative' or None,
                    'tense': 'present' in grammemes and 'present' or 
                            'past' in grammemes and 'past' or
                            'future' in grammemes and 'future' or None,
                })
            
            results.append({
                'english': en_text,
                'russian': ru_text,
                'analysis': analysis,
                'timestamp': datetime.now().timestamp() * 1000,
            })
        
        return {'status': 'success', 'results': results}
    
    except Exception as e:
        logger.error(f"Processing error: {e}")
        return {'status': 'error', 'message': str(e)}

# FastAPI app
@asynccontextmanager
async def lifespan(app: FastAPI):
    """Startup and shutdown events"""
    logger.info("LearnLive backend starting...")
    yield
    logger.info("LearnLive backend shutting down...")

app = FastAPI(title="LearnLive Backend", lifespan=lifespan)

# CORS
app.add_middleware(
    CORSMiddleware,
    allow_origins=["localhost", "127.0.0.1", "tauri://localhost"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# WebSocket connection manager
class ConnectionManager:
    def __init__(self):
        self.active_connections: list[WebSocket] = []
    
    async def connect(self, websocket: WebSocket):
        await websocket.accept()
        self.active_connections.append(websocket)
        logger.info(f"Client connected. Total: {len(self.active_connections)}")
    
    def disconnect(self, websocket: WebSocket):
        self.active_connections.remove(websocket)
        logger.info(f"Client disconnected. Total: {len(self.active_connections)}")
    
    async def broadcast(self, message: dict):
        for connection in self.active_connections:
            try:
                await connection.send_json(message)
            except Exception as e:
                logger.error(f"Failed to send message: {e}")

manager = ConnectionManager()
audio_capture = AudioCapture()

@app.websocket("/ws/transcribe")
async def websocket_transcribe(websocket: WebSocket):
    """WebSocket endpoint for real-time transcription and translation"""
    await manager.connect(websocket)
    
    try:
        while True:
            data = await websocket.receive_json()
            
            if data.get('action') == 'start':
                logger.info("Starting audio capture")
                audio_capture.is_capturing = True
                # Start capture in background
                asyncio.create_task(audio_capture.capture_from_blackhole())
            
            elif data.get('action') == 'stop':
                logger.info("Stopping audio capture")
                audio_capture.is_capturing = False
            
            # For demo purposes, simulate transcription
            # In production, you'd process the actual audio buffer
            if audio_capture.audio_buffer:
                audio_data = np.concatenate(audio_capture.audio_buffer)
                audio_capture.audio_buffer = []
                
                result = await process_audio_buffer(audio_data)
                
                if result['status'] == 'success':
                    for item in result['results']:
                        item['id'] = f"{int(item['timestamp'])}"
                        await websocket.send_json(item)
    
    except Exception as e:
        logger.error(f"WebSocket error: {e}")
    finally:
        manager.disconnect(websocket)
        audio_capture.is_capturing = False

@app.get("/health")
async def health():
    """Health check endpoint"""
    return {
        "status": "ok",
        "whisper_model": "base",
        "translation_available": ru is not None,
    }

@app.get("/devices")
async def list_audio_devices():
    """List available audio devices (for debugging BlackHole setup)"""
    try:
        import sounddevice as sd
        devices = sd.query_devices()
        device_list = []
        for i, device in enumerate(devices):
            device_list.append({
                "index": i,
                "name": device.get('name', 'Unknown'),
                "channels": device.get('max_input_channels', 0),
            })
        return {"devices": device_list}
    except Exception as e:
        return {"error": str(e)}

if __name__ == "__main__":
    import uvicorn
    logger.info("Starting FastAPI server on http://127.0.0.1:8000")
    logger.info("WebSocket endpoint: ws://127.0.0.1:8000/ws/transcribe")
    logger.info("Audio devices list: http://127.0.0.1:8000/devices")
    uvicorn.run(app, host="127.0.0.1", port=8000)
