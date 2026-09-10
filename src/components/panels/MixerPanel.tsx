import { api } from '../../lib/api'
import type { AudioDevice, Levels, SessionConfig } from '../../lib/types'
import Button from '../ui/Button'
import { Group } from '../ui/Form'

interface Props {
  cfg: SessionConfig
  setCfg: (f: (c: SessionConfig) => SessionConfig) => void
  devices: AudioDevice[]
  levels: Levels | null
}

/** Per-source gain, mute and a live meter while a session runs. */
export default function MixerPanel({ cfg, setCfg, devices, levels }: Props) {
  const level = (id: string) => levels?.per_source.find(([d]) => d === id)?.[1] ?? 0
  const update = (device_id: string, patch: Partial<{ gain: number; muted: boolean }>) =>
    setCfg(c => {
      const sources = c.sources.map(s => (s.device_id === device_id ? { ...s, ...patch } : s))
      const src = sources.find(s => s.device_id === device_id)
      if (src) api.updateMixer(src)
      return { ...c, sources }
    })

  return (
    <Group title="Mix">
      {cfg.sources.map(s => {
        const name = devices.find(d => d.id === s.device_id)?.name ?? s.device_id
        const db = 20 * Math.log10(Math.max(level(s.device_id), 1e-4))
        const pct = Math.max(0, (db + 60) / 60) * 100
        return (
          <div key={s.device_id} className={`row stack strip ${s.muted ? 'muted' : ''}`}>
            <span className="row-text">
              <span className="row-label strip-name" title={name}>
                {s.role === 'local' ? 'You' : 'Call'} · {name}
              </span>
            </span>
            <span className="row-control">
              <span className={`meter ${pct > 85 ? 'hot' : ''}`} aria-hidden>
                <span style={{ width: `${pct}%` }} />
              </span>
              <Button variant="plain" pressed={s.muted} onClick={() => update(s.device_id, { muted: !s.muted })}>
                {s.muted ? 'Unmute' : 'Mute'}
              </Button>
            </span>
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
    </Group>
  )
}
