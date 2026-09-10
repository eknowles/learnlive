import type { Dispatch, SetStateAction } from 'react'
import type { AudioDevice, Language, ModelProgress, ModelStatus, SessionConfig } from '../lib/types'

interface Props {
  cfg: SessionConfig
  setCfg: Dispatch<SetStateAction<SessionConfig>>
  devices: AudioDevice[]
  languages: Language[]
  models: ModelStatus[]
  progress: Record<string, ModelProgress>
  phase: 'setup' | 'downloading' | 'starting' | 'live'
  onStart: () => void
  onStop: () => void
}

const mb = (n: number) => (n / 1_048_576).toFixed(0)

export default function Setup({ cfg, setCfg, devices, languages, models, progress, phase, onStart, onStop }: Props) {
  const busy = phase !== 'setup' && phase !== 'live'
  const set = <K extends keyof SessionConfig>(k: K, v: SessionConfig[K]) => setCfg(c => ({ ...c, [k]: v }))
  const missing = models.filter(m => !m.present)
  const missingMb = missing.reduce((a, m) => a + m.approx_mb, 0)
  const loopback = devices.filter(d => d.is_loopback)
  const learning = languages.find(l => l.code === cfg.learning)
  const remote = cfg.sources.find(s => s.role === 'remote')
  const local = cfg.sources.find(s => s.role === 'local')

  const setSource = (role: 'remote' | 'local', device_id: string) =>
    setCfg(c => ({
      ...c,
      sources: [...c.sources.filter(s => s.role !== role), ...(device_id ? [{ device_id, role, gain: 1, muted: false }] : [])],
    }))

  return (
    <section className="setup" aria-label="Session setup">
      <label>
        I'm learning
        <select value={cfg.learning} disabled={phase === 'live'} onChange={e => set('learning', e.target.value)}>
          {languages.map(l => (
            <option key={l.code} value={l.code}>
              {l.name} · {l.native}
            </option>
          ))}
        </select>
        {learning && !learning.grammar && (
          <small className="hint">No grammar tagger installed, so words won't be clickable. Everything else works.</small>
        )}
      </label>
      <label>
        I already speak
        <select value={cfg.native} disabled={phase === 'live'} onChange={e => set('native', e.target.value)}>
          {languages.map(l => (
            <option key={l.code} value={l.code}>
              {l.name}
            </option>
          ))}
        </select>
      </label>

      <label>
        Call audio
        <select value={remote?.device_id ?? ''} disabled={phase === 'live'} onChange={e => setSource('remote', e.target.value)}>
          <option value="">None</option>
          {devices.map(d => (
            <option key={d.id} value={d.id}>
              {d.name}
              {d.is_loopback ? '' : ' (not a loopback device)'}
            </option>
          ))}
        </select>
        {loopback.length === 0 && (
          <small className="hint">No loopback device found. Install BlackHole and set it as your call's speaker.</small>
        )}
      </label>
      <label>
        Your microphone
        <select value={local?.device_id ?? ''} disabled={phase === 'live'} onChange={e => setSource('local', e.target.value)}>
          <option value="">None</option>
          {devices
            .filter(d => !d.is_loopback)
            .map(d => (
              <option key={d.id} value={d.id}>
                {d.name}
              </option>
            ))}
        </select>
      </label>

      <details>
        <summary>Advanced</summary>
        <label>
          Recognition quality
          <select
            value={cfg.asr_model}
            disabled={phase === 'live'}
            onChange={e => set('asr_model', e.target.value as SessionConfig['asr_model'])}
          >
            <option value="tiny">Fastest (tiny)</option>
            <option value="base">Fast (base)</option>
            <option value="small">Balanced (small)</option>
            <option value="medium">Accurate (medium)</option>
            <option value="large-v3-turbo">Best (large‑v3 turbo)</option>
          </select>
        </label>
        <label className="row">
          <input type="checkbox" checked={cfg.diarize} disabled={phase === 'live'} onChange={e => set('diarize', e.target.checked)} />
          Tell speakers apart
        </label>
        <label className="row">
          <input
            type="checkbox"
            checked={cfg.speak_translations}
            disabled={phase === 'live'}
            onChange={e => set('speak_translations', e.target.checked)}
          />
          Read translations aloud
        </label>
        {cfg.speak_translations && (
          <label>
            Lower the call while reading · {Math.round(cfg.duck_amount * 100)}%
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={cfg.duck_amount}
              onChange={e => set('duck_amount', Number(e.target.value))}
            />
          </label>
        )}
      </details>

      {phase === 'live' ? (
        <button className="primary stop" onClick={onStop}>
          Stop listening
        </button>
      ) : (
        <button className="primary" onClick={onStart} disabled={busy || cfg.sources.length === 0}>
          {phase === 'downloading'
            ? 'Downloading models…'
            : phase === 'starting'
              ? 'Loading models…'
              : missing.length
                ? `Download ${missingMb} MB and start`
                : 'Start listening'}
        </button>
      )}

      {phase === 'downloading' && (
        <ul className="downloads">
          {missing.map(m => {
            const p = progress[m.id]
            const pct = p?.total ? Math.min(100, (p.bytes / p.total) * 100) : p?.done ? 100 : 0
            return (
              <li key={m.id}>
                <span>{m.id}</span>
                <progress value={pct} max={100} />
                <span>{p?.done ? 'done' : p ? `${mb(p.bytes)} MB` : 'queued'}</span>
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}
