//! Die offenen Ideen-Entwürfe je Chat: im Speicher und in `<home>/telegram/drafts.json`.
//!
//! Je Chat gibt es höchstens einen Entwurf mit einer kurzen ID; die ID steckt in den Knöpfen
//! unter der Bestätigungsnachricht. Nach [`IDEA_EXPIRY_HOURS`] gilt ein Entwurf als
//! abgelaufen – der Knopf löst dann nichts mehr aus. Geschrieben wird atomar wie beim
//! Laufzeit-Zustand des Pollers ([`nexus_core::telegram::TelegramStateStore`]), damit ein
//! Neustart mitten im Schreiben keine halbe Datei hinterlässt.
//!
//! Kommt eine neue Idee, während noch ein Entwurf offen ist, wird sie nicht sofort
//! ausgewertet: sie wartet als [`IdeaChat::pending`] am Chat, bis der Nutzer entschieden hat,
//! ob der alte Entwurf weg darf.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use chrono::{DateTime, TimeDelta, Utc};
use nexus_core::clock::Clock;
use nexus_core::locking::AtomicWriter;
use nexus_core::telegram::idea::IdeaDraft;
use nexus_core::telegram::Questionnaire;
use nexus_core::{Error, HarnessHome, Result};
use serde::{Deserialize, Serialize};

/// So lange lässt sich ein Entwurf noch anlegen oder korrigieren
pub const IDEA_EXPIRY_HOURS: i64 = 24;

/// Ein offener Entwurf eines Chats
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaEntry {
    /// Kurze ID; sie steckt in den Knöpfen
    pub id: String,
    pub draft: IdeaDraft,
    /// Wann der Entwurf entstand (ISO); davon hängt der Ablauf ab
    pub created_at: String,
    /// Der Fragebogen zu den Rückfragen, solange sie einzeln durchgegangen werden
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub questionnaire: Option<Questionnaire>,
}

/// Was ein Chat an Ideen offen hat
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct IdeaChat {
    /// Der Entwurf, über den die Bestätigungsnachricht spricht
    pub draft: Option<IdeaEntry>,
    /// Eine neue Idee, die auf die Entscheidung über den alten Entwurf wartet
    pub pending: Option<String>,
}

impl IdeaChat {
    /// Nichts offen? Dann braucht der Chat keinen Eintrag in der Datei
    fn empty(&self) -> bool {
        self.draft.is_none() && self.pending.is_none()
    }
}

/// Der Inhalt von `drafts.json`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct IdeaDrafts {
    /// Je Chat (als Text, weil JSON-Schlüssel Text sind) höchstens ein Entwurf
    chats: BTreeMap<String, IdeaChat>,
}

/// Hält die offenen Entwürfe je Chat und schreibt sie nach jeder Änderung.
pub struct IdeaDraftStore {
    home: HarnessHome,
    clock: Clock,
    writer: AtomicWriter,
    chats: Mutex<BTreeMap<i64, IdeaChat>>,
    /// Zähler für die kurzen IDs; der Start hängt an der Uhr, damit ein Knopf aus dem Lauf
    /// vor dem Neustart nicht auf einen neuen Entwurf zeigt
    next_id: AtomicU32,
}

impl std::fmt::Debug for IdeaDraftStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdeaDraftStore")
            .field("offen", &self.chats.lock().map(|chats| chats.len()).ok())
            .finish_non_exhaustive()
    }
}

impl IdeaDraftStore {
    /// Store im Benutzerordner; die Datei wird gleich gelesen, damit Entwürfe einen Neustart
    /// überstehen. Eine kaputte Datei gilt als „keine Entwürfe“ – ein verlorener Entwurf
    /// kostet nur eine erneute Ansage
    pub fn new(home: HarnessHome, clock: Clock) -> Self {
        let store = Self {
            home,
            clock,
            writer: AtomicWriter::new(),
            chats: Mutex::new(BTreeMap::new()),
            next_id: AtomicU32::new(0),
        };
        let loaded = store.load();
        *store.chats() = loaded;
        store.next_id.store(
            store.clock.now().timestamp_subsec_millis(),
            Ordering::Relaxed,
        );
        store
    }

