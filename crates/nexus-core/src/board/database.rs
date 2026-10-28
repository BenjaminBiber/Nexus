//! Verträge zur Board-Datenbank: Filter, die 17 Repository-Traits, die Unit of Work und
//! [`BoardDatabase`]. Portiert aus `packages/core/src/board/BoardDatabase.ts`.
//!
//! # Wo die Verträge liegen
//!
//! In TypeScript beschreibt `packages/core` die Schnittstelle und `packages/db` setzt sie um.
//! In Rust genauso: Alle Traits dieser Datei liegen in `nexus-core`, `nexus-db` hängt davon ab
//! und gibt sie mit `pub use` weiter. `nexus-core` darf nie von `nexus-db` abhängen – sonst
//! entstünde ein Zyklus, sobald die Dienste des Fachkerns den Trait brauchen.
//!
//! # Warum die Formen so gewählt sind
//!
//! * [`BoardDatabase`] ist wegen der generischen `transaction` **nicht objektsicher**. Aufrufer
//!   arbeiten generisch (`D: BoardDatabase`); die Fabrik liefert später ein Enum
//!   `AnyBoardDatabase`, das an SQLite, SQL Server und die In-Memory-Datenbank weiterreicht.
//! * Die Repository-Traits sind dagegen **objektsicher** (`async_trait`), damit eine Umsetzung
//!   alle 17 Traits auf *einem* Unit-of-Work-Struct erfüllen kann: SQLite und SQL Server brauchen
//!   dafür nur eine `&mut`-Verbindung. Jede Umsetzung legt ihre `impl XRepository for …UnitOfWork`
//!   in eine eigene Datei.
//! * Die Arbeit einer Transaktion ist `Fn`, nicht `FnOnce`: SQLite wiederholt sie einmal mit
//!   Schreibsperre, wenn ein anderer Prozess dazwischen geschrieben hat
//!   (`SqliteBoardDatabase.runInTransaction`). Aufrufer klonen benötigte Werte im Closure.
//! * `Record<string, X>` aus TypeScript wird zu [`BTreeMap`]: stabile Reihenfolge für Tests.
//!
//! # Abweichungen von der Skizze des Tickets
//!
//! * Die Repository-Methoden nehmen `&mut self`, obwohl die TS-Felder einfache Objekte sind:
//!   eine Umsetzung über eine einzelne Verbindung braucht den ausschließlichen Zugriff.
//! * Filter werden als Referenz übergeben (`&TicketFilter`); die optionalen Filter von TypeScript
//!   entsprechen [`Default`], ein weggelassener Filter also `&TicketFilter::default()`.
//! * Die Arbeit der Transaktion bekommt `&mut dyn BoardUnitOfWork` mit **einer** Lebensdauer `'u`,
//!   die zugleich die des gelieferten Futures ist. Weil `'u` höherrangig gebunden ist (`for<'u>`),
//!   muss das Ergebnis `T` unabhängig von `'u` sein: `T: Send + 'static`. Das ist keine Einschränkung
//!   in der Praxis – zurückgegeben werden DTOs, keine Verweise in die Transaktion.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;

use crate::actors::AuthorKind;
use crate::board::basics::{NoteState, TicketStatus};
use crate::board::dispatch_run_types::{
    DispatchCommentCounts, DispatchConfigVersion, DispatchRun, DispatchRunFilter, DispatchRunSource,
};
use crate::board::question_types::{
    BoardQuestion, BoardQuestionComment, BoardQuestionHistoryEntry, BoardQuestionOption,
    BoardQuestionResearch, BoardQuestionResearchSummary,
};
use crate::board::search_types::{SearchRequest, SearchRows};
use crate::board::types::{
    Board, BoardUser, Comment, Deletion, HistoryEntry, Note, Project, StepProgress, Ticket,
    TicketStep,
};
use crate::error::Result;
use crate::questions::state::QuestionState;

/// Ein abgelegtes Future mit Lebensdauer – die Form, die eine Transaktionsarbeit zurückgibt
pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

