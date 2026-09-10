import { useEffect, useRef, useState } from 'react'
import TranslationDisplay from './components/TranslationDisplay'
import ControlPanel from './components/ControlPanel'
import './App.css'

interface TranslationData {
  id: string
  english: string
  russian: string
  analysis: WordAnalysis[]
  timestamp: number
}

interface WordAnalysis {
  word: string
  lemma: string
  pos: string // NOUN, VERB, ADJ, ADV, etc.
  case?: string
  tense?: string
  english_equiv?: string
}

function App() {
  const [translations, setTranslations] = useState<TranslationData[]>([])
  const [isListening, setIsListening] = useState(false)
  const [status, setStatus] = useState('Ready')
  const wsRef = useRef<WebSocket | null>(null)

  useEffect(() => {
    // Connect to Python backend WebSocket
    const connectWebSocket = () => {
      try {
        wsRef.current = new WebSocket('ws://127.0.0.1:8000/ws/transcribe')
        
        wsRef.current.onopen = () => {
          console.log('Connected to backend')
          setStatus('Connected')
        }

        wsRef.current.onmessage = (event) => {
          const data = JSON.parse(event.data) as TranslationData
          setTranslations(prev => [...prev, data])
        }

        wsRef.current.onerror = (error) => {
          console.error('WebSocket error:', error)
          setStatus('Connection error - is backend running?')
        }

        wsRef.current.onclose = () => {
          setStatus('Disconnected')
          // Attempt reconnect
          setTimeout(connectWebSocket, 3000)
        }
      } catch (error) {
        console.error('Failed to connect:', error)
        setStatus('Failed to connect to backend')
      }
    }

    connectWebSocket()

    return () => {
      if (wsRef.current) {
        wsRef.current.close()
      }
    }
  }, [])

  const handleStartListening = () => {
    setIsListening(true)
    setStatus('Listening...')
    // Send signal to backend to start capturing audio
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ action: 'start' }))
    }
  }

  const handleStopListening = () => {
    setIsListening(false)
    setStatus('Stopped')
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ action: 'stop' }))
    }
  }

  const handleClear = () => {
    setTranslations([])
  }

  return (
    <div className="app">
      <header className="app-header">
        <h1>LearnLive</h1>
        <p className="subtitle">Live translation for Russian learning</p>
      </header>

      <ControlPanel
        isListening={isListening}
        status={status}
        onStart={handleStartListening}
        onStop={handleStopListening}
        onClear={handleClear}
      />

      <main className="app-main">
        {translations.length === 0 ? (
          <div className="empty-state">
            <p>Start a Google Meet session and enable audio capture to begin</p>
            <small>Make sure the Python backend is running: python backend.py</small>
          </div>
        ) : (
          <TranslationDisplay translations={translations} />
        )}
      </main>
    </div>
  )
}

export default App
