//! Session orchestration.
//!
//! `Sentencer` is the heart: it turns VAD utterances into revisable sentences and is pure
//! logic over the engine traits, so it's unit-tested with fakes. `start()` wires it to the
//! live mixer + Tauri; `run_offline()` wires it to a WAV file for tests, CI and demos.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use crossbeam_channel::{bounded, Sender};
use log::{error, info, warn};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};

use crate::audio::{MixFrame, Mixer, MixerCommand, Player, FRAME_SAMPLES};
use crate::engine::diarize::{cosine, SpeakerRegistry};
use crate::engine::vad::{Segmenter, Utterance};
use crate::engine::{Engines, Loaded};
use crate::types::{Levels, Segment, SessionConfig, SourceRole, SpeakerRef};

/// Tunables for sentence revision. Defaults are what worked for conversational speech;
/// exposed here so they can become user settings without touching the logic.
#[derive(Debug, Clone)]
pub struct SentenceOptions {
    /// A chunk from the same speaker arriving within this gap continues the open sentence.
    pub continue_gap_ms: u64,
    /// Never let one sentence grow past this; finalise and start a new one.
    pub max_sentence_ms: u64,
    /// Idle time after which an open (unfinished-looking) sentence is finalised anyway.
    pub idle_finalise_ms: u64,
    /// Shortest transcript worth showing, measured after `strip_non_speech` (filters the bare
    /// "." Whisper returns for near-silent audio).
    pub min_chars: usize,
}
impl Default for SentenceOptions {
    fn default() -> Self {
        Self { continue_gap_ms: 1500, max_sentence_ms: 20_000, idle_finalise_ms: 1800, min_chars: 2 }
    }
}

// ---------------------------------------------------------------------------------------------
// Event sink: Tauri in the app, a Vec in tests.

pub trait EventSink: Send + Sync {
    fn segment(&self, s: &Segment);
    fn levels(&self, _l: &Levels) {}
    fn error(&self, _msg: &str) {}
}

impl EventSink for AppHandle {
    fn segment(&self, s: &Segment) {
        let _ = self.emit("segment", s);
    }
    fn levels(&self, l: &Levels) {
        let _ = self.emit("levels", l);
    }
    fn error(&self, m: &str) {
        let _ = self.emit("pipeline-error", m);
    }
}

/// Collects everything; `final_only()` gives the transcript a reader would end up with.
#[derive(Default)]
pub struct CollectSink {
    pub all: Mutex<Vec<Segment>>,
}
impl EventSink for CollectSink {
    fn segment(&self, s: &Segment) {
        self.all.lock().push(s.clone());
    }
}
/// Fan-out to several sinks (UI + database).
pub struct MultiSink(pub Vec<Arc<dyn EventSink>>);
impl EventSink for MultiSink {
    fn segment(&self, s: &Segment) {
        for k in &self.0 {
            k.segment(s);
        }
    }
    fn levels(&self, l: &Levels) {
        for k in &self.0 {
            k.levels(l);
        }
    }
    fn error(&self, m: &str) {
        for k in &self.0 {
            k.error(m);
        }
    }
}

/// Persists finalised sentences to the local history.
pub struct DbSink {
    pub db: Arc<crate::db::Db>,
    pub meeting_id: i64,
}
impl EventSink for DbSink {
    fn segment(&self, s: &Segment) {
        if s.is_final {
            if let Err(e) = self.db.insert_segment(self.meeting_id, s) {
                error!("db: {e:#}");
            }
        }
    }
}

impl CollectSink {
    pub fn final_only(&self) -> Vec<Segment> {
        let all = self.all.lock();
        let mut out: Vec<Segment> = vec![];
        for s in all.iter() {
            match out.iter_mut().find(|o| o.id == s.id) {
                Some(o) => *o = s.clone(),
                None => out.push(s.clone()),
            }
        }
        out.retain(|s| s.is_final);
        out
    }
}

// ---------------------------------------------------------------------------------------------

pub struct SessionHandle {
    running: Arc<AtomicBool>,
    mixer_cmd: Sender<MixerCommand>,
    pub speakers: Arc<Mutex<SpeakerRegistry>>,
    pub player: Arc<Player>,
    /// Held by anything that plays through the speakers — see [`EchoGate`].
    pub gate: Arc<EchoGate>,
    pub cfg: SessionConfig,
    pub meeting_id: i64,
}

impl SessionHandle {
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = self.mixer_cmd.send(MixerCommand::Stop);
        self.player.clear();
    }
    pub fn mixer(&self) -> &Sender<MixerCommand> {
        &self.mixer_cmd
    }
}

pub struct Job {
    pub utt: Utterance,
    pub role: SourceRole,
}

/// The expensive half of `build()`: ASR + translation + tagging. Cached against the length of
/// the audio it was derived from, because finalising a sentence that has not grown since its
/// draft would otherwise re-run all of it on byte-identical PCM — and translation is by far the
/// most expensive stage in the pipeline (see `bench-latency`).
#[derive(Clone)]
struct Built {
    src_lang: String,
    text: String,
    target_lang: String,
    target_text: String,
    tokens: Vec<crate::types::Token>,
}

struct Open {
    id: String,
    /// Resolved on the first build that produces words — see [`Voiceprint`]. `None` until then.
    speaker: Option<SpeakerRef>,
    /// Who we think is talking, before ASR has said whether this was talking at all.
    voice: Voiceprint,
    role: SourceRole,
    pcm: Vec<f32>,
    started_ms: u64,
    ended_ms: u64,
    last_text: String,
    revision: u32,
    arrived_at: u64,
    last_touch: Instant,
    /// (samples of `pcm` this was computed from, result). `None` for the result means there
    /// was nothing worth showing — cached like any other verdict, so a cough is not
    /// transcribed a second time on its way out.
    cached: Option<(usize, Option<Built>)>,
}

