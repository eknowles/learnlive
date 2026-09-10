import type { AudioDevice, Language, SessionConfig } from '../../lib/types'
import { Group, Row } from '../ui/Form'

interface Props {
  cfg: SessionConfig
  setCfg: (f: (c: SessionConfig) => SessionConfig) => void
  devices: AudioDevice[]
  languages: Language[]
  locked: boolean
}

/** Languages and audio sources for the next session. Locked while listening. */
export default function SessionPanel({ cfg, setCfg, devices, languages, locked }: Props) {
  const set = <K extends keyof SessionConfig>(k: K, v: SessionConfig[K]) => setCfg(c => ({ ...c, [k]: v }))
  const learning = languages.find(l => l.code === cfg.learning)
  const remote = cfg.sources.find(s => s.role === 'remote')
  const local = cfg.sources.find(s => s.role === 'local')
  const loopback = devices.filter(d => d.is_loopback)

  const setSource = (role: 'remote' | 'local', device_id: string) =>
    setCfg(c => ({
      ...c,
      sources: [...c.sources.filter(s => s.role !== role), ...(device_id ? [{ device_id, role, gain: 1, muted: false }] : [])],
    }))

  return (
    <>
      <Group
        title="Languages"
        footer={learning && !learning.grammar ? 'No grammar tagger is installed, so words won’t have hover cards.' : undefined}
      >
        <Row label="I’m learning">
          <select value={cfg.learning} disabled={locked} onChange={e => set('learning', e.target.value)}>
            {languages.map(l => (
              <option key={l.code} value={l.code}>
                {l.name} · {l.native}
              </option>
            ))}
          </select>
        </Row>
        <Row label="I already speak">
          <select value={cfg.native} disabled={locked} onChange={e => set('native', e.target.value)}>
            {languages.map(l => (
              <option key={l.code} value={l.code}>
                {l.name}
              </option>
            ))}
          </select>
        </Row>
        <Row label="People are speaking" caption="Leave auto to detect from the audio.">
          <select value={cfg.source_lang} disabled={locked} onChange={e => set('source_lang', e.target.value)}>
            <option value="auto">Auto-detect</option>
            {languages.map(l => (
              <option key={l.code} value={l.code}>
                {l.name}
              </option>
            ))}
          </select>
        </Row>
      </Group>

      <Group
        title="Audio"
        footer={loopback.length === 0 ? 'No loopback device found. Install BlackHole and make it your call’s speaker.' : undefined}
      >
        <Row label="Call audio" stack>
          <select value={remote?.device_id ?? ''} disabled={locked} onChange={e => setSource('remote', e.target.value)}>
            <option value="">None</option>
            {devices.map(d => (
              <option key={d.id} value={d.id}>
                {d.name}
                {d.is_loopback ? '' : ' (not a loopback device)'}
              </option>
            ))}
          </select>
        </Row>
        <Row label="Your microphone" stack>
          <select value={local?.device_id ?? ''} disabled={locked} onChange={e => setSource('local', e.target.value)}>
            <option value="">None</option>
            {devices
              .filter(d => !d.is_loopback)
              .map(d => (
                <option key={d.id} value={d.id}>
                  {d.name}
                </option>
              ))}
          </select>
        </Row>
      </Group>
    </>
  )
}
