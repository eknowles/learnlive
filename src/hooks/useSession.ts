import { useEffect, useState } from 'react'
import { api } from '../lib/api'
import { DEFAULT_CONFIG, defaultSources, modelKey } from '../lib/config'
import { asAppError, describe } from '../lib/errors'
import type { AudioDevice, CalendarEvent, Language, Levels, MeetingSummary, ModelProgress, ModelStatus, SessionConfig } from '../lib/types'

export type Phase = 'setup' | 'downloading' | 'starting' | 'live'

/** Everything about the current session: config, devices, models, start/stop, live levels. */
export function useSession(onStarted?: () => void) {
  const [cfg, setCfg] = useState<SessionConfig>(DEFAULT_CONFIG)
  const [devices, setDevices] = useState<AudioDevice[]>([])
  const [languages, setLanguages] = useState<Language[]>([])
  const [models, setModels] = useState<ModelStatus[]>([])
  const [progress, setProgress] = useState<Record<string, ModelProgress>>({})
  const [phase, setPhase] = useState<Phase>('setup')
  const [levels, setLevels] = useState<Levels | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [meeting, setMeeting] = useState<MeetingSummary | null>(null)
  const [pendingEvent, setPendingEvent] = useState<CalendarEvent | null>(null)

  useEffect(() => {
    api
      .devices()
      .then(ds => {
        setDevices(ds)
        setCfg(c => ({ ...c, sources: defaultSources(ds) }))
      })
      .catch(e => setError(describe(asAppError(e))))
    api.languages().then(setLanguages)
    const subs = Promise.all([
      api.onLevels(setLevels),
      api.onModelProgress(p => setProgress(prev => ({ ...prev, [p.model]: p }))),
      api.onError(m => setError(m)),
    ])
    return () => {
      subs.then(fns => fns.forEach(f => f()))
    }
  }, [])

  const key = modelKey(cfg)
  useEffect(() => {
    api
      .modelStatus(cfg)
      .then(setModels)
      .catch(() => {})
  }, [key])

  const start = async () => {
    setError(null)
    try {
      if (models.some(m => !m.present)) {
        setPhase('downloading')
        await api.prepareModels(cfg)
      }
      setPhase('starting')
      const id = await api.start(cfg, pendingEvent)
      setMeeting((await api.listMeetings(5)).find(m => m.id === id) ?? null)
      onStarted?.()
      setPhase('live')
    } catch (e) {
      setError(describe(asAppError(e)))
      setPhase('setup')
    }
  }
  const stop = async () => {
    await api.stop()
    setPhase('setup')
    setPendingEvent(null)
  }

  return {
    cfg,
    setCfg,
    devices,
    languages,
    models,
    progress,
    phase,
    levels,
    error,
    meeting,
    setMeeting,
    pendingEvent,
    setPendingEvent,
    start,
    stop,
  }
}