impl Open {
    /// Whether `v` is the voice this sentence is already carrying. Once the sentence has been
    /// committed to the registry we can compare speaker ids; before that either side may still
    /// be unregistered, so fall back to comparing the two embeddings against each other.
    fn is_same_voice(&self, v: &Voiceprint, threshold: f32) -> bool {
        match &self.speaker {
            Some(s) => v.id() == Some(s.id),
            None => match (self.voice.id(), v.id()) {
                (Some(a), Some(b)) => a == b,
                (None, None) => self.voice.sounds_like(v, threshold),
                _ => false,
            },
        }
    }
}

/// Who an utterance belongs to, before ASR has had its say.
///
/// Resolution is deferred on purpose. Coughs, door slams and bursts of line noise all clear the
/// VAD, all get embedded, and none of them match a real voice — so identifying a speaker the
/// moment an utterance arrived minted a new "Speaker N" every time somebody cleared their
/// throat. `Sentencer` carries this instead, and commits it only once a sentence has words in
/// it.
enum Voiceprint {
    /// Identity known without a model: the local mic ("You"), or diarization switched off.
    Fixed(SpeakerRef),
    /// An embedded voice, plus the registered speaker it matched if there was one.
    Embedded { emb: Vec<f32>, matched: Option<SpeakerRef> },
}

impl Voiceprint {
    /// The speaker id this already resolves to. `None` means the registry has never heard it,
    /// which covers both a person who has not spoken yet and a cough.
    fn id(&self) -> Option<u32> {
        match self {
            Voiceprint::Fixed(s) => Some(s.id),
            Voiceprint::Embedded { matched, .. } => matched.as_ref().map(|s| s.id),
        }
    }

