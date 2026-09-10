import { useEffect, useRef, useState } from 'react'
import { api } from './lib/api'
import type { AudioDevice, Language, Levels, ModelProgress, ModelStatus, Segment, SessionConfig } from './lib/types'
import Setup from './components/Setup'
import Mixer from './components/Mixer'
import Speakers from './components/Speakers'
import Transcript from './components/Transcript'
import './styles.css'

type Phase = 'setup' | 'downloading' | 'starting' | 'live'

const DEFAULT: SessionConfig = {
  sources: [], learning: 'ru', native: 'en', source_lang: 'auto',
  speak_translations: true, duck_amount: 0.5, asr_model: 'small', diarize: true,
}

export default function App() {
  const [cfg, setCfg] = useState<SessionConfig>(DEFAULT)
  const [devices, setDevices] = useState<AudioDevice[]>([])
  const [languages, setLanguages] = useState<Language[]>([])
  const [models, setModels] = useState<ModelStatus[]>([])
  const [progress, setProgress] = useState<Record<string, ModelProgress>>({})
  const [phase, setPhase] = useState<Phase>('setup')
  const [segments, setSegments] = useState<Segment[]>([])
  const [levels, setLevels] = useState<Levels | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [names, setNames] = useState<Record<number, string>>({})
  const unlisten = useRef<(() => void)[]>([])

  useEffect(() => {
    api.devices().then(ds => {
      setDevices(ds)
      // Sensible default: first loopback device as "remote", default mic as "local".
      const loop = ds.find(d => d.is_loopback)
      const mic = ds.find(d => !d.is_loopback)
      setCfg(c => ({ ...c, sources: [
        ...(loop ? [{ device_id: loop.id, role: 'remote' as const, gain: 1, muted: false }] : []),
        ...(mic ? [{ device_id: mic.id, role: 'local' as const, gain: 1, muted: false }] : []),
      ] }))
    }).catch(e => setError(String(e)))
    api.languages().then(setLanguages)
    Promise.all([
      api.onSegment(s => setSegments(prev => prev.some(p => p.id === s.id) ? prev.map(p => p.id === s.id ? s : p) : [...prev, s])),
      api.onLevels(setLevels),
      api.onModelProgress(p => setProgress(prev => ({ ...prev, [p.model]: p }))),
      api.onError(setError),
    ]).then(fns => { unlisten.current = fns })
    return () => unlisten.current.forEach(f => f())
  }, [])

  useEffect(() => { api.modelStatus(cfg).then(setModels).catch(() => {}) }, [cfg.learning, cfg.native, cfg.asr_model, cfg.diarize, cfg.speak_translations])

  const start = async () => {
    setError(null)
    try {
      if (models.some(m => !m.present)) { setPhase('downloading'); await api.prepareModels(cfg) }
      setPhase('starting')
      await api.start(cfg)
      setPhase('live')
    } catch (e) { setError(String(e)); setPhase('setup') }
  }
  const stop = async () => { await api.stop(); setPhase('setup') }

  const rename = (id: number, label: string) => { setNames(n => ({ ...n, [id]: label })); api.renameSpeaker(id, label) }
  const shown = segments.map(s => ({ ...s, speaker: { ...s.speaker, label: names[s.speaker.id] ?? s.speaker.label } }))

  return (
    <div className="shell">
      <aside className="rail">
        <h1 className="brand">LearnLive</h1>
        <Setup cfg={cfg} setCfg={setCfg} devices={devices} languages={languages} models={models} progress={progress} phase={phase} onStart={start} onStop={stop} />
        {phase === 'live' && <Mixer cfg={cfg} setCfg={setCfg} devices={devices} levels={levels} />}
        <Speakers segments={shown} onRename={rename} />
        {error && <p className="error" role="alert">{error}</p>}
      </aside>
      <main className="stage">
        <Transcript segments={shown} learning={cfg.learning} live={phase === 'live'} speechActive={levels?.speech_active ?? false} />
      </main>
    </div>
  )
}