// ---------------------------------------------------------------------------------------------
// Eingabeformen ohne vergebene ID (in TypeScript `Omit<…, 'id'>`)
// ---------------------------------------------------------------------------------------------

/// Ein neues Projekt; die ID und die fortlaufenden Nummern vergibt die Datenbank
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewProject {
    pub key: String,
    pub name: String,
    pub created_at: String,
    pub created_by: String,
}

/// Ein neues Board; die ID vergibt die Datenbank
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewBoard {
    pub project_id: i32,
    pub owner_id: String,
    pub name: String,
    pub repo_path: Option<String>,
    pub created_at: String,
}

/// Ein neuer Kommentar; ohne Lesemarke gilt er als noch nicht eingespeist
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewComment {
    pub ticket_id: String,
    pub author_id: String,
    pub author_name: String,
    pub body: String,
    pub created_at: String,
    pub read_by_agent_at: Option<String>,
}

/// Eine neue Notiz; die ID vergibt die Datenbank
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewNote {
    pub board_id: i32,
    pub author_id: String,
    pub author_name: String,
    /// Was auffällt, als ganzer Satz
    pub title: String,
    /// Details: wo, woran man es sieht, vermutete Ursache, Vorschlag (darf leer sein)
    pub body: String,
    pub source: Option<String>,
    pub labels: Vec<String>,
    pub seen_count: i32,
    pub seen_in: Vec<String>,
    pub state: NoteState,
    pub ticket_id: Option<String>,
    pub reason: Option<String>,
    pub created_at: String,
}

/// Eine neue Recherche zu einer Frage; die ID vergibt die Datenbank
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewQuestionResearch {
    pub question_id: String,
    pub at: String,
    pub by_id: String,
    pub by_name: String,
    pub kind: AuthorKind,
    pub context: String,
    pub options: Vec<BoardQuestionOption>,
}

/// Ein neuer Kommentar an einer Frage; die ID vergibt die Datenbank
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewQuestionComment {
    pub question_id: String,
    pub author_id: String,
    pub author_name: String,
    pub author_kind: AuthorKind,
    pub body: String,
    pub created_at: String,
}

// ---------------------------------------------------------------------------------------------
// Filter
// ---------------------------------------------------------------------------------------------

/// Auswahl von Boards; genannte Kriterien werden mit „und“ verknüpft
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoardFilter {
    pub project_id: Option<i32>,
    pub owner_id: Option<String>,
}

/// Auswahl von Tickets; genannte Kriterien werden mit „und“ verknüpft
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TicketFilter {
    pub board_ids: Option<Vec<i32>>,
    pub ids: Option<Vec<String>>,
    /// Ohne die langen Texte (Beschreibung, Akzeptanz, Plan, Handbuch, Diagramm): ein leerer Text
    /// bleibt leer, ein anderer darf durch ein Kennzeichen ersetzt kommen. Für Abhängigkeiten,
    /// Graphen und Board-Karten, die nur IDs, Status, Branch und „Text vorhanden“ brauchen;
    /// solche Tickets nie zurückschreiben
    pub without_texts: bool,
}

/// Auswahl von Notizen eines Boards
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteFilter {
    pub board_id: i32,
    pub states: Option<Vec<NoteState>>,
    pub source: Option<String>,
}

impl NoteFilter {
    /// Alle Notizen eines Boards
    pub fn of_board(board_id: i32) -> Self {
        Self {
            board_id,
            states: None,
            source: None,
        }
    }
}

/// Auswahl von Fragen; genannte Kriterien werden mit „und“ verknüpft, leere Listen ergeben
/// keine Fragen
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoardQuestionFilter {
    pub project_id: Option<i32>,
    pub ids: Option<Vec<String>>,
    /// Fragen, die mindestens eines dieser Tickets betreffen
    pub ticket_ids: Option<Vec<String>>,
    pub states: Option<Vec<QuestionState>>,
}

