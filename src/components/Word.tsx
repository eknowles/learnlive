import { useCallback, useRef, useState } from 'react'
import { api } from '../lib/api'
import Button from './ui/Button'
import { FEATS, POS } from '../lib/pos'
import type { Token } from '../lib/types'

/** Looked-up words, kept for the life of the window so re-hovering never waits. The backend
 *  caches too; this saves the IPC round trip as well as the decode. */
const glossed = new Map<string, string | null>()

interface Props {
  text: string
  /** Language of this word. */
  lang: string
  /** Language to gloss it into — the other side of the pair. */
  into: string
  /** The sentence it came from; reserved for the aligner (see `api.lookupWord`). */
  sentence: string
  /** Grammar, when a POS tagger is installed. Absent today: the tagger has no model source. */
  token?: Token
  changed?: boolean
}

/** A hoverable word. The popover opens on the first frame and the translation drops in when it
 *  arrives — waiting for the round trip before showing anything made hovering feel broken. */
export default function Word({ text, lang, into, sentence, token, changed = false }: Props) {
  const key = `${lang}|${into}|${text.toLowerCase()}`
  const [open, setOpen] = useState(false)
  const [gloss, setGloss] = useState<string | null | undefined>(() => glossed.get(key))
  const asked = useRef(false)

  const info = token ? (POS[token.pos] ?? POS.X) : null
  const feats = token ? Object.entries(token.feats).filter(([k]) => k !== 'Confidence') : []

  const show = useCallback(() => {
    setOpen(true)
    if (asked.current || glossed.has(key)) return
    asked.current = true
    api
      .lookupWord(text, sentence, lang, into)
      .then(g => {
        glossed.set(key, g.translation)
        setGloss(g.translation)
      })
      .catch(() => {
        // Usually the translation model is not downloaded yet. Remember the miss so we do not
        // retry on every hover, but let the popover say so rather than sit on a spinner.
        glossed.set(key, null)
        setGloss(null)
      })
  }, [key, text, sentence, lang, into])

  return (
    <span
      className={`w ${changed ? 'changed' : ''} ${open ? 'lit' : ''}`}
      style={info ? { ['--pos' as string]: info.color } : undefined}
      onMouseEnter={show}
      onMouseLeave={() => setOpen(false)}
      onFocus={show}
      onBlur={() => setOpen(false)}
      tabIndex={0}
    >
      {text}
      {open && (
        <span className="card" role="tooltip">
          <span className="card-head">
            <span className="card-word" lang={lang}>
              {text}
            </span>
            <Button
              icon="speaker"
              aria-label={`Hear ${text}`}
              onMouseDown={e => {
                e.preventDefault()
                api.speak(text, lang)
              }}
            />
          </span>
          <span className={`card-gloss ${gloss === undefined ? 'pending' : ''}`} lang={into}>
            {gloss === undefined ? 'Looking up…' : (gloss ?? 'No translation available')}
          </span>
          {token && info && (
            <>
              {token.lemma !== text && <span className="card-lemma">{token.lemma}</span>}
              <span className="card-pos" style={{ color: info.color }}>
                {info.name}
              </span>
              {feats.length > 0 && (
                <span className="card-feats">
                  {feats.map(([k, v]) => (
                    <span key={k}>{FEATS[k]?.[v] ?? `${k} ${v}`}</span>
                  ))}
                  {token.feats.Confidence === 'hint' && <span className="quiet">guessed from the ending</span>}
                </span>
              )}
            </>
          )}
        </span>
      )}
    </span>
  )
}