    /// Whether two voices the registry does not know sound like the same person.
    fn sounds_like(&self, other: &Voiceprint, threshold: f32) -> bool {
        match (self, other) {
            (Voiceprint::Embedded { emb: a, .. }, Voiceprint::Embedded { emb: b, .. }) => cosine(a, b) >= threshold,
            _ => false,
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub fn looks_finished(text: &str) -> bool {
    let t = text.trim_end();
    t.ends_with(['.', '!', '?', '。', '！', '？']) && !t.ends_with("...") && !t.ends_with('…')
}

/// Strip Whisper's non-speech annotations, leaving whatever was actually said.
///
/// Whisper narrates sound it cannot transcribe as a bracketed aside — "(coughing)", "[static]",
/// "*sighs*", "♪♪♪" — written in whichever language it thinks it is hearing, so a Russian call
/// produces "(Кашель)" and "(статика)". We match on the wrapper rather than on keywords: a
/// keyword list would only ever cover the languages we happened to test, and Whisper has 99.
///
/// An opening bracket with no closer is left alone, on the grounds that it is punctuation in a
/// real sentence rather than an annotation that lost its other half. A stray "♪" or "*" is not
/// punctuation in any sentence, so that one goes.
pub fn strip_non_speech(text: &str) -> String {
    // Wrappers Whisper uses, including the full-width CJK forms.
    const PAIRS: [(char, char); 6] = [('(', ')'), ('[', ']'), ('{', '}'), ('（', '）'), ('［', '］'), ('【', '】')];
    // Wrappers that are their own closer.
    const SYMMETRIC: [char; 2] = ['♪', '*'];

    let chars: Vec<char> = text.chars().collect();
    let mut kept = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let symmetric = SYMMETRIC.contains(&c);
        let closer = PAIRS.iter().find(|(open, _)| *open == c).map(|(_, close)| *close).or(symmetric.then_some(c));
        match closer.and_then(|close| chars[i + 1..].iter().position(|&x| x == close)) {
            Some(offset) => i += offset + 2,
            None => {
                if !symmetric {
                    kept.push(c);
                }
                i += 1;
            }
        }
    }
    // Cutting an aside out of the middle of a sentence leaves a double space behind.
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// How long after our own audio stops that we carry on ignoring the input. It has to cover the
/// capture buffers, the output device's own latency and the room's reverb tail; 600 ms is
/// comfortably past all three without swallowing a reply.
const ECHO_TAIL: Duration = Duration::from_millis(600);

/// Stops the pipeline listening to its own voice.
///
/// Whatever we play reaches the microphone acoustically, and — when the call audio is captured
/// through a loopback device — goes straight back down the remote source as well. Left alone
/// that closes a loop: we speak the translation, hear it, transcribe it, translate it back and
/// speak it again, forever, with the two languages ping-ponging.
///
/// Ducking the remote source could never fix this on its own. It is partial by default, it does
/// not touch the microphone, and above all the mixer applies gain when a frame is *mixed* while
/// the audio was captured some buffers earlier — so the tail of every spoken translation
/// arrives after the duck has already lifted. Hence a gate with a hangover rather than a fader.
pub struct EchoGate {
    speaking: AtomicBool,
    /// Once playback ends, keep suppressing until this instant.
    until: Mutex<Option<Instant>>,
    tail: Duration,
}

impl Default for EchoGate {
    fn default() -> Self {
        Self::with_tail(ECHO_TAIL)
    }
}

impl EchoGate {
    pub fn with_tail(tail: Duration) -> Self {
        Self { speaking: AtomicBool::new(false), until: Mutex::new(None), tail }
    }

    /// Our own audio is about to start. Call before the first sample is queued.
    pub fn begin(&self) {
        self.speaking.store(true, Ordering::SeqCst);
    }

    /// Our own audio has finished. The tail keeps running after this returns.
    pub fn end(&self) {
        // Deadline first, so `suppressed` never sees a gap between the two stores.
        *self.until.lock() = Some(Instant::now() + self.tail);
        self.speaking.store(false, Ordering::SeqCst);
    }

    /// True while our own audio may still be reaching the microphone.
    pub fn suppressed(&self) -> bool {
        self.speaking.load(Ordering::SeqCst) || self.until.lock().is_some_and(|t| Instant::now() < t)
    }
}

/// Play through our own speakers with the input gated off until the sound has died away.
///
/// Every path that makes a noise during a session has to go through here. Anything that does
/// not will be transcribed, translated and — if it came from the call side — spoken back.
pub fn play_gated(gate: &EchoGate, player: &Player, pcm: &[f32], rate: u32) {
    gate.begin();
    player.play(pcm, rate);
    // Drain-driven rather than a computed sleep, so the gate tracks real playback.
    while player.is_playing() {
        thread::sleep(Duration::from_millis(25));
    }
    gate.end();
}

/// Optional audio output for spoken translations (absent in offline runs).
///
/// Synthesis and playback happen on their own thread: the ML thread must never block on
/// audio, or the bounded job channel backs up and utterances get dropped. Dropping the
/// `Voice` (i.e. the `Sentencer`) ends that thread.
pub struct Voice(Sender<(String, String)>);

impl Voice {
    pub fn spawn(
        engines: Loaded,
        cfg: SessionConfig,
        player: Arc<Player>,
        mixer: Sender<MixerCommand>,
        gate: Arc<EchoGate>,
    ) -> Result<Self> {
        let (tx, rx) = bounded::<(String, String)>(8);
        thread::Builder::new().name("learnlive-voice".into()).spawn(move || {
            for (lang, text) in rx {
                speak(&engines, &cfg, &player, &mixer, &gate, &lang, &text);
            }
        })?;
        Ok(Self(tx))
    }

    /// Never blocks: if a translation is still being spoken, the new one is dropped rather
    /// than queued behind it — stale audio is worse than no audio in a live conversation.
    fn say(&self, lang: &str, text: &str) {
        if self.0.try_send((lang.to_string(), text.to_string())).is_err() {
            warn!("voice busy; skipped speaking a translation");
        }
    }
}

/// Turns utterances into revisable sentences. Not thread-safe by design: one per ML thread.
pub struct Sentencer {
    engines: Loaded,
    cfg: SessionConfig,
    opts: SentenceOptions,
    speakers: Arc<Mutex<SpeakerRegistry>>,
    sink: Arc<dyn EventSink>,
    voice: Option<Voice>,
    clip_dir: Option<PathBuf>,
    open: Option<Open>,
    /// Injectable clock so tests can simulate idle without sleeping.
    now: Box<dyn Fn() -> Instant + Send>,
}

impl Sentencer {
    pub fn new(
        engines: Loaded,
        cfg: SessionConfig,
        speakers: Arc<Mutex<SpeakerRegistry>>,
        sink: Arc<dyn EventSink>,
        voice: Option<Voice>,
        clip_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            engines,
            cfg,
            opts: SentenceOptions::default(),
            speakers,
            sink,
            voice,
            clip_dir,
            open: None,
            now: Box::new(Instant::now),
        }
    }

    pub fn with_options(mut self, opts: SentenceOptions) -> Self {
        self.opts = opts;
        self
    }

    #[cfg(test)]
    fn with_clock(mut self, f: impl Fn() -> Instant + Send + 'static) -> Self {
        self.now = Box::new(f);
        self
    }

    /// Embed the utterance and ask the registry whether it already knows the voice. Nothing is
    /// written to the registry here — see [`Voiceprint`] for why that has to wait.
    fn voiceprint(&self, job: &Job) -> Result<Voiceprint> {
        if job.role == SourceRole::Local {
            return Ok(Voiceprint::Fixed(self.speakers.lock().local()));
        }
        let Some(embedder) = &self.engines.speaker else {
            return Ok(Voiceprint::Fixed(SpeakerRef { id: 1, label: "Speaker".into(), confidence: 0.0 }));
        };
        let emb = embedder.embed(&job.utt.pcm)?;
        let matched = self.speakers.lock().matching(&emb);
        Ok(Voiceprint::Embedded { emb, matched })
    }

    pub fn push(&mut self, job: Job) -> Result<()> {
        let voice = self.voiceprint(&job)?;
        let threshold = self.speakers.lock().threshold();

        let continues = self
            .open
            .as_ref()
            .map(|o| {
                o.is_same_voice(&voice, threshold)
                    && job.utt.started_ms.saturating_sub(o.ended_ms) <= self.opts.continue_gap_ms
                    && !looks_finished(&o.last_text)
                    && job.utt.ended_ms.saturating_sub(o.started_ms) <= self.opts.max_sentence_ms
            })
            .unwrap_or(false);

        if !continues {
            self.finalise()?;
            self.open = Some(Open {
                id: uuid::Uuid::new_v4().to_string(),
                speaker: None,
                voice,
                role: job.role,
                pcm: vec![],
                started_ms: job.utt.started_ms,
                ended_ms: job.utt.ended_ms,
                last_text: String::new(),
                revision: 0,
                arrived_at: now_ms(),
                last_touch: (self.now)(),
                cached: None,
            });
        } else {
            // This utterance is joining a sentence that already has words in it, so its voice is
            // a real one and may sharpen the speaker's centroid.
            let id = {
                let o = self.open.as_mut().expect("open");
                o.revision += 1;
                o.speaker.as_ref().map(|s| s.id)
            };
            if let (Some(id), Voiceprint::Embedded { emb, .. }) = (id, &voice) {
                self.speakers.lock().reinforce(id, emb);
            }
        }

        let o = self.open.as_mut().expect("open");
        o.pcm.extend_from_slice(&job.utt.pcm);
        o.ended_ms = job.utt.ended_ms;
        o.last_touch = (self.now)();
        o.cached = None; // the audio grew; the previous transcript no longer describes it

        if let Some(seg) = self.build(false)? {
            self.open.as_mut().unwrap().last_text = seg.source_text.clone();
            self.sink.segment(&seg);
            if looks_finished(&seg.source_text) {
                self.finalise()?;
            }
        }
        Ok(())
    }

    /// Call periodically; finalises an open sentence nobody has added to for a while.
    pub fn tick(&mut self) -> Result<()> {
        let idle = self
            .open
            .as_ref()
            .map(|o| (self.now)().duration_since(o.last_touch) >= Duration::from_millis(self.opts.idle_finalise_ms))
            .unwrap_or(false);
        if idle {
            self.finalise()?;
        }
        Ok(())
    }

    pub fn finalise(&mut self) -> Result<()> {
        if self.open.is_none() {
            return Ok(());
        }
        let seg = self.build(true)?;
        let o = self.open.take().unwrap();
        if let Some(seg) = seg {
            self.sink.segment(&seg);
            if self.cfg.speak_translations && o.role == SourceRole::Remote {
                if let Some(v) = &self.voice {
                    v.say(&seg.target_lang, &seg.target_text);
                }
            }
        }
        Ok(())
    }

    fn build(&mut self, is_final: bool) -> Result<Option<Segment>> {
        let pcm_len = self.open.as_ref().expect("open").pcm.len();

        // Reuse the draft's work when finalising a sentence that hasn't grown. Without this,
        // every sentence ending in punctuation is transcribed and translated twice. "Nothing
        // worth showing" is cached the same way, so a cough is not transcribed again either.
        let hit = matches!(self.open.as_ref().and_then(|o| o.cached.as_ref()), Some((n, _)) if *n == pcm_len);
        if !hit {
            let built = self.compute()?;
            self.open.as_mut().expect("open").cached = Some((pcm_len, built));
        }
        if !matches!(self.open.as_ref().and_then(|o| o.cached.as_ref()), Some((_, Some(_)))) {
            return Ok(None);
        }

        // Words came back, so a person said them — only now may this voice enter the registry.
        let speaker = self.commit_speaker();

        let o = self.open.as_ref().expect("open");
        let b = o.cached.as_ref().and_then(|(_, b)| b.as_ref()).expect("just checked");
        let clip_path = match (&self.clip_dir, is_final) {
            (Some(d), true) => write_wav(d, &o.id, &o.pcm).ok().map(|p| p.to_string_lossy().to_string()),
            _ => None,
        };

        Ok(Some(Segment {
            id: o.id.clone(),
            speaker,
            role: o.role,
            started_ms: o.started_ms,
            ended_ms: o.ended_ms,
            source_lang: b.src_lang.clone(),
            source_text: b.text.clone(),
            target_lang: b.target_lang.clone(),
            target_text: b.target_text.clone(),
            tokens: b.tokens.clone(),
            clip_path,
            is_final,
            revision: o.revision,
            arrived_at: o.arrived_at,
        }))
    }

    /// Resolve the open sentence's voice against the registry, once. Only ever called after
    /// ASR has confirmed the sentence has words in it, so nothing non-speech gets this far.
    fn commit_speaker(&mut self) -> SpeakerRef {
        let o = self.open.as_ref().expect("open");
        if let Some(s) = &o.speaker {
            return s.clone();
        }
        let speaker = match &o.voice {
            Voiceprint::Fixed(s) => s.clone(),
            Voiceprint::Embedded { emb, matched } => {
                let mut registry = self.speakers.lock();
                match matched {
                    Some(s) => {
                        registry.reinforce(s.id, emb);
                        s.clone()
                    }
                    None => registry.enroll(emb),
                }
            }
        };
        self.open.as_mut().expect("open").speaker = Some(speaker.clone());
        speaker
    }

    /// ASR + translation + tagging over the open sentence's audio. The expensive part.
    fn compute(&self) -> Result<Option<Built>> {
        let o = self.open.as_ref().expect("open");
        let cfg = &self.cfg;
        let (src_lang, heard) = self.engines.asr.transcribe(&o.pcm, &cfg.source_lang)?;
        // Whisper narrates what it cannot transcribe: a cough, a door slam or a burst of line
        // noise comes back as "(Кашель)", "(coughing)" or "[static]". Nobody said those, so they
        // must not be translated, shown, or — by way of `build` — allowed to register a speaker.
        let text = strip_non_speech(&heard);
        if text.chars().count() < self.opts.min_chars || !text.chars().any(char::is_alphanumeric) {
            return Ok(None);
        }

        let target_lang = if src_lang == cfg.learning { cfg.native.clone() } else { cfg.learning.clone() };
        let target_text = self.engines.translator.translate(&text, &src_lang, &target_lang)?;

        let study_text = if target_lang == cfg.learning { &target_text } else { &text };
        let tokens = self
            .engines
            .grammar
            .as_ref()
            .map(|g| g.analyze(study_text, &cfg.learning).unwrap_or_default())
            .unwrap_or_default();

        Ok(Some(Built { src_lang, text, target_lang, target_text, tokens }))
    }
}

// ---------------------------------------------------------------------------------------------
// Live session

pub fn start(
    app: AppHandle,
    engines: Arc<Engines>,
    db: Arc<crate::db::Db>,
    meeting_id: i64,
    cfg: SessionConfig,
    clip_dir: PathBuf,
) -> Result<SessionHandle> {
    let loaded = engines.load(&cfg)?;
    let mixer = Mixer::start(&cfg.sources)?;
    let player = Arc::new(Player::open()?);
    let gate = Arc::new(EchoGate::default());
    let running = Arc::new(AtomicBool::new(true));
    let mut registry = SpeakerRegistry::new(512);
    if db.remember_voices() {
        registry.preload(&db.voiceprints()?);
    }
    let speakers = Arc::new(Mutex::new(registry));
    let (job_tx, job_rx) = bounded::<Job>(16);
    std::fs::create_dir_all(&clip_dir)?;
    let sink: Arc<dyn EventSink> =
        Arc::new(MultiSink(vec![Arc::new(app.clone()), Arc::new(DbSink { db, meeting_id })]));

    {
        let running = running.clone();
        let sink = sink.clone();
        let model_dir = engines.model_dir.clone();
        let frames = mixer.frames.clone();
        let gate = gate.clone();
        thread::Builder::new().name("learnlive-vad".into()).spawn(move || {
            let mut seg = match Segmenter::new(&model_dir) {
                Ok(s) => s,
                Err(e) => {
                    error!("vad: {e}");
                    return;
                }
            };
            let mut votes = (0u32, 0u32);
            let mut last_meter = Instant::now();
            let mut speaking = false;
            let silence = vec![0.0f32; FRAME_SAMPLES];
            while running.load(Ordering::SeqCst) {
                let Ok(MixFrame { mix, rms, dominant }) = frames.recv_timeout(Duration::from_millis(100)) else {
                    continue;
                };
                // While our own audio is playing — and for a moment after, see `EchoGate` —
                // everything arriving is that audio coming back round through the loopback and
                // the microphone. Feed the VAD silence rather than skipping the frame, so the
                // utterance timeline stays lined up with the wall clock across the gap.
                let deaf = gate.suppressed();
                if !deaf {
                    match dominant {
                        Some(SourceRole::Local) => votes.0 += 1,
                        Some(SourceRole::Remote) => votes.1 += 1,
                        None => {}
                    }
                }
                let heard = if deaf { &silence[..mix.len().min(FRAME_SAMPLES)] } else { &mix[..] };
                match seg.push(heard) {
                    Ok((sp, Some(utt))) => {
                        speaking = sp;
                        let role = if votes.0 > votes.1 { SourceRole::Local } else { SourceRole::Remote };
                        votes = (0, 0);
                        if job_tx.try_send(Job { utt, role }).is_err() {
                            warn!("ML thread busy; dropped an utterance");
                        }
                    }
                    Ok((sp, None)) => speaking = sp,
                    Err(e) => error!("vad push: {e}"),
                }
                if last_meter.elapsed() > Duration::from_millis(66) {
                    last_meter = Instant::now();
                    let level = (mix.iter().map(|x| x * x).sum::<f32>() / mix.len() as f32).sqrt();
                    sink.levels(&Levels { per_source: rms, mix: level, speech_active: speaking });
                }
            }
        })?;
    }

    {
        let running = running.clone();
        let voice = if cfg.speak_translations {
            Some(Voice::spawn(loaded.clone(), cfg.clone(), player.clone(), mixer.commands.clone(), gate.clone())?)
        } else {
            None
        };
        let mut s = Sentencer::new(loaded, cfg.clone(), speakers.clone(), sink.clone(), voice, Some(clip_dir));
        let sink = sink.clone();
        thread::Builder::new().name("learnlive-ml".into()).spawn(move || {
            while running.load(Ordering::SeqCst) {
                let r = match job_rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(job) => s.push(job),
                    Err(_) => s.tick(),
                };
                if let Err(e) = r {
                    error!("pipeline: {e:#}");
                    sink.error(&e.to_string());
                }
            }
            let _ = s.finalise();
        })?;
    }

    info!("session started: learning={} native={} sources={}", cfg.learning, cfg.native, cfg.sources.len());
    Ok(SessionHandle { running, mixer_cmd: mixer.commands.clone(), speakers, player, gate, cfg, meeting_id })
}

// ---------------------------------------------------------------------------------------------
// Offline run: WAV → segments, as fast as the models allow. Mono = remote speech.
// Stereo = channel 0 is you (local), channel 1 is the call (remote) — record it that way and
// the role split gets tested too.

pub fn run_offline(engines: Arc<Engines>, cfg: &SessionConfig, wav_path: &std::path::Path) -> Result<Vec<Segment>> {
    let loaded = engines.load(cfg)?;
    let wav = crate::audio::read_wav(wav_path)?;
    let sink = Arc::new(CollectSink::default());
    let speakers = Arc::new(Mutex::new(SpeakerRegistry::new(512)));
    let mut s = Sentencer::new(loaded, cfg.clone(), speakers, sink.clone(), None, None);

    // Split channels, resample each, then walk them in lockstep through the VAD like the mixer would.
    let ch = wav.channels as usize;
    let mut lanes: Vec<Vec<f32>> = (0..ch.min(2))
        .map(|c| {
            let mut rs = crate::audio::resample::ToPipelineRate::new(wav.sample_rate, 1).unwrap();
            let mono: Vec<f32> = wav.samples.iter().skip(c).step_by(ch).copied().collect();
            rs.push(&mono).unwrap_or_default()
        })
        .collect();
    if ch > 2 {
        warn!("using first two channels of a {ch}-channel file");
    }
    let n = lanes.iter().map(|l| l.len()).max().unwrap_or(0);
    for l in lanes.iter_mut() {
        l.resize(n, 0.0);
    }

    let mut seg = Segmenter::new(&engines.model_dir)?;
    let mut votes = (0u32, 0u32);
    let frame = crate::audio::FRAME_SAMPLES;
    for i in (0..n).step_by(frame) {
        let end = (i + frame).min(n);
        let mut mix = vec![0.0f32; end - i];
        let mut rms = vec![0.0f32; lanes.len()];
        for (li, l) in lanes.iter().enumerate() {
            for (m, x) in mix.iter_mut().zip(&l[i..end]) {
                *m += x;
                rms[li] += x * x;
            }
        }
        if lanes.len() == 2 {
            if rms[0] > rms[1] * 1.5 && rms[0] > 1e-4 {
                votes.0 += 1
            } else if rms[1] > 1e-4 {
                votes.1 += 1
            }
        }
        if let (_, Some(utt)) = seg.push(&mix)? {
            let role = if lanes.len() == 2 && votes.0 > votes.1 { SourceRole::Local } else { SourceRole::Remote };
            votes = (0, 0);
            s.push(Job { utt, role })?;
        }
    }
    // The VAD holds the final utterance until it sees enough trailing silence; a file usually
    // ends mid-pause, so drain it explicitly or the last sentence is silently lost.
    let role = if lanes.len() == 2 && votes.0 > votes.1 { SourceRole::Local } else { SourceRole::Remote };
    for utt in seg.flush() {
        s.push(Job { utt, role })?;
    }
    s.finalise()?;
    Ok(sink.final_only())
}

// ---------------------------------------------------------------------------------------------

pub fn speak(
    engines: &Loaded,
    cfg: &SessionConfig,
    player: &Player,
    mixer: &Sender<MixerCommand>,
    gate: &EchoGate,
    lang: &str,
    text: &str,
) {
    let Some(voice) = engines.tts.get(lang) else { return };
    match voice.synthesize(text) {
        Ok((pcm, _)) if pcm.is_empty() => warn!("tts: produced no audio for {lang}"),
        Ok((pcm, rate)) => {
            let _ = mixer.send(MixerCommand::Duck(1.0 - cfg.duck_amount));
            play_gated(gate, player, &pcm, rate);
            let _ = mixer.send(MixerCommand::Duck(1.0));
        }
        Err(e) => warn!("tts: {e}"),
    }
}

fn write_wav(dir: &std::path::Path, id: &str, pcm: &[f32]) -> Result<PathBuf> {
    let path = dir.join(format!("{id}.wav"));
    crate::audio::write_wav16(&path, 16_000, 1, pcm)?;
    Ok(path)
}

pub fn read_wav(path: &str) -> Result<Vec<f32>> {
    Ok(crate::audio::read_wav(std::path::Path::new(path))?.samples)
}

// =============================================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{GrammarAnalyzer, SpeakerEmbedder, Transcriber, Translator};