// ---------------------------------------------------------------------------------------------
// Die 17 Repository-Traits
// ---------------------------------------------------------------------------------------------

/// Benutzer des Boards (Tabelle `board.Users`)
#[async_trait]
pub trait UserRepository: Send {
    /// Legt den Benutzer an oder aktualisiert seinen Namen
    async fn upsert(&mut self, user: &BoardUser) -> Result<()>;
    async fn by_id(&mut self, id: &str) -> Result<Option<BoardUser>>;
    async fn list(&mut self) -> Result<Vec<BoardUser>>;
}

/// Projekte samt der fortlaufenden Nummern für Tickets und Fragen
#[async_trait]
pub trait ProjectRepository: Send {
    async fn list(&mut self) -> Result<Vec<Project>>;
    async fn by_id(&mut self, id: i32) -> Result<Option<Project>>;
    async fn by_key(&mut self, key: &str) -> Result<Option<Project>>;
    async fn insert(&mut self, project: &NewProject) -> Result<Project>;
    async fn rename(&mut self, id: i32, name: &str) -> Result<()>;
    /// Liefert die nächste freie Ticketnummer und zählt atomar weiter
    async fn take_ticket_number(&mut self, id: i32) -> Result<i32>;
    /// Liefert die nächste freie Fragenummer und zählt atomar weiter
    async fn take_question_number(&mut self, id: i32) -> Result<i32>;
}

/// Boards eines Projekts
#[async_trait]
pub trait BoardRepository: Send {
    async fn list(&mut self, filter: &BoardFilter) -> Result<Vec<Board>>;
    async fn by_id(&mut self, id: i32) -> Result<Option<Board>>;
    async fn insert(&mut self, board: &NewBoard) -> Result<Board>;
    async fn update(&mut self, board: &Board) -> Result<()>;
    async fn delete(&mut self, id: i32) -> Result<()>;
}

/// Tickets samt Labels, Voraussetzungen und Sortierung
#[async_trait]
pub trait TicketRepository: Send {
    /// `for_update` sperrt die Zeile bis zum Ende der Transaktion (SQL: UPDLOCK)
    async fn by_id(&mut self, id: &str, for_update: bool) -> Result<Option<Ticket>>;
    async fn list(&mut self, filter: &TicketFilter) -> Result<Vec<Ticket>>;
    async fn insert(&mut self, ticket: &Ticket) -> Result<()>;
    /// Schreibt das Ticket; schlägt mit `HarnessError` „conflict“ fehl, wenn in der DB nicht
    /// rev-1 steht
    async fn update(&mut self, ticket: &Ticket) -> Result<()>;
    async fn delete(&mut self, id: &str) -> Result<()>;
    /// IDs der Tickets, die dieses Ticket voraussetzen
    async fn dependents(&mut self, id: &str) -> Result<Vec<String>>;
    /// Entfernt alle Verweise anderer Tickets auf dieses Ticket und liefert deren IDs
    async fn remove_dependency_on(&mut self, id: &str) -> Result<Vec<String>>;
    /// Höchste Sortierung in einer Spalte eines Boards, `None` wenn leer
    async fn max_sort_order(&mut self, board_id: i32, status: TicketStatus) -> Result<Option<i32>>;
    /// Zahl der Tickets je Board, die weder erledigt noch „nicht mehr nötig“ sind
    /// (Boards ohne solche fehlen)
    async fn open_count_by_board(&mut self, board_ids: &[i32]) -> Result<BTreeMap<i32, i64>>;
}

/// Kommentare an Tickets
#[async_trait]
pub trait CommentRepository: Send {
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<Comment>>;
    async fn insert(&mut self, comment: &NewComment) -> Result<Comment>;
    /// Zahl der Kommentare je Ticket eines Boards (Tickets ohne Kommentar fehlen)
    async fn count_by_board(&mut self, board_id: i32) -> Result<BTreeMap<String, i64>>;
    /// Kommentare dieser Tickets ohne Lesemarke, in Reihenfolge des Anlegens
    async fn unread_by_agent(&mut self, ticket_ids: &[String]) -> Result<Vec<Comment>>;
    /// Setzt die Lesemarke der Kommentare (bereits gesetzte bleiben unverändert)
    async fn mark_read_by_agent(&mut self, ids: &[i64], at: &str) -> Result<()>;
}

