// Mock of Tauri internals so the UI renders outside the app, with plausible data.
(() => {
  const now = Math.floor(Date.now() / 1000)
  const h = 3600, d = 86400
  const tok = (text, pos, lemma, feats = {}) => ({ text, lemma, pos, feats, aligned_to: null })
  const seg = (i, spk, label, src, tgt, tokens, role = 'remote', final = true, mins = 0) => ({
    id: 's' + i, speaker: { id: spk, label, confidence: spk === 2 ? 0.5 : 0.9 }, role,
    started_ms: i * 9000, ended_ms: i * 9000 + 4000, source_lang: role === 'local' ? 'en' : 'ru',
    source_text: src, target_lang: role === 'local' ? 'ru' : 'en', target_text: tgt, tokens, clip_path: final ? '/clip' + i : null,
    final, revision: 2, arrived_at: Date.now() - (6 - i) * 60000 - mins,
  })
  const segments = [
    seg(1, 1, 'Anna', 'Вчера я читала интересную книгу о путешествиях.', 'Yesterday I was reading an interesting book about travel.', []),
    seg(2, 0, 'You', 'Which book was it?', 'Какая это была книга?', [tok('Какая','DET','какой',{Case:'Nom',Gender:'Fem'}), tok('это','PRON','это'), tok('была','AUX','быть',{Tense:'Past',Gender:'Fem'}), tok('книга','NOUN','книга',{Case:'Nom',Number:'Sing',Gender:'Fem'}), tok('?','PUNCT','?')], 'local'),
    seg(3, 1, 'Anna', 'Она называется «Тихий Дон», но это скорее роман.', 'It’s called “Quiet Flows the Don”, but it’s more of a novel.', []),
    seg(4, 2, 'Speaker 2', 'Я тоже хотел бы её прочитать.', 'I would like to read it too.', []),
    seg(5, 1, 'Anna', 'Мы можем обсудить первую главу на следующей', 'We can discuss the first chapter next', [], 'remote', false),
  ]
  const meeting = (id, title, start, mins, n, parts = []) => ({ id, title, started_at: start, ended_at: start + mins * 60, learning: 'ru', native: 'en', calendar_event_id: parts.length ? 'ev' + id : null, participants: parts, sentence_count: n })
  const meetings = [
    meeting(7, 'Russian lesson with Anna', now - 2 * h, 48, 213, [{ id: 1, name: 'Anna Petrova', email: 'anna@example.com', speaker_id: 1, has_voiceprint: true }]),
    meeting(6, 'Session Wed 10 Sep 08:15', now - 5 * h, 12, 31),
    meeting(5, 'Weekly sync · Product', now - d - 3 * h, 55, 402, [{ id: 2, name: 'Dmitri Volkov', email: 'd@example.com', speaker_id: null, has_voiceprint: false }, { id: 3, name: 'Olga S.', email: null, speaker_id: 2, has_voiceprint: false }]),
    meeting(4, 'Conversation practice', now - 3 * d, 30, 140),
    meeting(3, 'Interview prep', now - 9 * d, 25, 96),
    meeting(2, 'Session Tue 26 Aug 19:02', now - 15 * d, 41, 187),
  ]
  const hits = segments.slice(0, 3).map(s => ({ meeting_id: 7, meeting_title: 'Russian lesson with Anna', started_at: now - 2 * h, segment: s }))
  // Enough of a dictionary for the hover popover to look like itself in a preview.
  const GLOSS = {
    'вчера': 'yesterday', 'я': 'I', 'читала': 'was reading', 'интересную': 'interesting',
    'книгу': 'book', 'о': 'about', 'путешествиях': 'travels', 'она': 'it', 'называется': 'is called',
    'тихий': 'quiet', 'дон': 'Don', 'но': 'but', 'это': 'this', 'скорее': 'rather', 'роман': 'novel',
    'тоже': 'too', 'хотел': 'wanted', 'бы': 'would', 'её': 'it', 'прочитать': 'to read',
    'мы': 'we', 'можем': 'can', 'обсудить': 'discuss', 'первую': 'first', 'главу': 'chapter',
    'на': 'on', 'следующей': 'next', 'какая': 'which', 'была': 'was', 'книга': 'book',
  }
  const listeners = {}
  const responses = {
    list_audio_devices: () => [
      { id: 'bh', name: 'BlackHole 2ch', input_channels: 2, default_sample_rate: 48000, is_loopback: true },
      { id: 'mic', name: 'MacBook Pro Microphone', input_channels: 1, default_sample_rate: 48000, is_loopback: false },
      { id: 'air', name: 'AirPods Pro', input_channels: 1, default_sample_rate: 24000, is_loopback: false },
    ],
    list_languages: () => [
      { code: 'ru', name: 'Russian', native: 'Русский', nllb: 'rus_Cyrl', tts: true, grammar: true },
      { code: 'en', name: 'English', native: 'English', nllb: 'eng_Latn', tts: true, grammar: true },
      { code: 'de', name: 'German', native: 'Deutsch', nllb: 'deu_Latn', tts: true, grammar: false },
      { code: 'es', name: 'Spanish', native: 'Español', nllb: 'spa_Latn', tts: true, grammar: false },
    ],
    model_status: () => [{ id: 'whisper-small', kind: 'asr', present: false, approx_mb: 466 }, { id: 'nllb-200-600M', kind: 'mt', present: true, approx_mb: 620 }],
    list_meetings: () => meetings,
    get_meeting: a => [meetings.find(m => m.id === a.id), segments],
    search_history: () => hits,
    calendar_events_near_now: () => [{ id: 'ev7', title: 'Russian lesson with Anna', start: now - 600, end: now + 3000, attendees: [{ name: 'Anna Petrova', email: 'anna@example.com' }], url: null, calendar: 'Personal' }],
    get_remember_voices: () => true,
    translate_word: a => ({ word: a.word, translation: GLOSS[a.word.toLowerCase()] ?? '(' + a.word + ')', aligned_to: null }),
    start_session: () => 7,
    'plugin:event|listen': (a, cbId) => { listeners[a.event] = cbId; return 1 },
  }
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: location.hash === '#settings' ? 'settings' : 'main' }, currentWebview: { label: 'main' }, windows: [], webviews: [] },
    transformCallback: (cb, once) => { const id = Math.floor(Math.random() * 1e9); window['_' + id] = cb; return id },
    invoke: (cmd, args = {}) => {
      const r = responses[cmd]
      const cbId = args && args.handler
      return Promise.resolve(r ? r(args, cbId) : null)
    },
    convertFileSrc: p => p,
  }
  window.__mock = {
    emit: (event, payload) => { const id = listeners[event]; if (id != null) window['_' + id]({ event, id: 1, payload }) },
    segments,
  }
})()
