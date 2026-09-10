import { useEffect, useRef } from 'react'
import { api } from '../lib/api'
import { speakerColor } from '../lib/pos'
import type { Segment } from '../lib/types'
import Word from './Word'

interface Props { segments: Segment[]; learning: string; live: boolean; speechActive: boolean }

const t = (ms: number) => {
  const s = Math.floor(ms / 1000)
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

export default function Transcript({ segments, learning, live, speechActive }: Props) {
  const end = useRef<HTMLDivElement>(null)
  useEffect(() => { end.current?.scrollIntoView({ behavior: 'smooth', block: 'end' }) }, [segments.length])

  if (segments.length === 0) {
    return (
      <div className="empty">
        {live
          ? <p>{speechActive ? 'Listening…' : 'Waiting for someone to speak.'}</p>
          : <><p>Join your call, then press <b>Start listening</b>.</p>
              <p className="quiet">Everything runs on this machine. Nothing is uploaded.</p></>}
      </div>
    )
  }

  return (
    <div className="transcript">
      {segments.map(seg => {
        const studyIsTarget = seg.target_lang === learning
        const study = studyIsTarget ? seg.target_text : seg.source_text
        const gloss = studyIsTarget ? seg.source_text : seg.target_text
        return (
          <article key={seg.id} className={`seg ${seg.role}`} style={{ ['--spk' as string]: speakerColor(seg.speaker.id) }}>
            <header>
              <span className="who">{seg.speaker.label}</span>
              <time>{t(seg.started_ms)}</time>
              {seg.speaker.confidence < 0.6 && seg.role === 'remote' && <span className="unsure" title="Not sure who this was">?</span>}
            </header>
            <p className="study" lang={learning}>
              {seg.tokens.length
                ? seg.tokens.map((tok, i) => <Word key={i} token={tok} lang={learning} />)
                : study}
            </p>
            <p className="gloss">{gloss}</p>
            <footer>
              {seg.clip_path && <button onClick={() => api.playClip(seg.clip_path!)}>Replay original</button>}
              <button onClick={() => api.speak(study, learning)}>Hear it</button>
              <button onClick={() => api.speak(study, learning, 0.75)}>Slower</button>
            </footer>
          </article>
        )
      })}
      <div ref={end} />
    </div>
  )
}
