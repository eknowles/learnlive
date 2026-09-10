import { useEffect, useMemo, useRef, useState } from 'react'
import { api } from '../lib/api'
import { speakerColor } from '../lib/pos'
import { ago, clock, gap } from '../lib/time'
import type { Segment, Token } from '../lib/types'
import Word from './Word'
import { changedIndices as changed } from '../lib/diff'

interface Props {
  segments: Segment[]
  learning: string
  live: boolean
  speechActive: boolean
}

export default function Transcript({ segments, learning, live, speechActive }: Props) {
  const scroller = useRef<HTMLElement | null>(null)
  const end = useRef<HTMLDivElement>(null)
  const [pinned, setPinned] = useState(true) // following the live edge?
  const [unseen, setUnseen] = useState(0)
  const [now, setNow] = useState(Date.now())
  const prevTokens = useRef<Map<string, Token[]>>(new Map())
  const flash = useRef<Map<string, Set<number>>>(new Map())

  // Tick once a second so "3 min ago" stays honest.
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(t)
  }, [])

  // Diff each revision against the last one we rendered.
  useMemo(() => {
    for (const s of segments) {
      const prev = prevTokens.current.get(s.id)
      if (prev !== s.tokens) {
        flash.current.set(s.id, changed(prev, s.tokens))
        prevTokens.current.set(s.id, s.tokens)
      }
    }
  }, [segments])

  // Find the scroll container once mounted.
  useEffect(() => {
    scroller.current = end.current?.closest('.stage') as HTMLElement | null
  }, [])

  // Only follow the live edge if the reader is already there.
  useEffect(() => {
    const el = scroller.current
    if (!el) return
    const onScroll = () => {
      const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80
      setPinned(atBottom)
      if (atBottom) setUnseen(0)
    }
    el.addEventListener('scroll', onScroll, { passive: true })
    return () => el.removeEventListener('scroll', onScroll)
  }, [])

  const lastId = segments[segments.length - 1]?.id
  useEffect(() => {
    if (pinned) end.current?.scrollIntoView({ behavior: 'smooth', block: 'end' })
    else setUnseen(n => n + 1)
  }, [lastId])

  const jump = () => {
    end.current?.scrollIntoView({ behavior: 'smooth', block: 'end' })
    setPinned(true)
    setUnseen(0)
  }

  if (segments.length === 0) {
    return (
      <div className="empty">
        {live ? (
          <p>{speechActive ? 'Listening…' : 'Waiting for someone to speak.'}</p>
        ) : (
          <>
            <p>
              Join your call, then press <b>Start listening</b>.
            </p>
            <p className="quiet">Everything runs on this machine. Nothing is uploaded.</p>
          </>
        )}
      </div>
    )
  }

  return (
    <div className="transcript">
      {segments.map((seg, i) => {
        const studyIsTarget = seg.target_lang === learning
        const study = studyIsTarget ? seg.target_text : seg.source_text
        const gloss = studyIsTarget ? seg.source_text : seg.target_text
        const pause = i > 0 ? gap(segments[i - 1].ended_ms, seg.started_ms) : null
        const hot = flash.current.get(seg.id) ?? new Set<number>()
        return (
          <div key={seg.id}>
            {pause && (
              <div className="pause" aria-label={pause}>
                {pause}
              </div>
            )}
            <article
              className={`seg ${seg.role} ${seg.final ? '' : 'draft'}`}
              style={{ ['--spk' as string]: speakerColor(seg.speaker.id) }}
            >
              <header>
                <span className="who">{seg.speaker.label}</span>
                <time dateTime={new Date(seg.arrived_at).toISOString()} title={clock(seg.arrived_at)}>
                  {clock(seg.arrived_at)}
                </time>
                <span className="ago">{ago(seg.arrived_at, now)}</span>
                {!seg.final && <span className="draft-tag">still talking…</span>}
                {seg.speaker.confidence < 0.6 && seg.role === 'remote' && (
                  <span className="unsure" title="Not sure who this was">
                    ?
                  </span>
                )}
              </header>
              <p className="study" lang={learning}>
                {seg.tokens.length
                  ? seg.tokens.map((tok, j) => <Word key={`${seg.revision}-${j}`} token={tok} lang={learning} changed={hot.has(j)} />)
                  : study}
              </p>
              <p className="gloss">{gloss}</p>
              {seg.final && (
                <footer>
                  {seg.clip_path && <button onClick={() => api.playClip(seg.clip_path!)}>Replay original</button>}
                  <button onClick={() => api.speak(study, learning)}>Hear it</button>
                  <button onClick={() => api.speak(study, learning, 0.75)}>Slower</button>
                </footer>
              )}
            </article>
          </div>
        )
      })}
      <div ref={end} />
      {!pinned && unseen > 0 && (
        <button className="jump" onClick={jump}>
          {unseen} new · back to now
        </button>
      )}
    </div>
  )
}
