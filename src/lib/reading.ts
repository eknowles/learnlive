/** How the conversation log reads: which languages are shown, and in what type. */

export type LangMode = 'both' | 'study' | 'native'
export type FontChoice = 'serif' | 'sans' | 'mono'

export interface ReadingPrefs {
  mode: LangMode
  studyFont: FontChoice
  studySize: number
  glossFont: FontChoice
  glossSize: number
}

export const DEFAULT_READING: ReadingPrefs = {
  mode: 'both',
  studyFont: 'serif',
  studySize: 21,
  glossFont: 'sans',
  glossSize: 14,
}

/** The three families already in the design tokens, so a choice still follows light/dark. */
export const FONT_STACKS: Record<FontChoice, string> = {
  serif: 'var(--font-reading)',
  sans: 'var(--font-ui)',
  mono: 'var(--font-mono)',
}

export const FONT_NAMES: Record<FontChoice, string> = { serif: 'Serif', sans: 'Sans', mono: 'Mono' }

export const SIZE_RANGE = { study: [15, 34] as const, gloss: [11, 26] as const }

export const MODES: { id: LangMode; label: string; title: string }[] = [
  { id: 'both', label: 'Both', title: 'Show both languages side by side' },
  { id: 'study', label: 'Learning', title: 'Show only the language you are learning' },
  { id: 'native', label: 'Native', title: 'Show only your own language' },
]

export const nextMode = (m: LangMode): LangMode => (m === 'both' ? 'study' : m === 'study' ? 'native' : 'both')

/** Push the preferences onto the document as custom properties, so the stylesheet owns the
 *  layout and only the numbers come from here. */
export const applyReading = (r: ReadingPrefs) => {
  const s = document.documentElement.style
  s.setProperty('--study-font', FONT_STACKS[r.studyFont])
  s.setProperty('--study-size', `${r.studySize}px`)
  s.setProperty('--gloss-font', FONT_STACKS[r.glossFont])
  s.setProperty('--gloss-size', `${r.glossSize}px`)
}

export const isDefaultReading = (r: ReadingPrefs) =>
  (Object.keys(DEFAULT_READING) as (keyof ReadingPrefs)[]).every(k => r[k] === DEFAULT_READING[k])
