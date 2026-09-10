import type { ReactNode, RefObject } from 'react'
import type { Phase } from '../../hooks/useSession'
import Button from '../ui/Button'
import SearchField from '../ui/SearchField'

interface Props {
  title: string
  subtitle?: string
  sidebarHidden: boolean
  onShowSidebar: () => void
  phase: Phase
  canStart: boolean
  onStart: () => void
  onStop: () => void
  query: string
  setQuery: (q: string) => void
  searchRef: RefObject<HTMLInputElement>
  inspectorOpen: boolean
  onToggleInspector: () => void
  children?: ReactNode
}

/** Unified toolbar: title on the left, actions on the right, the whole strip drags the window. */
export default function Toolbar(p: Props) {
  const busy = p.phase === 'downloading' || p.phase === 'starting'
  return (
    <header className="toolbar" data-tauri-drag-region>
      {p.sidebarHidden && (
        <Button variant="toolbar" icon="sidebar.left" aria-label="Show Sidebar" title="Show Sidebar (⌃⌘S)" onClick={p.onShowSidebar} />
      )}
      <div className="tb-title" data-tauri-drag-region>
        <h1 data-tauri-drag-region>{p.title}</h1>
        {p.subtitle && <span data-tauri-drag-region>{p.subtitle}</span>}
      </div>
      <div className="tb-group">
        {p.children}
        {/* Keyed so Start→Stop remounts: WebKit will not transition between two system-colour keywords. */}
        {p.phase === 'live' ? (
          <Button key="stop" variant="toolbar-stop" icon="stop" onClick={p.onStop} title="Stop Listening (⌘.)">
            Stop
          </Button>
        ) : (
          <Button
            key="start"
            variant="toolbar-primary"
            icon="waveform"
            onClick={p.onStart}
            disabled={busy || !p.canStart}
            title="Start Listening (⌘R)"
          >
            {p.phase === 'downloading' ? 'Downloading…' : p.phase === 'starting' ? 'Loading…' : 'Start Listening'}
          </Button>
        )}
        <SearchField ref={p.searchRef} value={p.query} onChange={p.setQuery} placeholder="Search" aria-label="Search all meetings" />
        <Button
          variant="toolbar"
          icon="sidebar.right"
          aria-label={p.inspectorOpen ? 'Hide Inspector' : 'Show Inspector'}
          title="Toggle Inspector (⌥⌘I)"
          onClick={p.onToggleInspector}
        />
      </div>
    </header>
  )
}
