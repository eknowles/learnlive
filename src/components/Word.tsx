import { useState } from 'react'
import { api } from '../lib/api'
import { FEATS, POS } from '../lib/pos'
import type { Token } from '../lib/types'

export default function Word({ token, lang, changed = false }: { token: Token; lang: string; changed?: boolean }) {
  const [open, setOpen] = useState(false)
  const info = POS[token.pos] ?? POS.X
  if (token.pos === 'PUNCT') return <span className="punct">{token.text}</span>
  const feats = Object.entries(token.feats).filter(([k]) => k !== 'Confidence')
  const isHint = token.feats.Confidence === 'hint'
  return (
    <span className={`w ${changed ? 'changed' : ''}`} style={{ ['--pos' as string]: info.color }}
      onMouseEnter={() => setOpen(true)} onMouseLeave={() => setOpen(false)}
      onFocus={() => setOpen(true)} onBlur={() => setOpen(false)} tabIndex={0}>
      {token.text}
      {open && (
        <span className="card" role="tooltip">
          <span className="card-lemma">{token.lemma}</span>
          <span className="card-pos" style={{ color: info.color }}>{info.name}</span>
          {feats.length > 0 && (
            <span className="card-feats">
              {feats.map(([k, v]) => <span key={k}>{FEATS[k]?.[v] ?? `${k} ${v}`}</span>)}
              {isHint && <span className="quiet">guessed from the ending</span>}
            </span>
          )}
          <button onMouseDown={e => { e.preventDefault(); api.speak(token.text, lang) }}>Hear</button>
        </span>
      )}
    </span>
  )
}
