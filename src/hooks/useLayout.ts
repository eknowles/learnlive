import { useEffect, useState } from 'react'
import { loadJson, saveJson } from '../lib/prefs'

interface Layout {
  sidebar: boolean
  inspector: boolean
  sidebarWidth: number
}
const KEY = 'learnlive.layout.v1'
const DEFAULT: Layout = { sidebar: true, inspector: true, sidebarWidth: 240 }
export const SIDEBAR_MIN = 200
export const SIDEBAR_MAX = 340

/** Sidebar / inspector visibility and the sidebar width, remembered like a real split view. */
export function useLayout() {
  const [layout, setLayout] = useState<Layout>(() => loadJson(KEY, DEFAULT))
  useEffect(() => saveJson(KEY, layout), [layout])
  return {
    ...layout,
    toggleSidebar: () => setLayout(l => ({ ...l, sidebar: !l.sidebar })),
    toggleInspector: () => setLayout(l => ({ ...l, inspector: !l.inspector })),
    showInspector: () => setLayout(l => ({ ...l, inspector: true })),
    setSidebarWidth: (w: number) => setLayout(l => ({ ...l, sidebarWidth: Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, Math.round(w))) })),
  }
}