    /// Fake ASR: "hears" a script keyed by audio length, so merged audio yields merged text.
    struct FakeAsr;
    impl Transcriber for FakeAsr {
        fn transcribe(&self, pcm: &[f32], _: &str) -> Result<(String, String)> {
            // Each 1000 "samples" is one word of a fixed sentence; a full stop only on the 4th.
            let words = ["я", "читаю", "книгу", "дома."];
            let n = (pcm.len() / 1000).clamp(1, 4);
            Ok(("ru".into(), words[..n].join(" ")))
        }
    }
    /// Counts how many times the engines actually run, so duplicated work is visible.
    #[derive(Default)]
    struct Counting {
        asr: std::sync::atomic::AtomicUsize,
        mt: std::sync::atomic::AtomicUsize,
    }
    struct CountingAsr(Arc<Counting>);
    impl Transcriber for CountingAsr {
        fn transcribe(&self, pcm: &[f32], hint: &str) -> Result<(String, String)> {
            self.0.asr.fetch_add(1, Ordering::SeqCst);
            FakeAsr.transcribe(pcm, hint)
        }
    }
    struct CountingMt(Arc<Counting>);
    impl Translator for CountingMt {
        fn translate(&self, t: &str, a: &str, b: &str) -> Result<String> {
            self.0.mt.fetch_add(1, Ordering::SeqCst);
            FakeMt.translate(t, a, b)
        }
    }

