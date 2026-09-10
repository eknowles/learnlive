import { useEffect, useMemo, useRef, useState } from 'react'
import { api } from '../lib/api'
import { showSegmentMenu } from '../lib/contextMenu'
import { speakerColor } from '../lib/pos'
import { ago, clock, gap } from '../lib/time'
import type { Segment, Token } from '../lib/types'
import Button from './ui/Button'
import Icon from './ui/Icon'
import Word from './Word'
import { changedIndices as changed } from '../lib/diff'

interface Props {
  segments: Segment[]
  learning: string
  live: boolean
  speechActive: boolean
}

/** The reading surface: big serif study line, quiet gloss beneath, hover cards on words. */
export default function Transcript({ segments, learning, live }: Props) {
  const scroller = useRef<HTMLElement | null>(null)
  const end = useRef<HTMLDivElement>(null)
  const [pinned, setPinned] = useState(true) // following the live edge?
  const [unseen, setUnseen] = useState(0)
  const [now, setNow] = useState(Date.now())
  const prevTokens = useRef<Map<string, Token[]>>(new Map())
  const flash = useRef<Map<string, Set<number>>>(new Map())

  // Tick once a second so "3 min ago" stays honest.
  useEffect(() => {
    if (!live) return
    const t = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(t)
  }, [live])

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
    scroller.current = end.current?.closest('[data-scroller]') as HTMLElement | null
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
    if (!live) return
    if (pinned) end.current?.scrollIntoView({ behavior: 'smooth', block: 'end' })
    else setUnseen(n => n + 1)
  }, [lastId])

  const jump = () => {
    end.current?.scrollIntoView({ behavior: 'smooth', block: 'end' })
    setPinned(true)
    setUnseen(0)
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
              onContextMenu={e => {
                e.preventDefault()
                showSegmentMenu(seg, study, gloss, learning)
              }}
            >
              <header>
                <span className="who">{seg.speaker.label}</span>
                <time dateTime={new Date(seg.arrived_at).toISOString()} title={clock(seg.arrived_at)}>
                  {clock(seg.arrived_at).slice(0, 5)}
                </time>
                {live && <span className="ago">{ago(seg.arrived_at, now)}</span>}
                {!seg.final && <span className="draft-tag">still talking…</span>}
                {seg.speaker.confidence < 0.6 && seg.role === 'remote' && (
                  <span className="unsure" title="Not sure who this was">
                    unsure who
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
                  <Button variant="plain" icon="speaker" onClick={() => api.speak(study, learning)}>
                    Hear
                  </Button>
                  <Button variant="plain" onClick={() => api.speak(study, learning, 0.75)}>
                    Slower
                  </Button>
                  {seg.clip_path && (
                    <Button variant="plain" onClick={() => api.playClip(seg.clip_path!)}>
                      Original
                    </Button>
                  )}
                </footer>
              )}
            </article>
          </div>
        )
      })}
      <div ref={end} />
      {live && !pinned && unseen > 0 && (
        <button type="button" className="jump" onClick={jump}>
          <Icon name="arrow.down" size={12} />
          {unseen} new
        </button>
      )}
    </div>
  )
}