    /// Der offene Entwurf des Chats; ein abgelaufener wird dabei weggeräumt
    pub fn get(&self, chat_id: i64) -> Option<IdeaEntry> {
        self.clean(chat_id);
        self.chats()
            .get(&chat_id)
            .and_then(|chat| chat.draft.clone())
    }

    /// Der Entwurf, wenn er noch offen ist und diese ID trägt; `None` heißt „veralteter
    /// Knopf“
    pub fn with_id(&self, chat_id: i64, draft_id: &str) -> Option<IdeaEntry> {
        self.get(chat_id).filter(|entry| entry.id == draft_id)
    }

    /// Legt einen neuen Entwurf an und verdrängt damit den alten; die wartende Idee ist
    /// danach abgearbeitet
    pub fn put(&self, chat_id: i64, draft: IdeaDraft) -> Result<IdeaEntry> {
        let entry = IdeaEntry {
            id: self.new_id(),
            draft,
            created_at: self.clock.iso(),
            questionnaire: None,
        };
        {
            let mut chats = self.chats();
            chats.insert(
                chat_id,
                IdeaChat {
                    draft: Some(entry.clone()),
                    pending: None,
                },
            );
        }
        self.save()?;
        Ok(entry)
    }

    /// Ersetzt den Entwurf unter derselben ID (Ziel umgeschaltet, Board gewechselt,
    /// Korrekturrunde); `false`, wenn der Chat diesen Entwurf nicht mehr hält
    pub fn replace(&self, chat_id: i64, draft_id: &str, draft: IdeaDraft) -> Result<bool> {
        {
            let mut chats = self.chats();
            let Some(entry) = chats
                .get_mut(&chat_id)
                .and_then(|chat| chat.draft.as_mut())
                .filter(|entry| entry.id == draft_id)
            else {
                return Ok(false);
            };
            entry.draft = draft;
        }
        self.save()?;
        Ok(true)
    }

    /// Setzt den Fragebogen des Entwurfs unter dieser ID (`None` räumt ihn weg); `false`, wenn
    /// der Chat diesen Entwurf nicht mehr hält
    pub fn set_questionnaire(
        &self,
        chat_id: i64,
        draft_id: &str,
        questionnaire: Option<Questionnaire>,
    ) -> Result<bool> {
        {
            let mut chats = self.chats();
            let Some(entry) = chats
                .get_mut(&chat_id)
                .and_then(|chat| chat.draft.as_mut())
                .filter(|entry| entry.id == draft_id)
            else {
                return Ok(false);
            };
            entry.questionnaire = questionnaire;
        }
        self.save()?;
        Ok(true)
    }

    /// Räumt den Entwurf des Chats weg (angelegt, verworfen); die wartende Idee bleibt
    pub fn remove(&self, chat_id: i64) -> Result<()> {
        {
            let mut chats = self.chats();
            if let Some(chat) = chats.get_mut(&chat_id) {
                chat.draft = None;
            }
            chats.retain(|_, chat| !chat.empty());
        }
        self.save()
    }

    /// Merkt eine neue Idee, die auf die Entscheidung über den offenen Entwurf wartet
    pub fn hold(&self, chat_id: i64, input: &str) -> Result<()> {
        {
            let mut chats = self.chats();
            chats.entry(chat_id).or_default().pending = Some(input.to_string());
        }
        self.save()
    }

    /// Nimmt die wartende Idee ab; `None`, wenn keine wartet
    pub fn take_pending(&self, chat_id: i64) -> Result<Option<String>> {
        let waiting = {
            let mut chats = self.chats();
            let waiting = chats.get_mut(&chat_id).and_then(|chat| chat.pending.take());
            chats.retain(|_, chat| !chat.empty());
            waiting
        };
        self.save()?;
        Ok(waiting)
    }

    /// Wirft alles weg, was dieser Chat offen hat
    pub fn clear(&self, chat_id: i64) -> Result<()> {
        self.chats().remove(&chat_id);
        self.save()
    }

