//! Pause, Merge-Aufträge, Recherchen und Planungen in `~/.nexus/dispatch/state.json`;
//! portiert aus `packages/core/src/dispatch/state/DispatchStateStore.ts`. Änderungen laufen
//! nacheinander, damit zwei gleichzeitige Änderungen sich nicht überschreiben.

use std::fs;
use std::io;
use std::sync::Arc;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::clock::Clock;
use crate::error::{Error, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;

/// Stand eines Merge-Auftrags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub enum MergePhase {
    /// Wartet auf den nächsten Merge-Versuch
    #[default]
    Pending,
    /// Wartet auf etwas anderes (laufende Git-Aktion, Reste im Worktree …)
    Waiting,
    /// Eine Merge-Session arbeitet daran
    Session,
}

/// Merge-Auftrag eines erledigten Tickets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct MergeEntry {
    pub ticket_id: String,
    pub branch: String,
    pub phase: MergePhase,
    /// true, sobald für diesen Merge-Versuch eine Merge-Session gestartet wurde
    pub session_used: bool,
    /// Merge-Sessions für den aktuellen Stand des Zielbranches; ältere Dateien kennen nur
    /// `session_used` und laden mit 1, wenn es gesetzt ist
    #[serde(default)]
    pub sessions: u8,
    /// „Merge wartet auf …“ wurde schon kommentiert
    pub wait_noted: bool,
    /// Ziel-Branch der Merge-Session
    pub target: Option<String>,
    /// Commit des Ziel-Branches beim Start der Merge-Session; gegen ihn wird die Session
    /// ausgewertet, denn das Ziel darf währenddessen weiterlaufen (andere Tickets werden gemergt)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target_head: Option<String>,
    /// Der letzte Merge-Versuch hatte einen Konflikt, die Merge-Session durfte aber nicht starten
    /// (aus, Pause, außerhalb); bis sie starten darf, wird nicht erneut gemergt. „Erneut mergen“
    /// setzt den Merker zurück.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub conflict_seen: Option<bool>,
    /// Art des zuletzt kommentierten Grundes, warum der Merge auf seinen Ordner wartet: `busy`
    /// (laufende Git-Aktion), `staged` (vorgemerkte), `overlap` (überschneidende lokale
    /// Änderungen) oder `leftover` (Reste im Merge-Worktree); nur eine neue Art wird erneut
    /// kommentiert. „Erneut mergen“ setzt sie zurück.
    #[serde(
        default,
        deserialize_with = "crate::dispatch::types::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub held_by: Option<Option<String>>,
    pub since: String,
}

/// Recherche-Läufe einer Frage
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ResearchEntry {
    pub question_id: String,
    pub last_started_at: String,
    /// Zahl der gestarteten Recherche-Läufe (von Hand und automatisch)
    pub count: i64,
    /// Zeitpunkt des automatischen Anstoßes; die Automatik stößt jede Frage nur einmal an
    pub auto_at: Option<String>,
}

/// Zustand der Recherche-Automatik
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "dispatch.ts")]
pub struct ResearchAutoState {
    /// Seit wann die Automatik an ist; nur Fragen, die danach entstanden sind, kommen in Frage
    /// (null = aus)
    pub since: Option<String>,
    /// Kalendertag (Ortszeit, JJJJ-MM-TT) des Tageszählers
    pub day: Option<String>,
    /// Automatisch gestartete Recherchen an diesem Tag
    pub count: i64,
    /// Nach einem Umgebungs- oder Repo-Fehler: frühester Neuversuch
    pub retry_at: Option<String>,
    /// Letzter Umgebungs- oder Repo-Fehler (wird je Text nur einmal protokolliert)
    pub last_error: Option<String>,
}

/// Planungsläufe eines Tickets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct PlanningEntry {
    pub ticket_id: String,
    pub last_started_at: String,
    /// Zahl der gestarteten Planungsläufe (von Hand und automatisch)
    pub count: i64,
    /// Zeitpunkt des automatischen Anstoßes; die Automatik stößt jedes Ticket nur einmal an
    pub auto_at: Option<String>,
}

