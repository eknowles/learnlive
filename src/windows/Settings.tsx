import { useEffect, useState } from 'react'
import { api } from '../lib/api'
import { confirmForgetVoices } from '../lib/dialogs'
import { loadConfig, onConfigChanged, saveConfig } from '../lib/prefs'
import { FONT_NAMES, isDefaultReading, MODES, SIZE_RANGE, type FontChoice, type ReadingPrefs } from '../lib/reading'
import type { SessionConfig } from '../lib/types'
import { useReading } from '../hooks/useReading'
import Button from '../components/ui/Button'
import { Group, Row } from '../components/ui/Form'
import Switch from '../components/ui/Switch'

const FONTS = Object.keys(FONT_NAMES) as FontChoice[]

/** Font family and size for one of the two transcript lines. */
function TypeRow({
  label,
  caption,
  font,
  size,
  range,
  onFont,
  onSize,
}: {
  label: string
  caption?: string
  font: FontChoice
  size: number
  range: readonly [number, number]
  onFont: (f: FontChoice) => void
  onSize: (n: number) => void
}) {
  return (
    <Row label={label} caption={caption} stack>
      <span className="type-row">
        <select value={font} onChange={e => onFont(e.target.value as FontChoice)} aria-label={`${label} font`}>
          {FONTS.map(f => (
            <option key={f} value={f}>
              {FONT_NAMES[f]}
            </option>
          ))}
        </select>
        <input
          type="range"
          min={range[0]}
          max={range[1]}
          step={1}
          value={size}
          onChange={e => onSize(Number(e.target.value))}
          aria-label={`${label} size`}
        />
        <output>{size}px</output>
      </span>
    </Row>
  )
}

/** The Settings window (⌘,). App-wide preferences; per-session choices live in the inspector. */
export default function Settings() {
  const [cfg, setCfgState] = useState<SessionConfig>(loadConfig)
  const [remember, setRemember] = useState(false)
  const { reading, update, reset } = useReading()

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

      <Group
        title="Reading"
        footer={
          <span className="reset-row">
            Sizes and fonts apply to the conversation log everywhere.
            <Button onClick={reset} disabled={isDefaultReading(reading)}>
              Reset to defaults
            </Button>
          </span>
        }
      >
        <Row label="Languages shown" caption="Also on the toolbar, or press ⌘L to cycle.">
          <select value={reading.mode} onChange={e => update({ mode: e.target.value as ReadingPrefs['mode'] })}>
            {MODES.map(m => (
              <option key={m.id} value={m.id}>
                {m.label}
              </option>
            ))}
          </select>
        </Row>
        <TypeRow
          label="Language you are learning"
          font={reading.studyFont}
          size={reading.studySize}
          range={SIZE_RANGE.study}
          onFont={f => update({ studyFont: f })}
          onSize={n => update({ studySize: n })}
        />
        <TypeRow
          label="Translation"
          font={reading.glossFont}
          size={reading.glossSize}
          range={SIZE_RANGE.gloss}
          onFont={f => update({ glossFont: f })}
          onSize={n => update({ glossSize: n })}
        />
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
