import { useEffect, useRef, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import Sidebar, { duration, type Selection } from './components/shell/Sidebar'
import Toolbar from './components/shell/Toolbar'
import Inspector from './components/shell/Inspector'
import SessionPanel from './components/panels/SessionPanel'
import MixerPanel from './components/panels/MixerPanel'
import SpeakersPanel from './components/panels/SpeakersPanel'
import MeetingPanel from './components/panels/MeetingPanel'
import DetailsPanel from './components/panels/DetailsPanel'
import LiveView from './components/views/LiveView'
import MeetingView from './components/views/MeetingView'
import SearchResults from './components/views/SearchResults'
import Banner from './components/ui/Banner'
import Button from './components/ui/Button'
import { useHistory } from './hooks/useHistory'
import { useLayout } from './hooks/useLayout'
import { useMenu } from './hooks/useMenu'
import { useReading } from './hooks/useReading'
import { useSession } from './hooks/useSession'
import { useTranscript } from './hooks/useTranscript'
import { useWindowFocus } from './hooks/useWindowFocus'
import { confirmDeleteMeeting } from './lib/dialogs'
import { clock } from './lib/time'

const day = (s: number) => new Date(s * 1000).toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' })

export default function App() {
  useWindowFocus()
  const layout = useLayout()
  const reading = useReading()
  const transcript = useTranscript()
  const history = useHistory()
  const [selection, setSelection] = useState<Selection>({ kind: 'live' })
  const s = useSession(
    () => {
      transcript.reset()
      setSelection({ kind: 'live' })
    },
    () => history.refresh(),
  )
  const searchRef = useRef<HTMLInputElement>(null)
  const contentRef = useRef<HTMLDivElement>(null)
  const [scrolled, setScrolled] = useState(false)

  const select = async (next: Selection) => {
    if (next.kind === 'meeting') {
      if (!(await history.openMeeting(next.id))) return
    } else history.close()
    history.setQuery('')
    setSelection(next)
  }

  useMenu({
    start: s.start,
    stop: s.stop,
    find: () => searchRef.current?.focus(),
    'go-live': () => select({ kind: 'live' }),
    'toggle-sidebar': layout.toggleSidebar,
    'toggle-inspector': layout.toggleInspector,
    'cycle-langs': reading.cycle,
  })

  // Window title for Mission Control / the Window menu; the title bar itself is hidden.
  const open = selection.kind === 'meeting' ? history.open : null
  useEffect(() => {
    getCurrentWindow().setTitle(open ? `${open.meeting.title} — LearnLive` : 'LearnLive')
  }, [open?.meeting.title])

  // Toolbar shows its bottom hairline only once content has scrolled under it.
  useEffect(() => {
    const el = contentRef.current
    if (!el) return
    const onScroll = () => setScrolled(el.scrollTop > 0)
    el.addEventListener('scroll', onScroll, { passive: true })
    return () => el.removeEventListener('scroll', onScroll)
  }, [])

  const remove = async (id: number, title: string) => {
    if (!(await confirmDeleteMeeting(title))) return
    await history.remove(id)
    setSelection({ kind: 'live' })
  }
  const reload = () => history.openMeeting(open!.meeting.id)

  const live = s.phase === 'live'
  const canStart = s.cfg.sources.length > 0
  const searching = history.query.trim().length > 0
  const missingMb = s.models.filter(m => !m.present).reduce((a, m) => a + m.approx_mb, 0)

  const title = searching ? 'Search' : open ? open.meeting.title : live ? (s.meeting?.title ?? 'Listening') : 'Live'
  const subtitle = searching
    ? `${history.hits.length} ${history.hits.length === 1 ? 'match' : 'matches'} across all meetings`
    : open
      ? `${day(open.meeting.started_at)} · ${clock(open.meeting.started_at * 1000).slice(0, 5)} · ${duration(open.meeting)}`
      : s.phase === 'downloading'
        ? 'Downloading models…'
        : s.phase === 'starting'
          ? 'Loading models…'
          : live
            ? s.levels?.speech_active
              ? 'Someone is speaking'
              : 'Listening'
            : missingMb
              ? `Ready · first start downloads ${missingMb} MB`
              : 'Ready'

  return (
    <div
      className="window"
      data-sidebar={layout.sidebar}
      data-inspector={layout.inspector}
      style={{ ['--sidebar-w' as string]: `${layout.sidebarWidth}px` }}
    >
      <Sidebar
        meetings={history.meetings}
        selection={selection}
        onSelect={select}
        live={live}
        onToggle={layout.toggleSidebar}
        onResize={layout.setSidebarWidth}
      />
      <div className="main" data-scrolled={scrolled}>
        <Toolbar
          title={title}
          subtitle={subtitle}
          sidebarHidden={!layout.sidebar}
          onShowSidebar={layout.toggleSidebar}
          phase={s.phase}
          canStart={canStart}
          onStart={s.start}
          onStop={s.stop}
          query={history.query}
          setQuery={history.setQuery}
          searchRef={searchRef}
          inspectorOpen={layout.inspector}
          onToggleInspector={layout.toggleInspector}
          mode={searching ? undefined : reading.reading.mode}
          onMode={m => reading.update({ mode: m })}
        >
          {open && !searching && (
            <Button
              variant="toolbar"
              icon="trash"
              aria-label="Delete Meeting"
              title="Delete Meeting"
              onClick={() => remove(open.meeting.id, open.meeting.title)}
            />
          )}
        </Toolbar>
        <div className="body">
          <div className="content">
            {s.error && <Banner error={s.error} onDismiss={s.dismissError} />}
            <main className="scroller" data-scroller ref={contentRef}>
              {searching ? (
                <SearchResults hits={history.hits} query={history.query} onOpen={id => select({ kind: 'meeting', id })} />
              ) : open ? (
                <MeetingView meeting={open.meeting} segments={open.segments} mode={reading.reading.mode} />
              ) : (
                <LiveView
                  phase={s.phase}
                  segments={transcript.segments}
                  learning={s.cfg.learning}
                  speechActive={s.levels?.speech_active ?? false}
                  models={s.models}
                  progress={s.progress}
                  canStart={canStart}
                  mode={reading.reading.mode}
                />
              )}
            </main>
          </div>
          <Inspector>
            {open ? (
              <DetailsPanel
                meeting={open.meeting}
                segments={open.segments}
                languages={s.languages}
                onRename={() => {}}
                onMeeting={reload}
              />
            ) : (
              <>
                <SessionPanel cfg={s.cfg} setCfg={s.setCfg} devices={s.devices} languages={s.languages} locked={s.phase !== 'setup'} />
                <MeetingPanel
                  live={live}
                  meeting={s.meeting}
                  pending={s.pendingEvent}
                  onPending={s.setPendingEvent}
                  onLinked={s.setMeeting}
                />
                {live && <MixerPanel cfg={s.cfg} setCfg={s.setCfg} devices={s.devices} levels={s.levels} />}
                <SpeakersPanel segments={transcript.segments} meeting={s.meeting} onRename={transcript.rename} onMeeting={s.setMeeting} />
              </>
            )}
          </Inspector>
        </div>
      </div>
    </div>
  )
}