/// Abhakbare Arbeitsschritte eines Tickets
#[async_trait]
pub trait TicketStepRepository: Send {
    /// Schritte eines Tickets in ihrer Reihenfolge
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<TicketStep>>;
    /// Ersetzt alle Schritte eines Tickets (leere Liste entfernt sie)
    async fn replace(&mut self, ticket_id: &str, steps: &[TicketStep]) -> Result<()>;
    /// Setzt den Haken eines Schritts (Position ab 0); `false`, wenn es ihn nicht gibt
    async fn set_done(&mut self, ticket_id: &str, position: i32, done: bool) -> Result<bool>;
    /// Erledigte und alle Schritte je Ticket; Tickets ohne Schritte fehlen
    async fn progress(&mut self, ticket_ids: &[String]) -> Result<BTreeMap<String, StepProgress>>;
}

/// Verknüpfung von Claude-Sessions mit Tickets
#[async_trait]
pub trait TicketSessionRepository: Send {
    /// Verknüpft eine Session mit einem Ticket; `false`, wenn sie es schon war
    async fn add(&mut self, ticket_id: &str, session_id: &str, at: &str) -> Result<bool>;
    /// Sessions je Ticket in Reihenfolge der Verknüpfung; Tickets ohne Session fehlen
    async fn by_tickets(&mut self, ticket_ids: &[String]) -> Result<BTreeMap<String, Vec<String>>>;
    /// Tickets, mit denen die Session verknüpft ist (sortiert)
    async fn tickets_of(&mut self, session_id: &str) -> Result<Vec<String>>;
}

/// Verlauf eines Tickets
#[async_trait]
pub trait HistoryRepository: Send {
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<HistoryEntry>>;
    async fn insert(&mut self, entries: &[HistoryEntry]) -> Result<()>;
}

/// Notizen eines Boards
#[async_trait]
pub trait NoteRepository: Send {
    async fn list(&mut self, filter: &NoteFilter) -> Result<Vec<Note>>;
    /// `for_update` sperrt die Zeile bis zum Ende der Transaktion (SQL: UPDLOCK)
    async fn by_id(&mut self, id: i32, for_update: bool) -> Result<Option<Note>>;
    async fn insert(&mut self, note: &NewNote) -> Result<Note>;
    async fn update(&mut self, note: &Note) -> Result<()>;
}

/// Papierkorb gelöschter Tickets
#[async_trait]
pub trait DeletionRepository: Send {
    async fn insert(&mut self, deletion: &Deletion) -> Result<()>;
    async fn list(&mut self) -> Result<Vec<Deletion>>;
}

/// Fragen an den Nutzer
#[async_trait]
pub trait BoardQuestionRepository: Send {
    /// `for_update` sperrt die Zeile bis zum Ende der Transaktion (SQL: UPDLOCK)
    async fn by_id(&mut self, id: &str, for_update: bool) -> Result<Option<BoardQuestion>>;
    /// Sortiert nach Projekt und Nummer
    async fn list(&mut self, filter: &BoardQuestionFilter) -> Result<Vec<BoardQuestion>>;
    async fn insert(&mut self, question: &BoardQuestion) -> Result<()>;
    /// Schreibt die Frage samt Tags, Tickets und Abhängigkeiten; „conflict“, wenn in der DB
    /// nicht rev-1 steht
    async fn update(&mut self, question: &BoardQuestion) -> Result<()>;
    /// IDs der Fragen, die das Ticket betreffen oder davon abhängen (sortiert); nur lesend
    async fn ids_referencing_ticket(&mut self, ticket_id: &str) -> Result<Vec<String>>;
    /// Entfernt alle Bezüge (betroffen, Abhängigkeit) auf ein Ticket; liefert die IDs der
    /// betroffenen Fragen (sortiert), Rev bleibt
    async fn remove_ticket(&mut self, ticket_id: &str) -> Result<Vec<String>>;
}

