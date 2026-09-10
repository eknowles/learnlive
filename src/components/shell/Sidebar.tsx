import { useRef, type KeyboardEvent, type PointerEvent } from 'react'
import { SIDEBAR_MAX, SIDEBAR_MIN } from '../../hooks/useLayout'
import { clock } from '../../lib/time'
import type { MeetingSummary } from '../../lib/types'
import Button from '../ui/Button'
import Icon from '../ui/Icon'

export type Selection = { kind: 'live' } | { kind: 'meeting'; id: number }

interface Props {
  meetings: MeetingSummary[]
  selection: Selection
  onSelect: (s: Selection) => void
  live: boolean
  onToggle: () => void
  onResize: (width: number) => void
}

const sameSel = (a: Selection, b: Selection) => a.kind === b.kind && (a.kind !== 'meeting' || b.kind !== 'meeting' || a.id === b.id)

/** "Today", "Yesterday", then weekday for the last week, then a date. */
const dayLabel = (s: number) => {
  const d = new Date(s * 1000)
  const today = new Date()
  const days = Math.round((startOfDay(today) - startOfDay(d)) / 86_400_000)
  if (days === 0) return 'Today'
  if (days === 1) return 'Yesterday'
  if (days < 7) return d.toLocaleDateString([], { weekday: 'long' })
  return d.toLocaleDateString([], { day: 'numeric', month: 'long', year: d.getFullYear() === today.getFullYear() ? undefined : 'numeric' })
}
const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
export const duration = (m: MeetingSummary) =>
  m.ended_at ? `${Math.max(1, Math.round((m.ended_at - m.started_at) / 60))} min` : 'in progress'

/** Source list: the live session first, then every past meeting grouped by day. */
export default function Sidebar({ meetings, selection, onSelect, live, onToggle, onResize }: Props) {
  const list = useRef<HTMLDivElement>(null)
  const options: Selection[] = [{ kind: 'live' }, ...meetings.map(m => ({ kind: 'meeting' as const, id: m.id }))]

  const onKey = (e: KeyboardEvent) => {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return
    e.preventDefault()
    const i = options.findIndex(o => sameSel(o, selection))
    const next = options[Math.min(options.length - 1, Math.max(0, i + (e.key === 'ArrowDown' ? 1 : -1)))]
    if (next && !sameSel(next, selection)) onSelect(next)
  }

  const startResize = (e: PointerEvent<HTMLDivElement>) => {
    const el = list.current?.parentElement?.parentElement
    if (!el) return
    e.currentTarget.setPointerCapture(e.pointerId)
    const left = el.getBoundingClientRect().left
    const move = (ev: globalThis.PointerEvent) => onResize(Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, ev.clientX - left)))
    const up = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
  }

  const groups: { day: string; items: MeetingSummary[] }[] = []
  for (const m of meetings) {
    const day = dayLabel(m.started_at)
    const g = groups[groups.length - 1]
    if (g?.day === day) g.items.push(m)
    else groups.push({ day, items: [m] })
  }

  return (
    <aside className="sidebar" aria-label="Sidebar">
      <div className="sidebar-inner">
        <div className="sidebar-head" data-tauri-drag-region>
          <Button variant="toolbar" icon="sidebar.left" aria-label="Hide Sidebar" title="Hide Sidebar (⌃⌘S)" onClick={onToggle} />
        </div>
        <div className="sidebar-list" role="listbox" aria-label="Sessions" tabIndex={0} onKeyDown={onKey} ref={list}>
          <button
            type="button"
            role="option"
            className="sb-row"
            aria-selected={selection.kind === 'live'}
            onClick={() => onSelect({ kind: 'live' })}
          >
            <Icon name="waveform" />
            <span className="sb-title">Live</span>
            {live && <span className="live-dot" aria-label="Listening" />}
          </button>

          <div className="sb-section">Past Meetings</div>
          {meetings.length === 0 && <p className="sb-empty">Sessions you record appear here.</p>}
          {groups.map(g => (
            <div key={g.day}>
              <div className="sb-day">{g.day}</div>
              {g.items.map(m => (
                <button
                  key={m.id}
                  type="button"
                  role="option"
                  className="sb-row sb-meeting"
                  aria-selected={selection.kind === 'meeting' && selection.id === m.id}
                  onClick={() => onSelect({ kind: 'meeting', id: m.id })}
                >
                  <span className="sb-title">{m.title}</span>
                  <span className="sb-sub">
                    {clock(m.started_at * 1000).slice(0, 5)} · {duration(m)} · {m.sentence_count} sentences
                  </span>
                </button>
              ))}
            </div>
          ))}
        </div>
      </div>
      <div className="sb-resizer" onPointerDown={startResize} aria-hidden />
    </aside>
  )
}
