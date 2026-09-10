// Universal Dependencies POS → colour + plain-English name. Function words stay grey so
// content words carry the colour.
export const POS: Record<string, { name: string; color: string }> = {
  NOUN: { name: 'noun', color: '#3A6EA5' },
  PROPN: { name: 'name', color: '#3A6EA5' },
  VERB: { name: 'verb', color: '#B5452B' },
  AUX: { name: 'auxiliary', color: '#B5452B' },
  ADJ: { name: 'adjective', color: '#4F7F3A' },
  ADV: { name: 'adverb', color: '#B8860B' },
  PRON: { name: 'pronoun', color: '#7A5AA8' },
  NUM: { name: 'number', color: '#7A5AA8' },
  ADP: { name: 'preposition', color: '#7E8A94' },
  DET: { name: 'determiner', color: '#7E8A94' },
  CCONJ: { name: 'conjunction', color: '#7E8A94' },
  SCONJ: { name: 'conjunction', color: '#7E8A94' },
  PART: { name: 'particle', color: '#7E8A94' },
  INTJ: { name: 'interjection', color: '#7E8A94' },
  PUNCT: { name: '', color: 'transparent' },
  X: { name: 'other', color: '#7E8A94' },
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

export const SPEAKER_COLORS = ['#0F766E', '#7C3AED', '#C2410C', '#0369A1', '#BE185D', '#4D7C0F']
export const speakerColor = (id: number) => (id === 0 ? '#1B2530' : SPEAKER_COLORS[(id - 1) % SPEAKER_COLORS.length])