/// Recherchen zu einer Frage
#[async_trait]
pub trait BoardQuestionResearchRepository: Send {
    /// Recherchen in Reihenfolge des Anlegens, Optionen nach Nummer
    async fn list(&mut self, question_id: &str) -> Result<Vec<BoardQuestionResearch>>;
    async fn insert(&mut self, research: &NewQuestionResearch) -> Result<BoardQuestionResearch>;
    /// Zahl der Recherchen und Optionen der neuesten je Frage (Fragen ohne Recherche fehlen)
    async fn summaries(
        &mut self,
        question_ids: &[String],
    ) -> Result<BTreeMap<String, BoardQuestionResearchSummary>>;
}

/// Kommentare an einer Frage
#[async_trait]
pub trait BoardQuestionCommentRepository: Send {
    async fn list(&mut self, question_id: &str) -> Result<Vec<BoardQuestionComment>>;
    async fn insert(&mut self, comment: &NewQuestionComment) -> Result<BoardQuestionComment>;
}

/// Verlauf einer Frage
#[async_trait]
pub trait BoardQuestionHistoryRepository: Send {
    async fn list(&mut self, question_id: &str) -> Result<Vec<BoardQuestionHistoryEntry>>;
    async fn insert(&mut self, entries: &[BoardQuestionHistoryEntry]) -> Result<()>;
}

/// Suche über alle Projekte und Boards; nur lesend
#[async_trait]
pub trait BoardSearchRepository: Send {
    /// Felder von Tickets, Kommentaren, Fragen, Fragen-Kommentaren und Notizen, in denen alle
    /// Begriffe vorkommen (Groß/Klein egal, Teilwörter zählen); je Quelle (Tabelle und Feld)
    /// höchstens `per_source` Zeilen, neueste zuerst
    async fn find(&mut self, request: &SearchRequest) -> Result<SearchRows>;
}

/// Läufe des Dispatchers und die Fassungen seiner Einstellung
#[async_trait]
pub trait DispatchRunRepository: Send {
    /// Legt den Lauf an; `false`, wenn es die ID schon gibt (doppelte Erfassung)
    async fn insert(&mut self, run: &DispatchRun) -> Result<bool>;
    /// Läufe mit Start im Zeitraum, ältester zuerst
    async fn list(&mut self, filter: &DispatchRunFilter) -> Result<Vec<DispatchRun>>;
    /// Läufe aller Projekte mit Start in [from, to) (ISO, UTC), ältester zuerst
    async fn list_all(&mut self, from: &str, to: &str) -> Result<Vec<DispatchRun>>;
    /// Frühester Start eines Laufs dieser Herkunft im Projekt; `None` ohne Lauf
    async fn first_started(
        &mut self,
        project_id: i32,
        source: DispatchRunSource,
    ) -> Result<Option<String>>;
    /// IDs der vorhandenen Läufe unter diesen IDs (für idempotentes Nachtragen)
    async fn existing(&mut self, ids: &[String]) -> Result<Vec<String>>;
    /// Legt die Konfig-Version an, wenn es den Hash noch nicht gibt; `false`, wenn schon vorhanden
    async fn record_config_version(&mut self, version: &DispatchConfigVersion) -> Result<bool>;
    /// Versionen, die in [from, to) zum ersten Mal auftraten, älteste zuerst
    async fn list_config_versions(
        &mut self,
        from: &str,
        to: &str,
    ) -> Result<Vec<DispatchConfigVersion>>;
}