/// Zustand der Planungs-Automatik
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "dispatch.ts")]
pub struct PlanningAutoState {
    /// Seit wann die Automatik an ist; nur Tickets, die danach dazugekommen sind, kommen in Frage
    /// (null = aus)
    pub since: Option<String>,
    /// Nach einem Umgebungs- oder Repo-Fehler: frühester Neuversuch
    pub retry_at: Option<String>,
    /// Letzter Umgebungs- oder Repo-Fehler (wird je Text nur einmal protokolliert)
    pub last_error: Option<String>,
}

/// Inhalt von `dispatch/state.json`
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "dispatch.ts")]
pub struct DispatchState {
    pub paused_until: Option<String>,
    /// Pausen einzelner Claude-Accounts (Nutzungslimit): Account-ID → ISO-Zeitpunkt
    #[ts(type = "Record<string, string>")]
    pub account_pauses: IndexMap<String, String>,
    #[ts(type = "Record<string, MergeEntry>")]
    pub merges: IndexMap<String, MergeEntry>,
    #[ts(type = "Record<string, ResearchEntry>")]
    pub research: IndexMap<String, ResearchEntry>,
    pub research_auto: ResearchAutoState,
    /// Recherchen des Autopilots ohne Empfehlung je Frage (Frage-ID → Zahl der Versuche)
    #[ts(type = "Record<string, number>")]
    pub autopilot_research: IndexMap<String, u32>,
    #[ts(type = "Record<string, PlanningEntry>")]
    pub planning: IndexMap<String, PlanningEntry>,
    pub planning_auto: PlanningAutoState,
}

/// Liest und schreibt den Zustand des Dispatchers.
#[derive(Debug, Clone)]
pub struct DispatchStateStore {
    home: HarnessHome,
    writer: AtomicWriter,
    clock: Clock,
    /// Reiht die Änderungen auf; in TypeScript ist das die Promise-Kette `queue`
    gate: Arc<tokio::sync::Mutex<()>>,
}

impl DispatchStateStore {
    /// Store im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self::with_clock(home, Clock::system())
    }

    /// Store mit eigener Uhr (der Name der beiseitegelegten Datei trägt den Zeitpunkt)
    pub fn with_clock(home: HarnessHome, clock: Clock) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
            clock,
            gate: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// Leerer Zustand (ohne Datei oder nach kaputtem JSON)
    pub fn empty() -> DispatchState {
        DispatchState::default()
    }

    /// Aktueller Zustand; ohne Datei leer. Kaputtes JSON wird nach
    /// `state.json.corrupt-<Zeitstempel>` verschoben und gilt als leer; andere Lesefehler werfen
    pub fn read(&self) -> Result<DispatchState> {
        let file = self.home.dispatch_state_file();
        let text = match fs::read_to_string(&file) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::empty()),
            Err(error) => return Err(Error::unexpected(error)),
        };
        match serde_json::from_str::<Option<DispatchState>>(&text) {
            // `null` in der Datei zählt wie eine leere Datei (wie `raw?.…` in TypeScript)
            Ok(state) => {
                let mut state = state.unwrap_or_default();
                // Ältere Dateien ohne Zähler: eine benutzte Merge-Session zählt als eine
                for entry in state.merges.values_mut() {
                    if entry.session_used && entry.sessions == 0 {
                        entry.sessions = 1;
                    }
                }
                Ok(state)
            }
            Err(_) => {
                let stamp = self.clock.iso().replace([':', '.'], "-");
                let mut aside = file.clone().into_os_string();
                aside.push(format!(".corrupt-{stamp}"));
                let _ = fs::rename(&file, aside);
                Ok(Self::empty())
            }
        }
    }

    /// Liest, ändert und schreibt in einem Zug; Aufrufe warten aufeinander
    pub async fn update(&self, mutate: impl FnOnce(&mut DispatchState)) -> Result<DispatchState> {
        let _guard = self.gate.lock().await;
        let mut state = self.read()?;
        mutate(&mut state);
        let file = self.home.dispatch_state_file();
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(Error::unexpected)?;
        }
        let text = serde_json::to_string_pretty(&state).map_err(Error::unexpected)?;
        self.writer.write(&file, &text)?;
        Ok(state)
    }
}