    /// Nimmt den abgelaufenen Entwurf des Chats weg; `true`, wenn einer abgelaufen war
    fn clean(&self, chat_id: i64) -> bool {
        let expired = {
            let mut chats = self.chats();
            let expired = chats
                .get(&chat_id)
                .and_then(|chat| chat.draft.as_ref())
                .is_some_and(|entry| self.expired(entry));
            if expired {
                if let Some(chat) = chats.get_mut(&chat_id) {
                    chat.draft = None;
                }
                chats.retain(|_, chat| !chat.empty());
            }
            expired
        };
        if expired {
            // Ein Schreibfehler darf den Chat nicht blockieren; im Speicher ist der Entwurf weg
            let _ = self.save();
        }
        expired
    }

    /// Wartet der Entwurf zu lange? Ein unlesbarer Zeitpunkt gilt als abgelaufen – lieber
    /// einmal neu ansagen als auf einem alten Entwurf anlegen
    fn expired(&self, entry: &IdeaEntry) -> bool {
        let Ok(created) = DateTime::parse_from_rfc3339(&entry.created_at) else {
            return true;
        };
        self.clock.now() - created.with_timezone(&Utc) >= TimeDelta::hours(IDEA_EXPIRY_HOURS)
    }

    /// Die nächste kurze ID
    fn new_id(&self) -> String {
        format!("d{:x}", self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Liest die Datei; ohne Datei und bei kaputtem JSON gibt es keine Entwürfe
    fn load(&self) -> BTreeMap<i64, IdeaChat> {
        let text = match fs::read_to_string(self.home.telegram_drafts_file()) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return BTreeMap::new(),
            Err(_) => return BTreeMap::new(),
        };
        let drafts: IdeaDrafts = serde_json::from_str(&text).unwrap_or_default();
        drafts
            .chats
            .into_iter()
            .filter_map(|(chat_id, chat)| chat_id.parse::<i64>().ok().map(|id| (id, chat)))
            .filter(|(_, chat)| !chat.empty())
            .collect()
    }

    /// Schreibt den Stand atomar
    fn save(&self) -> Result<()> {
        let drafts = IdeaDrafts {
            chats: self
                .chats()
                .iter()
                .map(|(chat_id, chat)| (chat_id.to_string(), chat.clone()))
                .collect(),
        };
        fs::create_dir_all(self.home.telegram_dir()).map_err(Error::unexpected)?;
        let text = serde_json::to_string_pretty(&drafts).map_err(Error::unexpected)?;
        self.writer.write(&self.home.telegram_drafts_file(), &text)
    }

    /// Die offenen Entwürfe; eine vergiftete Sperre wäre ein Fehler im Server
    fn chats(&self) -> std::sync::MutexGuard<'_, BTreeMap<i64, IdeaChat>> {
        self.chats.lock().expect("Sperre der Ideen-Entwürfe")
    }
}

#[cfg(test)]
mod tests {
    use nexus_core::telegram::idea::{IdeaKind, IdeaTargetRef};

    use super::*;

    /// Beginn aller Tests
    const START: &str = "2026-11-03T12:00:00Z";

    /// Der Chat der Tests
    const CHAT: i64 = 42;

    /// Ein Entwurf mit diesem Titel
    fn draft(title: &str) -> IdeaDraft {
        IdeaDraft {
            summary: "Der Cache läuft nie ab.".into(),
            target: IdeaTargetRef {
                kind: IdeaKind::Note,
                board_id: 3,
                ticket_id: None,
            },
            title: title.into(),
            details: "Gesehen in der Board-Ansicht.".into(),
            related: Vec::new(),
            duplicates: Vec::new(),
            questions: Vec::new(),
            confidence: 0.8,
            transcript: "Der Cache läuft nie ab.".into(),
            rounds: 1,
        }
    }

