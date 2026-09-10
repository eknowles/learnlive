import { useEffect, useState } from 'react'
import { api } from '../lib/api'
import { confirmForgetVoices } from '../lib/dialogs'
import { loadConfig, onConfigChanged, saveConfig } from '../lib/prefs'
import type { SessionConfig } from '../lib/types'
import { Group, Row } from '../components/ui/Form'
import Switch from '../components/ui/Switch'

/** The Settings window (⌘,). App-wide preferences; per-session choices live in the inspector. */
export default function Settings() {
  const [cfg, setCfgState] = useState<SessionConfig>(loadConfig)
  const [remember, setRemember] = useState(false)

  useEffect(() => {
    api.getRememberVoices().then(setRemember)
    const p = onConfigChanged(setCfgState)
    return () => {
      p.then(f => f())
    }
  }, [])

  const set = <K extends keyof SessionConfig>(k: K, v: SessionConfig[K]) =>
    setCfgState(c => {
      const next = { ...c, [k]: v }
      saveConfig(next)
      return next
    })

  const toggleRemember = async (on: boolean) => {
    if (!on && !(await confirmForgetVoices())) return
    await api.setRememberVoices(on)
    setRemember(on)
  }

  return (
    <div className="settings">
      <Group title="Recognition">
        <Row label="Quality" caption="Larger models are more accurate and slower. Changes apply to the next session.">
          <select value={cfg.asr_model} onChange={e => set('asr_model', e.target.value as SessionConfig['asr_model'])}>
            <option value="tiny">Fastest</option>
            <option value="base">Fast</option>
            <option value="small">Balanced</option>
            <option value="medium">Accurate</option>
            <option value="large-v3-turbo">Best</option>
          </select>
        </Row>
        <Row label="Tell speakers apart" caption="Label each voice on the call separately.">
          <Switch checked={cfg.diarize} onChange={e => set('diarize', e.target.checked)} />
        </Row>
      </Group>

      <Group title="Voice">
        <Row label="Read translations aloud">
          <Switch checked={cfg.speak_translations} onChange={e => set('speak_translations', e.target.checked)} />
        </Row>
        <Row label="Lower the call while reading" stack>
          <span className="slider-row">
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={cfg.duck_amount}
              disabled={!cfg.speak_translations}
              onChange={e => set('duck_amount', Number(e.target.value))}
              aria-label="Ducking amount"
            />
            <output>{Math.round(cfg.duck_amount * 100)}%</output>
          </span>
        </Row>
      </Group>

      <Group title="Privacy" footer="Voiceprints are stored only in this Mac’s app data. Turning this off deletes them.">
        <Row label="Remember voices" caption="Recognise people you’ve named from their first sentence next time.">
          <Switch checked={remember} onChange={e => toggleRemember(e.target.checked)} />
        </Row>
      </Group>

      <p className="settings-foot">Everything runs on this Mac. Nothing is uploaded.</p>
    </div>
  )
}
