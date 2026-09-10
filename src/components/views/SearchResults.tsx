import type { SearchHit } from '../../lib/types'
import Icon from '../ui/Icon'

const day = (s: number) => new Date(s * 1000).toLocaleDateString([], { day: 'numeric', month: 'short' })

/** Full-text hits across every meeting, in both languages. */
export default function SearchResults({ hits, query, onOpen }: { hits: SearchHit[]; query: string; onOpen: (meetingId: number) => void }) {
  if (hits.length === 0) {
    return (
      <div className="empty">
        <Icon name="magnifyingglass" size={40} />
        <h2>No results for “{query}”</h2>
        <p>Search matches whole words in either language; add * to match a prefix.</p>
      </div>
    )
  }
  return (
    <ul className="hits">
      {hits.map(h => {
        const studyIsSource = h.segment.target_lang === 'en'
        return (
          <li key={h.segment.id} className="hit">
            <div className="hit-meta">
              <button type="button" className="link" onClick={() => onOpen(h.meeting_id)}>
                {h.meeting_title}
              </button>
              <span>· {day(h.started_at)}</span>
              <span>· {h.segment.speaker.label}</span>
            </div>
            <p className="study selectable">{studyIsSource ? h.segment.source_text : h.segment.target_text}</p>
            <p className="gloss selectable">{studyIsSource ? h.segment.target_text : h.segment.source_text}</p>
          </li>
        )
      })}
    </ul>
  )
}