/// Grundlagen der Dispatcher-Auswertung
#[async_trait]
pub trait DispatchStatsRepository: Send {
    /// Tickets des Projekts (optional Board), deren letzter Wechsel nach Erledigt in [from, to)
    /// liegt (ISO, UTC)
    async fn done_ticket_ids(&mut self, filter: &DispatchRunFilter) -> Result<Vec<String>>;
    /// Statuswechsel und Blockiert-Gründe (Felder `status`, `blockedReason`) dieser Tickets,
    /// in Reihenfolge des Eintrags
    async fn status_history(&mut self, ticket_ids: &[String]) -> Result<Vec<HistoryEntry>>;
    /// Zahl der Kommentare „[dispatch] Start…“ und „[dispatch] Abbruch…“ je Ticket
    /// (Tickets ohne fehlen)
    async fn dispatch_comment_counts(
        &mut self,
        ticket_ids: &[String],
    ) -> Result<BTreeMap<String, DispatchCommentCounts>>;
    /// Kommentare „[dispatch] …“ der Tickets des Projekts, nach ID
    async fn dispatch_comments(&mut self, project_id: i32) -> Result<Vec<Comment>>;
}

// ---------------------------------------------------------------------------------------------
// Unit of Work und Datenbank
// ---------------------------------------------------------------------------------------------

/// Alle Repositories innerhalb einer Transaktion. Die Namen entsprechen den Feldern von
/// `BoardUnitOfWork` in TypeScript in `snake_case`.
///
/// Eine Umsetzung darf alle 17 Traits auf demselben Struct erfüllen und hier jeweils `self`
/// zurückgeben; die Zugriffe leihen deshalb ausschließlich (`&mut self`).
pub trait BoardUnitOfWork: Send {
    fn users(&mut self) -> &mut dyn UserRepository;
    fn projects(&mut self) -> &mut dyn ProjectRepository;
    fn boards(&mut self) -> &mut dyn BoardRepository;
    fn tickets(&mut self) -> &mut dyn TicketRepository;
    fn comments(&mut self) -> &mut dyn CommentRepository;
    fn steps(&mut self) -> &mut dyn TicketStepRepository;
    fn ticket_sessions(&mut self) -> &mut dyn TicketSessionRepository;
    fn history(&mut self) -> &mut dyn HistoryRepository;
    fn notes(&mut self) -> &mut dyn NoteRepository;
    fn deletions(&mut self) -> &mut dyn DeletionRepository;
    fn questions(&mut self) -> &mut dyn BoardQuestionRepository;
    fn question_research(&mut self) -> &mut dyn BoardQuestionResearchRepository;
    fn question_comments(&mut self) -> &mut dyn BoardQuestionCommentRepository;
    fn question_history(&mut self) -> &mut dyn BoardQuestionHistoryRepository;
    fn search(&mut self) -> &mut dyn BoardSearchRepository;
    fn dispatch_runs(&mut self) -> &mut dyn DispatchRunRepository;
    fn dispatch_stats(&mut self) -> &mut dyn DispatchStatsRepository;
}

/// Die Arbeit einer Transaktion: bekommt die Unit of Work geliehen und liefert ein abgelegtes
/// Future. `Fn` statt `FnOnce`, weil SQLite die Arbeit bei einem Schreibkonflikt wiederholt.
///
/// Als eigener Trait geschrieben, damit Aufrufer `W: TransactionWork<T>` statt der ganzen
/// `for<'u> Fn(…)`-Schranke notieren können.
pub trait TransactionWork<T>:
    for<'u> Fn(&'u mut dyn BoardUnitOfWork) -> BoxFut<'u, Result<T>> + Send + Sync
{
}

impl<T, F> TransactionWork<T> for F where
    F: for<'u> Fn(&'u mut dyn BoardUnitOfWork) -> BoxFut<'u, Result<T>> + Send + Sync
{
}