    /// Store im Temp-Ordner samt der Zahl der Stunden, die der Test vorstellen kann
    fn store() -> (
        tempfile::TempDir,
        IdeaDraftStore,
        std::sync::Arc<std::sync::atomic::AtomicI64>,
    ) {
        let dir = tempfile::tempdir().expect("Benutzerordner");
        let hours = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
        let read = hours.clone();
        let start: DateTime<Utc> = START.parse().expect("Zeitpunkt");
        let clock = Clock::from_fn(move || start + TimeDelta::hours(read.load(Ordering::Relaxed)));
        let store = IdeaDraftStore::new(HarnessHome::at(dir.path()), clock);
        (dir, store, hours)
    }

    #[test]
    fn haelt_je_chat_hoechstens_einen_entwurf() {
        let (_dir, store, _) = store();

        let erster = store.put(CHAT, draft("Cache läuft nie ab")).unwrap();
        let zweiter = store.put(CHAT, draft("Spalte fehlt")).unwrap();

        assert_ne!(erster.id, zweiter.id, "jeder Entwurf hat seine eigene ID");
        assert_eq!(store.get(CHAT).map(|entry| entry.id), Some(zweiter.id));
        assert_eq!(store.get(99), None, "ein anderer Chat hat nichts offen");
    }

    #[test]
    fn findet_den_entwurf_nur_unter_seiner_eigenen_id() {
        let (_dir, store, _) = store();
        let entry = store.put(CHAT, draft("Cache läuft nie ab")).unwrap();

        assert!(store.with_id(CHAT, &entry.id).is_some());
        assert_eq!(store.with_id(CHAT, "dfremd"), None);
        assert_eq!(store.with_id(99, &entry.id), None);
    }

    #[test]
    fn ersetzt_den_entwurf_unter_derselben_id() {
        let (_dir, store, _) = store();
        let entry = store.put(CHAT, draft("Cache läuft nie ab")).unwrap();

        assert!(store
            .replace(CHAT, &entry.id, draft("Cache läuft nach 5 Minuten ab"))
            .unwrap());

        let offen = store.get(CHAT).expect("der Entwurf ist offen");
        assert_eq!(
            offen.id, entry.id,
            "die ID bleibt, die Knöpfe gelten weiter"
        );
        assert_eq!(offen.draft.title, "Cache läuft nach 5 Minuten ab");
        assert!(
            !store.replace(CHAT, "dfremd", draft("fremd")).unwrap(),
            "eine fremde ID ändert nichts"
        );
    }

    #[test]
    fn nach_vierundzwanzig_stunden_ist_der_entwurf_abgelaufen() {
        let (_dir, store, stunden) = store();
        store.put(CHAT, draft("Cache läuft nie ab")).unwrap();

        stunden.store(IDEA_EXPIRY_HOURS - 1, Ordering::Relaxed);
        assert!(store.get(CHAT).is_some());

        stunden.store(IDEA_EXPIRY_HOURS, Ordering::Relaxed);
        assert_eq!(store.get(CHAT), None);
        assert_eq!(
            fs::read_to_string(store.home.telegram_drafts_file()).unwrap(),
            "{\n  \"chats\": {}\n}",
            "der abgelaufene Entwurf ist auch aus der Datei weg"
        );
    }

    #[test]
    fn eine_wartende_idee_bleibt_bis_sie_abgeholt_wird() {
        let (_dir, store, _) = store();
        store.put(CHAT, draft("Cache läuft nie ab")).unwrap();

        store.hold(CHAT, "Und die Spalte fehlt auch").unwrap();

        assert!(store.get(CHAT).is_some(), "der alte Entwurf bleibt offen");
        assert_eq!(
            store.take_pending(CHAT).unwrap(),
            Some("Und die Spalte fehlt auch".into())
        );
        assert_eq!(store.take_pending(CHAT).unwrap(), None);
    }

    #[test]
    fn ein_neuer_entwurf_erledigt_die_wartende_idee() {
        let (_dir, store, _) = store();
        store.put(CHAT, draft("Cache läuft nie ab")).unwrap();
        store.hold(CHAT, "Und die Spalte fehlt auch").unwrap();

        store.put(CHAT, draft("Spalte fehlt")).unwrap();

        assert_eq!(store.take_pending(CHAT).unwrap(), None);
    }

