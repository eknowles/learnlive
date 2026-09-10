import { Menu, PredefinedMenuItem } from '@tauri-apps/api/menu'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { api } from './api'
import type { Segment } from './types'

/** Right-click on a sentence: a real NSMenu, not a styled div. */
export async function showSegmentMenu(seg: Segment, study: string, gloss: string, lang: string) {
  const menu = await Menu.new({
    items: [
      { id: 'hear', text: 'Hear It', action: () => api.speak(study, lang) },
      { id: 'slow', text: 'Hear It Slowly', action: () => api.speak(study, lang, 0.75) },
      ...(seg.clip_path ? [{ id: 'replay', text: 'Replay Original Audio', action: () => api.playClip(seg.clip_path!) }] : []),
      await PredefinedMenuItem.new({ item: 'Separator' }),
      { id: 'copy', text: 'Copy Sentence', action: () => writeText(study) },
      { id: 'copy-gloss', text: 'Copy Translation', action: () => writeText(gloss) },
      { id: 'copy-both', text: 'Copy Both', action: () => writeText(`${study}\n${gloss}`) },
    ],
  })
  await menu.popup()
}
