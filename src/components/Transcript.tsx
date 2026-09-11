import { useEffect, useMemo, useRef, useState } from 'react'
import { api } from '../lib/api'
import { showSegmentMenu } from '../lib/contextMenu'
import { speakerColor } from '../lib/pos'
import { ago, clock, gap } from '../lib/time'
import { splitWords, wordsOf } from '../lib/words'
import type { LangMode } from '../lib/reading'
import type { Segment, Token } from '../lib/types'
import Button from './ui/Button'
import Icon from './ui/Icon'
import Word from './Word'
import { changedIndices as changed } from '../lib/diff'

interface Props {
  segments: Segment[]
  learning: string
  mode: LangMode
  live: boolean
  speechActive: boolean
}

/** A run of consecutive sentences from one speaker with no notable pause inside it.
 *
 *  Grouping is what stops a turn arriving as a stack of unrelated cards: the VAD cuts on
 *  breath, not on meaning, so one thought routinely spans several segments. */
interface Turn {
  key: string
  speaker: Segment['speaker']
  role: Segment['role']
  rows: Segment[]
  /** Silence before this turn, when it is long enough to be worth drawing. */
  pause: string | null
}

export const toTurns = (segments: Segment[]): Turn[] => {
  const turns: Turn[] = []
  segments.forEach((seg, i) => {
    const prev = i > 0 ? segments[i - 1] : null
    const pause = prev ? gap(prev.ended_ms, seg.started_ms) : null
    const open = turns[turns.length - 1]
    if (open && prev && !pause && prev.speaker.id === seg.speaker.id) open.rows.push(seg)
    else turns.push({ key: seg.id, speaker: seg.speaker, role: seg.role, rows: [seg], pause })
  })
  return turns
}

/** The reading surface: one block per speaker turn, the two languages side by side, every word
 *  hoverable for a translation. */
export default function Transcript({ segments, learning, mode, live }: Props) {
  const scroller = useRef<HTMLElement | null>(null)
  const end = useRef<HTMLDivElement>(null)
  const [pinned, setPinned] = useState(true) // following the live edge?
  const [unseen, setUnseen] = useState(0)
  const [now, setNow] = useState(Date.now())
  const prevWords = useRef<Map<string, string[]>>(new Map())
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
      const study = s.target_lang === learning ? s.target_text : s.source_text
      const words = s.tokens.length ? s.tokens.map(t => t.text) : wordsOf(study)
      const prev = prevWords.current.get(s.id)
      if (prev?.join(' ') !== words.join(' ')) {
        flash.current.set(s.id, changed(prev, words))
        prevWords.current.set(s.id, words)
      }
    }
  }, [segments, learning])

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

  const turns = useMemo(() => toTurns(segments), [segments])

  return (
    <div className="transcript" data-mode={mode}>
      {turns.map(turn => (
        <div key={turn.key}>
          {turn.pause && (
            <div className="pause" aria-label={turn.pause}>
              {turn.pause}
            </div>
          )}
          <section className={`turn ${turn.role}`} style={{ ['--spk' as string]: speakerColor(turn.speaker.id) }}>
            <header>
              <span className="who">{turn.speaker.label}</span>
              <time dateTime={new Date(turn.rows[0].arrived_at).toISOString()} title={clock(turn.rows[0].arrived_at)}>
                {clock(turn.rows[0].arrived_at).slice(0, 5)}
              </time>
              {live && <span className="ago">{ago(turn.rows[0].arrived_at, now)}</span>}
              {turn.speaker.confidence < 0.6 && turn.role === 'remote' && (
                <span className="unsure" title="Not sure who this was">
                  unsure who
                </span>
              )}
            </header>
            <div className="lines">
              {turn.rows.map(seg => (
                <Line key={seg.id} seg={seg} learning={learning} mode={mode} hot={flash.current.get(seg.id)} />
              ))}
            </div>
          </section>
        </div>
      ))}
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

/** One sentence: the language being learnt beside its translation. */
function Line({ seg, learning, mode, hot }: { seg: Segment; learning: string; mode: LangMode; hot?: Set<number> }) {
  const studyIsTarget = seg.target_lang === learning
  const study = studyIsTarget ? seg.target_text : seg.source_text
  const gloss = studyIsTarget ? seg.source_text : seg.target_text
  const glossLang = studyIsTarget ? seg.source_lang : seg.target_lang

  return (
    <article
      className={`line ${seg.final ? '' : 'draft'}`}
      onContextMenu={e => {
        e.preventDefault()
        showSegmentMenu(seg, study, gloss, learning)
      }}
    >
      {mode !== 'native' && (
        <p className="study" lang={learning}>
          <Words text={study} tokens={seg.tokens} lang={learning} into={glossLang} revision={seg.revision} hot={hot} />
        </p>
      )}
      {mode !== 'study' && (
        <p className="gloss" lang={glossLang}>
          {/* Hoverable only when there is no study line above to hover instead — otherwise every
              sentence carries two rows of underlines and reads as a worksheet. */}
          {mode === 'native' ? <Words text={gloss} tokens={[]} lang={glossLang} into={learning} revision={seg.revision} /> : gloss}
        </p>
      )}
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
  )
}

interface WordsProps {
  text: string
  /** From the POS tagger when one is installed; empty otherwise, and we split the text here. */
  tokens: Token[]
  lang: string
  into: string
  revision: number
  hot?: Set<number>
}

function Words({ text, tokens, lang, into, revision, hot }: WordsProps) {
  if (tokens.length) {
    // Tagger tokens exclude the whitespace between them, so the gaps come from CSS here.
    return (
      <span className="tokens">
        {tokens.map((tok, j) =>
          tok.pos === 'PUNCT' ? (
            <span className="punct" key={j}>
              {tok.text}
            </span>
          ) : (
            <Word key={`${revision}-${j}`} text={tok.text} token={tok} lang={lang} into={into} sentence={text} changed={hot?.has(j)} />
          ),
        )}
      </span>
    )
  }
  let w = 0
  return (
    <>
      {splitWords(text).map((span, j) => {
        if (!span.word) return <span key={j}>{span.text}</span>
        const i = w++
        return <Word key={`${revision}-${j}`} text={span.text} lang={lang} into={into} sentence={text} changed={hot?.has(i)} />
      })}
    </>
  )
}
