import type { Dispatch, SetStateAction } from 'react'
import { api } from '../lib/api'
import type { AudioDevice, Levels, SessionConfig } from '../lib/types'

interface Props {
  cfg: SessionConfig
  setCfg: Dispatch<SetStateAction<SessionConfig>>
  devices: AudioDevice[]
  levels: Levels | null
}

export default function Mixer({ cfg, setCfg, devices, levels }: Props) {
  const level = (id: string) => levels?.per_source.find(([d]) => d === id)?.[1] ?? 0
  const update = (device_id: string, patch: Partial<{ gain: number; muted: boolean }>) => {
    setCfg(c => {
      const sources = c.sources.map(s => (s.device_id === device_id ? { ...s, ...patch } : s))
      const src = sources.find(s => s.device_id === device_id)
      if (src) api.updateMixer(src)
      return { ...c, sources }
    })
  }
  return (
    <section className="mixer" aria-label="Mixer">
      <h2>Mix</h2>
      {cfg.sources.map(s => {
        const name = devices.find(d => d.id === s.device_id)?.name ?? s.device_id
        const db = 20 * Math.log10(Math.max(level(s.device_id), 1e-4))
        return (
          <div key={s.device_id} className={`strip ${s.muted ? 'muted' : ''}`}>
            <div className="strip-head">
              <span className="strip-name" title={name}>
                {s.role === 'local' ? 'You' : 'Call'} · {name}
              </span>
              <button className="mute" aria-pressed={s.muted} onClick={() => update(s.device_id, { muted: !s.muted })}>
                {s.muted ? 'Unmute' : 'Mute'}
              </button>
            </div>
            <div className="meter" aria-hidden>
              <span style={{ width: `${Math.max(0, (db + 60) / 60) * 100}%` }} />
            </div>
            <input
              type="range"
              min={0}
              max={2}
              step={0.05}
              value={s.gain}
              aria-label={`${name} gain`}
              onChange={e => update(s.device_id, { gain: Number(e.target.value) })}
            />
          </div>
        )
      })}
    </section>
  )
}