/// Zugang zur Board-Datenbank: jede fachliche Aktion läuft in genau einer Transaktion.
///
/// Der Trait ist wegen der generischen `transaction` nicht objektsicher – Aufrufer arbeiten
/// generisch über `D: BoardDatabase`, die Fabrik liefert ein Enum über die Anbieter.
pub trait BoardDatabase: Send + Sync {
    /// Führt die Arbeit in genau einer Transaktion aus. Die Arbeit ist `Fn`: SQLite wiederholt
    /// sie einmal mit Schreibsperre, wenn ein anderer Prozess dazwischen geschrieben hat.
    /// Aufrufer klonen benötigte Werte im Closure – deshalb ist die Arbeit `'static`.
    ///
    /// `'static` statt `'a`: Eine Umsetzung, die eine andere Datenbank umhüllt (etwa um
    /// Zugriffe mitzuschreiben), muss die Arbeit in ein eigenes Closure mit höherrangig
    /// gebundener Lebensdauer (`for<'u>`) einpacken; dafür muss die Arbeit beliebig lange
    /// leben. Alle Aufrufer erfüllen das ohnehin, weil `tx!` nur eigene Kopien einschließt.
    fn transaction<'a, T, W>(&'a self, work: W) -> impl Future<Output = Result<T>> + Send + 'a
    where
        T: Send + 'static,
        W: TransactionWork<T> + 'static;

    /// Kennwert für den Stand eines Boards (Tickets, Kommentare, Notizen, Board, Fragen zu
    /// seinen Tickets)
    fn change_marker(&self, board_id: i32) -> impl Future<Output = Result<String>> + Send + '_;

    /// Kennwert für den Stand der Fragen eines Projekts; ändert sich bei jeder Änderung einer
    /// seiner Fragen
    fn question_marker(&self, project_id: i32) -> impl Future<Output = Result<String>> + Send + '_;

    /// Schließt Verbindungen und gibt Ressourcen frei
    fn close(&self) -> impl Future<Output = Result<()>> + Send + '_;
}

/// Schreibt `db.transaction(|u| Box::pin(async move { … }))` kürzer als `tx!(db, |u| { … })`.
///
/// Der Rumpf darf `u` benutzen und muss `Result<T>` liefern; `?` funktioniert wie gewohnt.
///
/// Weil die Arbeit `Fn` ist und die Lebensdauer `'u` höherrangig gebunden, darf der Rumpf nur
/// Werte benutzen, die er selbst herstellt oder die ewig leben. Alles andere wird vorher
/// geklont – dafür gibt es die zweite Form `tx!(db, [wert, …], |u| { … })`: sie klont die
/// genannten Werte bei jedem Aufruf der Arbeit und gibt dem Rumpf die Kopien unter demselben
/// Namen.
///
/// ```
/// # use nexus_core::board::database::{BoardDatabase, BoardUnitOfWork};
/// # use nexus_core::board::in_memory::InMemoryBoardDatabase;
/// # use nexus_core::board::types::BoardUser;
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let db = InMemoryBoardDatabase::new();
/// let user = BoardUser { id: "u-1".into(), name: "Florian".into() };
/// nexus_core::tx!(db, [user], |u| { u.users().upsert(&user).await }).await.unwrap();
/// # });
/// ```
#[macro_export]
macro_rules! tx {
    ($db:expr, |$uow:ident| $body:block) => {
        $db.transaction(
            move |$uow: &mut dyn $crate::board::database::BoardUnitOfWork| {
                ::std::boxed::Box::pin(async move { $body })
            },
        )
    };
    ($db:expr, [$($wert:ident),+ $(,)?], |$uow:ident| $body:block) => {{
        // Einmal für die Arbeit kopieren – der Aufrufer behält seine Werte …
        $(let $wert = ::std::clone::Clone::clone(&$wert);)+
        $db.transaction(
            move |$uow: &mut dyn $crate::board::database::BoardUnitOfWork| {
                // … und einmal je Aufruf, weil die Arbeit mehrfach laufen darf
                $(let $wert = ::std::clone::Clone::clone(&$wert);)+
                ::std::boxed::Box::pin(async move { $body })
            },
        )
    }};
}

#[cfg(test)]
mod tests;
