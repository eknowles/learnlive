//! Local history. One SQLite file in the app data dir; nothing leaves the machine.
//!
//! Voiceprints are speaker embeddings averaged over everything a named person said. They are
//! biometric data: only stored when the user turns on "remember voices", only for people the
//! user explicitly named, and deletable per person.

use std::path::Path;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};

use crate::types::{MeetingSummary, ParticipantRef, SearchHit, Segment, SessionConfig, SourceRole, SpeakerRef};

pub struct Db {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meetings (
  id INTEGER PRIMARY KEY,
  title TEXT NOT NULL,
  calendar_event_id TEXT,
  started_at INTEGER NOT NULL,
  ended_at INTEGER,
  learning TEXT NOT NULL,
  native TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS participants (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  email TEXT UNIQUE,
  voiceprint BLOB,          -- f32 LE, 512 dims, mean embedding
  voiceprint_n INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS meeting_participants (
  meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  participant_id INTEGER NOT NULL REFERENCES participants(id) ON DELETE CASCADE,
  speaker_id INTEGER,       -- diarizer id within this meeting
  PRIMARY KEY (meeting_id, participant_id)
);
CREATE TABLE IF NOT EXISTS segments (
  id TEXT PRIMARY KEY,
  meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  speaker_id INTEGER NOT NULL,
  speaker_label TEXT NOT NULL,
  role TEXT NOT NULL,
  started_ms INTEGER NOT NULL,
  ended_ms INTEGER NOT NULL,
  arrived_at INTEGER NOT NULL,
  source_lang TEXT NOT NULL,
  source_text TEXT NOT NULL,
  target_lang TEXT NOT NULL,
  target_text TEXT NOT NULL,
  tokens TEXT NOT NULL,     -- JSON
  clip_path TEXT
);
CREATE INDEX IF NOT EXISTS segments_meeting ON segments(meeting_id, started_ms);
CREATE VIRTUAL TABLE IF NOT EXISTS segments_fts USING fts5(
  source_text, target_text, content='segments', content_rowid='rowid', tokenize='unicode61 remove_diacritics 2'
);
CREATE TRIGGER IF NOT EXISTS segments_ai AFTER INSERT ON segments BEGIN
  INSERT INTO segments_fts(rowid, source_text, target_text) VALUES (new.rowid, new.source_text, new.target_text);
END;
CREATE TRIGGER IF NOT EXISTS segments_ad AFTER DELETE ON segments BEGIN
  INSERT INTO segments_fts(segments_fts, rowid, source_text, target_text) VALUES ('delete', old.rowid, old.source_text, old.target_text);
END;
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
"#;

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    // ---- settings -------------------------------------------------------------------------

    pub fn remember_voices(&self) -> bool {
        self.conn.lock().query_row("SELECT value FROM settings WHERE key='remember_voices'", [], |r| r.get::<_, String>(0))
            .optional().ok().flatten().map(|v| v == "1").unwrap_or(false)
    }
    pub fn set_remember_voices(&self, on: bool) -> Result<()> {
        self.conn.lock().execute("INSERT INTO settings(key,value) VALUES('remember_voices',?1) ON CONFLICT(key) DO UPDATE SET value=?1", params![if on { "1" } else { "0" }])?;
        if !on { self.conn.lock().execute("UPDATE participants SET voiceprint=NULL, voiceprint_n=0", [])?; }
        Ok(())
    }

    // ---- meetings -------------------------------------------------------------------------

    pub fn start_meeting(&self, title: &str, event_id: Option<&str>, cfg: &SessionConfig, started_at: i64) -> Result<i64> {
        let c = self.conn.lock();
        c.execute("INSERT INTO meetings(title, calendar_event_id, started_at, learning, native) VALUES(?,?,?,?,?)",
            params![title, event_id, started_at, cfg.learning, cfg.native])?;
        Ok(c.last_insert_rowid())
    }

    pub fn end_meeting(&self, id: i64, ended_at: i64) -> Result<()> {
        self.conn.lock().execute("UPDATE meetings SET ended_at=? WHERE id=?", params![ended_at, id])?;
        Ok(())
    }

    pub fn link_event(&self, meeting_id: i64, event_id: &str, title: &str, attendees: &[(String, Option<String>)]) -> Result<()> {
        let mut c = self.conn.lock();
        let tx = c.transaction()?;
        tx.execute("UPDATE meetings SET calendar_event_id=?, title=? WHERE id=?", params![event_id, title, meeting_id])?;
        for (name, email) in attendees {
            let pid = upsert_participant(&tx, name, email.as_deref())?;
            tx.execute("INSERT OR IGNORE INTO meeting_participants(meeting_id, participant_id) VALUES(?,?)", params![meeting_id, pid])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Say "speaker 2 in this meeting is Anna". Creates the participant if needed.
    pub fn assign_speaker(&self, meeting_id: i64, speaker_id: u32, name: &str, email: Option<&str>) -> Result<i64> {
        let mut c = self.conn.lock();
        let tx = c.transaction()?;
        let pid = upsert_participant(&tx, name, email)?;
        tx.execute("UPDATE meeting_participants SET speaker_id=NULL WHERE meeting_id=? AND speaker_id=?", params![meeting_id, speaker_id])?;
        tx.execute("INSERT INTO meeting_participants(meeting_id, participant_id, speaker_id) VALUES(?,?,?) \
                    ON CONFLICT(meeting_id, participant_id) DO UPDATE SET speaker_id=excluded.speaker_id", params![meeting_id, pid, speaker_id])?;
        tx.execute("UPDATE segments SET speaker_label=? WHERE meeting_id=? AND speaker_id=?", params![name, meeting_id, speaker_id])?;
        tx.commit()?;
        Ok(pid)
    }

    pub fn list_meetings(&self, limit: i64) -> Result<Vec<MeetingSummary>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT m.id, m.title, m.started_at, m.ended_at, m.learning, m.native, m.calendar_event_id, \
                                (SELECT COUNT(*) FROM segments s WHERE s.meeting_id=m.id) \
                                FROM meetings m ORDER BY started_at DESC LIMIT ?")?;
        let rows = st.query_map([limit], |r| Ok(MeetingSummary {
            id: r.get(0)?, title: r.get(1)?, started_at: r.get(2)?, ended_at: r.get(3)?, learning: r.get(4)?, native: r.get(5)?,
            calendar_event_id: r.get(6)?, participants: vec![], sentence_count: r.get(7)?,
        }))?.collect::<Result<Vec<_>, _>>()?;
        drop(st);
        let mut out = vec![];
        for mut m in rows { m.participants = participants_of(&c, m.id)?; out.push(m); }
        Ok(out)
    }

    pub fn meeting(&self, id: i64) -> Result<Option<(MeetingSummary, Vec<Segment>)>> {
        let list = self.list_meetings(i64::MAX)?;
        let Some(m) = list.into_iter().find(|m| m.id == id) else { return Ok(None) };
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT * FROM segments WHERE meeting_id=? ORDER BY started_ms")?;
        let segs = st.query_map([id], row_to_segment)?.collect::<Result<Vec<_>, _>>()?;
        Ok(Some((m, segs)))
    }

    pub fn insert_segment(&self, meeting_id: i64, s: &Segment) -> Result<()> {
        self.conn.lock().execute(
            "INSERT OR REPLACE INTO segments(id, meeting_id, speaker_id, speaker_label, role, started_ms, ended_ms, arrived_at, source_lang, source_text, target_lang, target_text, tokens, clip_path) \
             VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            params![s.id, meeting_id, s.speaker.id, s.speaker.label, role_str(s.role), s.started_ms as i64, s.ended_ms as i64, s.arrived_at as i64,
                    s.source_lang, s.source_text, s.target_lang, s.target_text, serde_json::to_string(&s.tokens)?, s.clip_path])?;
        Ok(())
    }

    /// Full-text search across every meeting. FTS5 query syntax: words, "phrases", prefix*.
    pub fn search(&self, query: &str, limit: i64) -> Result<Vec<SearchHit>> {
        let c = self.conn.lock();
        let mut st = c.prepare(
            "SELECT s.*, m.title, m.started_at FROM segments_fts f \
             JOIN segments s ON s.rowid = f.rowid JOIN meetings m ON m.id = s.meeting_id \
             WHERE segments_fts MATCH ? ORDER BY bm25(segments_fts), m.started_at DESC LIMIT ?")?;
        let rows = st.query_map(params![query, limit], |r| {
            let segment = row_to_segment(r)?;
            Ok(SearchHit { meeting_id: r.get("meeting_id")?, meeting_title: r.get(14)?, started_at: r.get(15)?, segment })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // ---- voiceprints ------------------------------------------------------------------------

    /// All known voices: (participant id, name, mean embedding).
    pub fn voiceprints(&self) -> Result<Vec<(i64, String, Vec<f32>)>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT id, name, voiceprint FROM participants WHERE voiceprint IS NOT NULL")?;
        let rows = st.query_map([], |r| {
            let blob: Vec<u8> = r.get(2)?;
            Ok((r.get(0)?, r.get(1)?, blob.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()))
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Fold a new session centroid into the stored running mean.
    pub fn update_voiceprint(&self, participant_id: i64, centroid: &[f32], n_new: u32) -> Result<()> {
        let c = self.conn.lock();
        let existing: Option<(Vec<u8>, i64)> = c.query_row("SELECT voiceprint, voiceprint_n FROM participants WHERE id=?", [participant_id],
            |r| Ok((r.get::<_, Option<Vec<u8>>>(0)?, r.get(1)?))).optional()?.and_then(|(b, n)| b.map(|b| (b, n)));
        let (merged, n) = match existing {
            Some((blob, n0)) if n0 > 0 => {
                let old: Vec<f32> = blob.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
                let total = n0 as f32 + n_new as f32;
                (old.iter().zip(centroid).map(|(a, b)| (a * n0 as f32 + b * n_new as f32) / total).collect::<Vec<f32>>(), n0 + n_new as i64)
            }
            _ => (centroid.to_vec(), n_new as i64),
        };
        let blob: Vec<u8> = merged.iter().flat_map(|f| f.to_le_bytes()).collect();
        c.execute("UPDATE participants SET voiceprint=?, voiceprint_n=? WHERE id=?", params![blob, n, participant_id])?;
        Ok(())
    }

    pub fn forget_voice(&self, participant_id: i64) -> Result<()> {
        self.conn.lock().execute("UPDATE participants SET voiceprint=NULL, voiceprint_n=0 WHERE id=?", [participant_id])?;
        Ok(())
    }

    pub fn delete_meeting(&self, id: i64) -> Result<Vec<String>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT clip_path FROM segments WHERE meeting_id=? AND clip_path IS NOT NULL")?;
        let clips = st.query_map([id], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
        drop(st);
        c.execute("DELETE FROM meetings WHERE id=?", [id])?;
        Ok(clips)
    }
}

fn upsert_participant(tx: &rusqlite::Transaction, name: &str, email: Option<&str>) -> Result<i64> {
    if let Some(e) = email {
        tx.execute("INSERT INTO participants(name,email) VALUES(?,?) ON CONFLICT(email) DO UPDATE SET name=excluded.name", params![name, e])?;
        return Ok(tx.query_row("SELECT id FROM participants WHERE email=?", [e], |r| r.get(0))?);
    }
    if let Some(id) = tx.query_row("SELECT id FROM participants WHERE email IS NULL AND name=?", [name], |r| r.get::<_, i64>(0)).optional()? {
        return Ok(id);
    }
    tx.execute("INSERT INTO participants(name) VALUES(?)", [name])?;
    Ok(tx.last_insert_rowid())
}

fn participants_of(c: &Connection, meeting_id: i64) -> Result<Vec<ParticipantRef>> {
    let mut st = c.prepare("SELECT p.id, p.name, p.email, mp.speaker_id, p.voiceprint IS NOT NULL FROM meeting_participants mp \
                            JOIN participants p ON p.id = mp.participant_id WHERE mp.meeting_id=? ORDER BY p.name")?;
    let rows = st.query_map([meeting_id], |r| Ok(ParticipantRef {
        id: r.get(0)?, name: r.get(1)?, email: r.get(2)?, speaker_id: r.get::<_, Option<i64>>(3)?.map(|x| x as u32), has_voiceprint: r.get(4)?,
    }))?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn role_str(r: SourceRole) -> &'static str { match r { SourceRole::Local => "local", SourceRole::Remote => "remote" } }

fn row_to_segment(r: &rusqlite::Row) -> rusqlite::Result<Segment> {
    let tokens: String = r.get("tokens")?;
    let role: String = r.get("role")?;
    Ok(Segment {
        id: r.get("id")?,
        speaker: SpeakerRef { id: r.get::<_, i64>("speaker_id")? as u32, label: r.get("speaker_label")?, confidence: 1.0 },
        role: if role == "local" { SourceRole::Local } else { SourceRole::Remote },
        started_ms: r.get::<_, i64>("started_ms")? as u64,
        ended_ms: r.get::<_, i64>("ended_ms")? as u64,
        source_lang: r.get("source_lang")?, source_text: r.get("source_text")?,
        target_lang: r.get("target_lang")?, target_text: r.get("target_text")?,
        tokens: serde_json::from_str(&tokens).unwrap_or_default(),
        clip_path: r.get("clip_path")?,
        is_final: true, revision: 0,
        arrived_at: r.get::<_, i64>("arrived_at")? as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn seg(id: &str, spk: u32, text: &str, target: &str) -> Segment {
        Segment { id: id.into(), speaker: SpeakerRef { id: spk, label: format!("Speaker {spk}"), confidence: 0.7 }, role: SourceRole::Remote,
            started_ms: 0, ended_ms: 1000, source_lang: "ru".into(), source_text: text.into(), target_lang: "en".into(), target_text: target.into(),
            tokens: vec![], clip_path: None, is_final: true, revision: 0, arrived_at: 1 }
    }

    #[test]
    fn meeting_lifecycle_assign_and_search() {
        let db = Db::in_memory().unwrap();
        let cfg = SessionConfig::default();
        let m = db.start_meeting("Untitled", None, &cfg, 1_700_000_000).unwrap();
        db.link_event(m, "evt1", "Russian lesson", &[("Anna".into(), Some("anna@example.com".into())), ("Ed".into(), Some("ed@example.com".into()))]).unwrap();
        db.insert_segment(m, &seg("a", 1, "Я читаю книгу дома.", "I am reading a book at home.")).unwrap();
        db.insert_segment(m, &seg("b", 1, "Звучит полезно.", "Sounds useful.")).unwrap();

        db.assign_speaker(m, 1, "Anna", Some("anna@example.com")).unwrap();
        let (summary, segs) = db.meeting(m).unwrap().unwrap();
        assert_eq!(summary.title, "Russian lesson");
        assert_eq!(summary.participants.iter().find(|p| p.name == "Anna").unwrap().speaker_id, Some(1));
        assert!(segs.iter().all(|s| s.speaker.label == "Anna"), "labels backfilled");

        let hits = db.search("книгу", 10).unwrap();
        assert_eq!(hits.len(), 1); assert_eq!(hits[0].segment.id, "a");
        let hits = db.search("useful", 10).unwrap();
        assert_eq!(hits[0].meeting_title, "Russian lesson");
        assert_eq!(db.search("dom*", 10).unwrap().len(), 0, "only Latin prefix on Latin text");
    }

    #[test]
    fn voiceprint_is_opt_in_and_averages() {
        let db = Db::in_memory().unwrap();
        let cfg = SessionConfig::default();
        let m = db.start_meeting("x", None, &cfg, 0).unwrap();
        let pid = db.assign_speaker(m, 1, "Anna", None).unwrap();
        assert!(!db.remember_voices());
        db.update_voiceprint(pid, &[1.0, 0.0], 2).unwrap();
        db.update_voiceprint(pid, &[0.0, 1.0], 2).unwrap();
        let v = db.voiceprints().unwrap();
        assert_eq!(v[0].2, vec![0.5, 0.5]);
        db.set_remember_voices(false).unwrap();
        assert!(db.voiceprints().unwrap().is_empty(), "turning it off wipes stored voices");
    }
}
