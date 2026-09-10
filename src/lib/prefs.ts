import { emit, listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { DEFAULT_CONFIG } from './config'
import type { SessionConfig } from './types'

/** Session config survives relaunch and is shared with the Settings window.
 *  localStorage is the store (both windows are the same origin); a `config-changed` event
 *  tells the other window to re-read, tagged with the sender so nobody echoes its own write. */
const KEY = 'learnlive.config.v1'
const label = () => getCurrentWindow().label

export const loadConfig = (): SessionConfig => {
  try {
    const raw = localStorage.getItem(KEY)
    if (raw) return { ...DEFAULT_CONFIG, ...(JSON.parse(raw) as Partial<SessionConfig>) }
  } catch {
    /* corrupt or missing: fall through to defaults */
  }
  return DEFAULT_CONFIG
}

export const saveConfig = (cfg: SessionConfig) => {
  localStorage.setItem(KEY, JSON.stringify(cfg))
  void emit('config-changed', { from: label(), cfg })
}

export const onConfigChanged = (cb: (cfg: SessionConfig) => void) =>
  listen<{ from: string; cfg: SessionConfig }>('config-changed', e => {
    if (e.payload.from !== label()) cb(e.payload.cfg)
  })

/** Small UI state (sidebar/inspector visibility, widths) — per window, never synced. */
export const loadJson = <T>(key: string, fallback: T): T => {
  try {
    const raw = localStorage.getItem(key)
    return raw ? { ...fallback, ...(JSON.parse(raw) as Partial<T>) } : fallback
  } catch {
    return fallback
  }
}
export const saveJson = (key: string, value: unknown) => localStorage.setItem(key, JSON.stringify(value))