    struct FakeMt;
    impl Translator for FakeMt {
        fn translate(&self, t: &str, _: &str, _: &str) -> Result<String> {
            Ok(format!("[{t}]"))
        }
    }
    struct NoGrammar;
    impl GrammarAnalyzer for NoGrammar {
        fn analyze(&self, _: &str, _: &str) -> Result<Vec<crate::types::Token>> {
            Ok(vec![])
        }
    }
    /// Fake ASR that always "hears" the same thing, counting how often it is asked.
    struct SaysAsr(&'static str, std::sync::atomic::AtomicUsize);
    impl Transcriber for SaysAsr {
        fn transcribe(&self, _: &[f32], _: &str) -> Result<(String, String)> {
            self.1.fetch_add(1, Ordering::SeqCst);
            Ok(("ru".into(), self.0.to_string()))
        }
    }
    /// Fake ASR reading down a script, one entry per call, so a single test can interleave
    /// real speech with the sound events Whisper emits for coughs and static.
    struct ScriptedAsr(Mutex<std::collections::VecDeque<&'static str>>);
    impl Transcriber for ScriptedAsr {
        fn transcribe(&self, _: &[f32], _: &str) -> Result<(String, String)> {
            Ok(("ru".into(), self.0.lock().pop_front().unwrap_or_default().to_string()))
        }
    }

    /// Speaker = sign of the first sample. Two "voices" for tests.
    struct FakeEmb;
    impl SpeakerEmbedder for FakeEmb {
        fn embed(&self, pcm: &[f32]) -> Result<Vec<f32>> {
            let s = if pcm.first().copied().unwrap_or(1.0) >= 0.0 { 1.0 } else { -1.0 };
            let mut v = vec![0.0; 512];
            v[0] = s;
            v[1] = 0.1;
            Ok(v)
        }
    }

    /// Every utterance embeds to its own axis, so a test can make each cough sound like
    /// nobody the registry has ever heard — which is what a cough actually does.
    struct DistinctEmb;
    impl SpeakerEmbedder for DistinctEmb {
        fn embed(&self, pcm: &[f32]) -> Result<Vec<f32>> {
            let mut v = vec![0.0; 512];
            v[(pcm.first().copied().unwrap_or(0.0) as usize).min(511)] = 1.0;
            Ok(v)
        }
    }

    fn engines() -> Loaded {
        Loaded {
            asr: Arc::new(FakeAsr),
            translator: Arc::new(FakeMt),
            grammar: Some(Arc::new(NoGrammar)),
            speaker: Some(Arc::new(FakeEmb)),
            tts: Default::default(),
        }
    }
    fn cfg() -> SessionConfig {
        SessionConfig { speak_translations: false, ..Default::default() }
    }
    fn utt(words: usize, start: u64, voice: f32) -> Utterance {
        let mut pcm = vec![0.01; words * 1000];
        pcm[0] = voice;
        Utterance { pcm, started_ms: start, ended_ms: start + words as u64 * 500 }
    }
    fn sut() -> (Sentencer, Arc<CollectSink>) {
        let sink = Arc::new(CollectSink::default());
        let s =
            Sentencer::new(engines(), cfg(), Arc::new(Mutex::new(SpeakerRegistry::new(512))), sink.clone(), None, None);
        (s, sink)
    }

    #[test]
    fn continuation_merges_and_revises_in_place() {
        let (mut s, sink) = sut();
        s.push(Job { utt: utt(2, 0, 1.0), role: SourceRole::Remote }).unwrap(); // "я читаю"
        s.push(Job { utt: utt(2, 1500, 1.0), role: SourceRole::Remote }).unwrap(); // + "книгу дома."
        let all = sink.all.lock();
        assert_eq!(all.len(), 3, "draft, revised draft, final");
        assert!(all.iter().all(|x| x.id == all[0].id), "same sentence id throughout");
        assert_eq!(all[0].source_text, "я читаю");
        assert!(!all[0].is_final);
        assert_eq!(all[1].source_text, "я читаю книгу дома.");
        assert_eq!(all[1].revision, 1);
        assert!(all[2].is_final, "terminal punctuation finalises immediately");
        assert_eq!(all[2].target_text, "[я читаю книгу дома.]");
    }

    #[test]
    fn long_gap_starts_a_new_sentence() {
        let (mut s, sink) = sut();
        s.push(Job { utt: utt(2, 0, 1.0), role: SourceRole::Remote }).unwrap();
        s.push(Job { utt: utt(2, 5000, 1.0), role: SourceRole::Remote }).unwrap(); // gap 4 s > 1.5 s
        let f = sink.final_only();
        assert_eq!(f.len(), 1, "first sentence finalised by the new one; second still open");
        assert_eq!(f[0].source_text, "я читаю");
        s.finalise().unwrap();
        assert_eq!(sink.final_only().len(), 2);
    }

    #[test]
    fn speaker_change_never_merges() {
        let (mut s, sink) = sut();
        s.push(Job { utt: utt(2, 0, 1.0), role: SourceRole::Remote }).unwrap();
        s.push(Job { utt: utt(2, 1200, -1.0), role: SourceRole::Remote }).unwrap(); // different voice
        s.finalise().unwrap();
        let f = sink.final_only();
        assert_eq!(f.len(), 2);
        assert_ne!(f[0].speaker.id, f[1].speaker.id);
        assert_eq!(f[1].speaker.label, "Speaker 2");
    }

    #[test]
    fn local_mic_is_you_and_translates_into_learning_language() {
        let (mut s, sink) = sut();
        s.push(Job { utt: utt(2, 0, 1.0), role: SourceRole::Local }).unwrap();
        s.finalise().unwrap();
        let f = sink.final_only();
        assert_eq!(f[0].speaker.label, "You");
        assert_eq!(f[0].role, SourceRole::Local);
    }

    #[test]
    fn idle_tick_finalises() {
        let (mut s, sink) = sut();
        let base = Instant::now();
        let offset = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let o2 = offset.clone();
        s = s.with_clock(move || base + Duration::from_millis(o2.load(Ordering::SeqCst)));
        s.push(Job { utt: utt(2, 0, 1.0), role: SourceRole::Remote }).unwrap();
        s.tick().unwrap();
        assert!(sink.final_only().is_empty(), "not idle yet");
        offset.store(SentenceOptions::default().idle_finalise_ms + 1, Ordering::SeqCst);
        s.tick().unwrap();
        assert_eq!(sink.final_only().len(), 1);
    }

    /// Finalising a sentence that has not grown since its draft must not re-run the models.
    /// Translation is the most expensive stage in the pipeline, so doing it twice per sentence
    /// roughly doubled end-to-end latency (see `bench-latency`).
    #[test]
    fn finalising_reuses_the_draft_instead_of_recomputing() {
        let counts = Arc::new(Counting::default());
        let engines = Loaded {
            asr: Arc::new(CountingAsr(counts.clone())),
            translator: Arc::new(CountingMt(counts.clone())),
            grammar: Some(Arc::new(NoGrammar)),
            speaker: Some(Arc::new(FakeEmb)),
            tts: Default::default(),
        };
        let sink = Arc::new(CollectSink::default());
        let mut s =
            Sentencer::new(engines, cfg(), Arc::new(Mutex::new(SpeakerRegistry::new(512))), sink.clone(), None, None);

        // Four "words" ends in a full stop, so push() emits a draft and finalises immediately.
        s.push(Job { utt: utt(4, 0, 1.0), role: SourceRole::Remote }).unwrap();

        let all = sink.all.lock();
        assert_eq!(all.len(), 2, "a draft and a final");
        assert!(all[1].is_final);
        assert_eq!(all[0].source_text, all[1].source_text, "same audio must give the same text");
        drop(all);
        assert_eq!(counts.asr.load(Ordering::SeqCst), 1, "ASR ran more than once on identical audio");
        assert_eq!(counts.mt.load(Ordering::SeqCst), 1, "translation ran more than once on identical audio");
    }

    /// …but growing the sentence must invalidate that cache.
    #[test]
    fn extending_a_sentence_recomputes() {
        let counts = Arc::new(Counting::default());
        let engines = Loaded {
            asr: Arc::new(CountingAsr(counts.clone())),
            translator: Arc::new(CountingMt(counts.clone())),
            grammar: Some(Arc::new(NoGrammar)),
            speaker: Some(Arc::new(FakeEmb)),
            tts: Default::default(),
        };
        let sink = Arc::new(CollectSink::default());
        let mut s =
            Sentencer::new(engines, cfg(), Arc::new(Mutex::new(SpeakerRegistry::new(512))), sink.clone(), None, None);

        s.push(Job { utt: utt(2, 0, 1.0), role: SourceRole::Remote }).unwrap(); // "я читаю"
        s.push(Job { utt: utt(2, 1500, 1.0), role: SourceRole::Remote }).unwrap(); // grows, then finalises
        assert_eq!(counts.asr.load(Ordering::SeqCst), 2, "each distinct audio length transcribes once");
        assert_eq!(sink.final_only()[0].source_text, "я читаю книгу дома.");
    }

    /// The gate has to stay shut after playback stops. Audio captured while we were talking is
    /// still working its way through the capture buffers, and it is the tail of our own voice
    /// leaking past the gate that closes the loop.
    #[test]
    fn the_echo_gate_outlasts_playback() {
        let gate = EchoGate::with_tail(Duration::from_millis(120));
        assert!(!gate.suppressed(), "nothing is playing");
        gate.begin();
        assert!(gate.suppressed(), "our own voice is audible");
        gate.end();
        assert!(gate.suppressed(), "the tail has not run out yet");
        std::thread::sleep(Duration::from_millis(200));
        assert!(!gate.suppressed(), "the tail should have expired");
    }

    #[test]
    fn punctuation_heuristic() {
        assert!(looks_finished("Готово."));
        assert!(looks_finished("Правда?"));
        assert!(!looks_finished("и потом..."));
        assert!(!looks_finished("а ещё"));
    }

    #[test]
    fn sound_events_are_stripped_whatever_language_whisper_wrote_them_in() {
        for annotation in ["(Кашель)", "(coughing)", "(статика)", "[static]", "{noise}", "（音楽）", "*sighs*", "♪♪♪"]
        {
            assert_eq!(strip_non_speech(annotation), "", "{annotation:?} survived");
        }
        // Speech around an aside survives; the aside does not, and takes its spacing with it.
        assert_eq!(strip_non_speech("(cough) Как дела?"), "Как дела?");
        assert_eq!(strip_non_speech("Привет [static] как дела"), "Привет как дела");
        // Untouched: no annotation, and a lone bracket that is just punctuation.
        assert_eq!(strip_non_speech("я читаю книгу"), "я читаю книгу");
        assert_eq!(strip_non_speech("смайлик :-)"), "смайлик :-)");
    }

    /// A cough or a burst of static clears the VAD like speech does, and Whisper labels it
    /// "(Кашель)" / "(static)". It must not reach the transcript, and — since its embedding
    /// matches no real voice — it must not reach the speaker registry either.
    #[test]
    fn a_sound_event_is_neither_transcript_nor_speaker() {
        for heard in ["(Кашель)", "(coughing)", "(статика)", "[static]", "♪♪♪"] {
            let asr = Arc::new(SaysAsr(heard, Default::default()));
            let engines = Loaded {
                asr: asr.clone(),
                translator: Arc::new(FakeMt),
                grammar: Some(Arc::new(NoGrammar)),
                speaker: Some(Arc::new(FakeEmb)),
                tts: Default::default(),
            };
            let registry = Arc::new(Mutex::new(SpeakerRegistry::new(512)));
            let sink = Arc::new(CollectSink::default());
            let mut s = Sentencer::new(engines, cfg(), registry.clone(), sink.clone(), None, None);

            s.push(Job { utt: utt(2, 0, 1.0), role: SourceRole::Remote }).unwrap();
            s.finalise().unwrap();

            assert!(sink.all.lock().is_empty(), "{heard:?} was shown as a transcript");
            assert!(registry.lock().centroids().is_empty(), "{heard:?} created a speaker");
            assert_eq!(asr.1.load(Ordering::SeqCst), 1, "{heard:?} was transcribed twice");
        }
    }

    /// The reported symptom: coughs and static between sentences each minted a new speaker, so
    /// the second thing one person said came back as "Speaker 4".
    #[test]
    fn sound_events_do_not_advance_the_speaker_counter() {
        let script = ["Привет.", "(Кашель)", "(статика)", "Как дела?"].into_iter().collect();
        let engines = Loaded {
            asr: Arc::new(ScriptedAsr(Mutex::new(script))),
            translator: Arc::new(FakeMt),
            grammar: Some(Arc::new(NoGrammar)),
            speaker: Some(Arc::new(DistinctEmb)),
            tts: Default::default(),
        };
        let registry = Arc::new(Mutex::new(SpeakerRegistry::new(512)));
        let sink = Arc::new(CollectSink::default());
        let mut s = Sentencer::new(engines, cfg(), registry.clone(), sink.clone(), None, None);

        // One person, with a cough and a burst of static in between — each its own "voice".
        for (i, voice) in [1.0, 2.0, 3.0, 1.0].into_iter().enumerate() {
            s.push(Job { utt: utt(2, i as u64 * 5_000, voice), role: SourceRole::Remote }).unwrap();
        }
        s.finalise().unwrap();

        let f = sink.final_only();
        assert_eq!(f.len(), 2, "only the two spoken sentences should survive");
        assert_eq!(f[0].source_text, "Привет.");
        assert_eq!(f[1].source_text, "Как дела?");
        assert_eq!(f[0].speaker.id, f[1].speaker.id, "the same person said both");
        assert_eq!(f[1].speaker.label, "Speaker 1", "the sound events took speaker numbers");
        assert_eq!(registry.lock().centroids().len(), 1, "one voice was in the room");
    }

    /// An aside Whisper tacks onto real speech loses the aside, not the speech.
    #[test]
    fn speech_alongside_an_aside_is_kept() {
        let engines = Loaded {
            asr: Arc::new(SaysAsr("(cough) Я читаю книгу.", Default::default())),
            translator: Arc::new(FakeMt),
            grammar: Some(Arc::new(NoGrammar)),
            speaker: Some(Arc::new(FakeEmb)),
            tts: Default::default(),
        };
        let sink = Arc::new(CollectSink::default());
        let mut s =
            Sentencer::new(engines, cfg(), Arc::new(Mutex::new(SpeakerRegistry::new(512))), sink.clone(), None, None);
        s.push(Job { utt: utt(2, 0, 1.0), role: SourceRole::Remote }).unwrap();
        s.finalise().unwrap();
        let f = sink.final_only();
        assert_eq!(f[0].source_text, "Я читаю книгу.");
        assert_eq!(f[0].speaker.label, "Speaker 1");
    }
}