    #[test]
    fn entwuerfe_ueberstehen_einen_neustart() {
        let (dir, store, _) = store();
        let entry = store.put(CHAT, draft("Cache läuft nie ab")).unwrap();
        store.hold(CHAT, "Und die Spalte fehlt auch").unwrap();

        // Ein zweiter Store auf demselben Ordner ist der Bot nach dem Neustart
        let start: DateTime<Utc> = START.parse().expect("Zeitpunkt");
        let nach_neustart = IdeaDraftStore::new(HarnessHome::at(dir.path()), Clock::fixed(start));

        let offen = nach_neustart.get(CHAT).expect("der Entwurf ist noch da");
        assert_eq!(offen.id, entry.id);
        assert_eq!(offen.draft.title, "Cache läuft nie ab");
        assert_eq!(
            nach_neustart.take_pending(CHAT).unwrap(),
            Some("Und die Spalte fehlt auch".into())
        );
    }

    #[test]
    fn der_fragebogen_haengt_am_entwurf_und_uebersteht_einen_neustart() {
        let (dir, store, _) = store();
        let entry = store.put(CHAT, draft("Cache läuft nie ab")).unwrap();
        assert_eq!(entry.questionnaire, None);

        let mut bogen = Questionnaire::new(2);
        bogen.current = 1;
        assert!(store
            .set_questionnaire(CHAT, &entry.id, Some(bogen.clone()))
            .unwrap());
        assert!(
            !store.set_questionnaire(CHAT, "dfremd", None).unwrap(),
            "eine fremde ID ändert nichts"
        );

        let start: DateTime<Utc> = START.parse().expect("Zeitpunkt");
        let nach_neustart = IdeaDraftStore::new(HarnessHome::at(dir.path()), Clock::fixed(start));
        assert_eq!(
            nach_neustart
                .get(CHAT)
                .and_then(|entry| entry.questionnaire),
            Some(bogen)
        );
    }

    #[test]
    fn liest_entwuerfe_von_vor_den_fragebogen() {
        let dir = tempfile::tempdir().expect("Benutzerordner");
        fs::create_dir_all(dir.path().join("telegram")).unwrap();
        let alt = serde_json::json!({ "chats": { "42": { "draft": {
            "id": "d1",
            "createdAt": START,
            "draft": {
                "summary": "s", "target": { "kind": "note", "boardId": 3, "ticketId": null },
                "title": "t", "details": "", "related": [], "duplicates": [],
                "questions": ["Auch für SQL Server?"], "confidence": 0.5,
                "transcript": "x", "rounds": 1
            }
        } } } });
        fs::write(
            dir.path().join("telegram").join("drafts.json"),
            alt.to_string(),
        )
        .unwrap();

        let store = IdeaDraftStore::new(
            HarnessHome::at(dir.path()),
            Clock::fixed(START.parse().expect("Zeitpunkt")),
        );

        let entry = store.get(CHAT).expect("der alte Entwurf ist lesbar");
        assert_eq!(entry.draft.questions[0].text, "Auch für SQL Server?");
        assert!(entry.draft.questions[0].options.is_empty());
        assert_eq!(entry.questionnaire, None);
    }

    #[test]
    fn verwerfen_und_anlegen_raeumen_den_entwurf_weg() {
        let (dir, store, _) = store();
        store.put(CHAT, draft("Cache läuft nie ab")).unwrap();

        store.remove(CHAT).unwrap();

        assert_eq!(store.get(CHAT), None);
        assert!(
            !fs::read_to_string(dir.path().join("telegram").join("drafts.json"))
                .unwrap()
                .contains("Cache"),
            "auch die Datei ist leer"
        );
    }

    #[test]
    fn beginnt_bei_kaputter_datei_ohne_entwuerfe() {
        let dir = tempfile::tempdir().expect("Benutzerordner");
        fs::create_dir_all(dir.path().join("telegram")).unwrap();
        fs::write(dir.path().join("telegram").join("drafts.json"), "{kaputt").unwrap();

        let store = IdeaDraftStore::new(
            HarnessHome::at(dir.path()),
            Clock::fixed(START.parse().expect("Zeitpunkt")),
        );

        assert_eq!(store.get(CHAT), None);
    }
}
