// Universal Dependencies POS → colour + plain-English name. Function words stay grey so
// content words carry the colour. Colours are the macOS system palette (WebKit keywords), so
// they adapt to light/dark and increased contrast like every other control.
const blue = '-apple-system-blue'
const red = '-apple-system-red'
const green = '-apple-system-green'
const orange = '-apple-system-orange'
const purple = '-apple-system-purple'
const gray = '-apple-system-gray'

export const POS: Record<string, { name: string; color: string }> = {
  NOUN: { name: 'noun', color: blue },
  PROPN: { name: 'name', color: blue },
  VERB: { name: 'verb', color: red },
  AUX: { name: 'auxiliary', color: red },
  ADJ: { name: 'adjective', color: green },
  ADV: { name: 'adverb', color: orange },
  PRON: { name: 'pronoun', color: purple },
  NUM: { name: 'number', color: purple },
  ADP: { name: 'preposition', color: gray },
  DET: { name: 'determiner', color: gray },
  CCONJ: { name: 'conjunction', color: gray },
  SCONJ: { name: 'conjunction', color: gray },
  PART: { name: 'particle', color: gray },
  INTJ: { name: 'interjection', color: gray },
  PUNCT: { name: '', color: 'transparent' },
  X: { name: 'other', color: gray },
}

export const FEATS: Record<string, Record<string, string>> = {
  Case: {
    Nom: 'nominative',
    Gen: 'genitive',
    Dat: 'dative',
    Acc: 'accusative',
    Ins: 'instrumental',
    Loc: 'prepositional',
    Voc: 'vocative',
  },
  Number: { Sing: 'singular', Plur: 'plural' },
  Gender: { Masc: 'masculine', Fem: 'feminine', Neut: 'neuter' },
  Tense: { Past: 'past', Pres: 'present', Fut: 'future' },
  Aspect: { Perf: 'perfective', Imp: 'imperfective' },
  Person: { '1': '1st person', '2': '2nd person', '3': '3rd person' },
}

export const SPEAKER_COLORS = [
  '-apple-system-teal',
  '-apple-system-purple',
  '-apple-system-orange',
  '-apple-system-blue',
  '-apple-system-pink',
  '-apple-system-green',
]
/** Speaker 0 is you: plain label colour, so the other voices carry the colour. */
export const speakerColor = (id: number) => (id === 0 ? 'var(--label)' : SPEAKER_COLORS[(id - 1) % SPEAKER_COLORS.length])
