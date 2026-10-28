//! Gemeinsame Teile der Gegenprobe TypeScript ↔ Rust: der feste Datensatz, das Anlegen und
//! Auslesen über die Rust-Repositories, der Aufruf des Node-Skripts und der Vergleich der
//! Abzüge. Anbieterunabhängig – `sqlite_crosscheck.rs` arbeitet damit auf einer Datei,
//! `mssql_crosscheck.rs` auf der SQL-Server-Test-Datenbank.
//!
//! Beide Seiten legen den festen Datensatz aus `tests/crosscheck/dataset.json` an und lesen ihn
//! über ihre Repositories wieder aus. Verglichen wird kanonisches JSON (Schlüssel sortiert):
//! `serde_json::Value` hält seine Objekte in einer `BTreeMap`, das Node-Skript sortiert selbst.
//!
//! Die TypeScript-Seite ist `tests/crosscheck/crosscheck.mjs`; sie arbeitet über
//! `packages/db/dist`, deshalb braucht jeder Fall vorher `npm run build`. Die Fälle sind
//! darum `#[ignore]` und laufen nur mit `-- --ignored`.
//!
//! Nur stdout des Node-Skripts trägt das Ergebnis – Node schreibt seine
//! „ExperimentalWarning“ zu `node:sqlite` nach stderr.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Map, Value};

use nexus_core::actors::AuthorKind;
use nexus_core::board::basics::NoteState;
use nexus_core::board::database::{
    BoardDatabase, BoardFilter, BoardQuestionFilter, NewBoard, NewComment, NewNote, NewProject,
    NewQuestionComment, NewQuestionResearch, NoteFilter, TicketFilter,
};
use nexus_core::board::dispatch_run_types::{
    DispatchConfigVersion, DispatchRun, DispatchRunFilter, DispatchRunSource,
};
use nexus_core::board::question_types::{BoardQuestion, BoardQuestionHistoryEntry};
use nexus_core::board::search_types::SearchRequest;
use nexus_core::board::types::{BoardUser, Deletion, HistoryEntry, Ticket, TicketStep};
use nexus_core::questions::QuestionState;
use nexus_core::{tx, Error, Result};

/// Der Datensatz, den beide Seiten anlegen; als Text eingebettet, damit der Test ihn nicht
/// über einen Pfad suchen muss
const DATASET: &str = include_str!("../crosscheck/dataset.json");

// =============================================================================== Datensatz

/// Ein neues Projekt, wie es in `dataset.json` steht (ohne ID und Nummernzähler)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectInput {
    key: String,
    name: String,
    created_at: String,
    created_by: String,
}

/// Ein neues Board, wie es in `dataset.json` steht (ohne ID)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BoardInput {
    project_id: i32,
    owner_id: String,
    name: String,
    repo_path: Option<String>,
    created_at: String,
}

/// Die Schritte eines Tickets
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StepsInput {
    ticket_id: String,
    steps: Vec<TicketStep>,
}

/// Ein nachträglich abgehakter Schritt
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StepDoneInput {
    ticket_id: String,
    position: i32,
    done: bool,
}

/// Eine Verknüpfung von Ticket und Session
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionInput {
    ticket_id: String,
    session_id: String,
    at: String,
}

/// Ein neuer Kommentar, wie er in `dataset.json` steht (ohne ID)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentInput {
    ticket_id: String,
    author_id: String,
    author_name: String,
    body: String,
    created_at: String,
    read_by_agent_at: Option<String>,
}

/// Eine neue Notiz, wie sie in `dataset.json` steht (ohne ID)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NoteInput {
    board_id: i32,
    author_id: String,
    author_name: String,
    title: String,
    body: String,
    source: Option<String>,
    labels: Vec<String>,
    seen_count: i32,
    seen_in: Vec<String>,
    state: NoteState,
    ticket_id: Option<String>,
    reason: Option<String>,
    created_at: String,
}

/// Eine neue Recherche, wie sie in `dataset.json` steht (ohne ID)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResearchInput {
    question_id: String,
    at: String,
    by_id: String,
    by_name: String,
    kind: AuthorKind,
    context: String,
    options: Vec<nexus_core::board::question_types::BoardQuestionOption>,
}

/// Ein neuer Kommentar an einer Frage, wie er in `dataset.json` steht (ohne ID)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuestionCommentInput {
    question_id: String,
    author_id: String,
    author_name: String,
    author_kind: AuthorKind,
    body: String,
    created_at: String,
}

/// Der ganze Datensatz; die Reihenfolge der Listen ist die Reihenfolge des Anlegens
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dataset {
    users: Vec<BoardUser>,
    projects: Vec<ProjectInput>,
    boards: Vec<BoardInput>,
    tickets: Vec<Ticket>,
    steps: Vec<StepsInput>,
    steps_done: Vec<StepDoneInput>,
    sessions: Vec<SessionInput>,
    comments: Vec<CommentInput>,
    history: Vec<HistoryEntry>,
    notes: Vec<NoteInput>,
    deletions: Vec<Deletion>,
    questions: Vec<BoardQuestion>,
    question_research: Vec<ResearchInput>,
    question_comments: Vec<QuestionCommentInput>,
    question_history: Vec<BoardQuestionHistoryEntry>,
    dispatch_runs: Vec<DispatchRun>,
    config_versions: Vec<DispatchConfigVersion>,
    search: SearchRequest,
    dispatch_filter: DispatchRunFilter,
}

impl Dataset {
    /// IDs der Tickets in der Reihenfolge des Datensatzes
    pub fn ticket_ids(&self) -> Vec<String> {
        self.tickets.iter().map(|t| t.id.clone()).collect()
    }

    /// IDs der Fragen in der Reihenfolge des Datensatzes
    pub fn question_ids(&self) -> Vec<String> {
        self.questions.iter().map(|q| q.id.clone()).collect()
    }

    /// Die von der Datenbank vergebenen Board-IDs (1 … n)
    pub fn board_ids(&self) -> Vec<i32> {
        (1..=self.boards.len() as i32).collect()
    }

    /// Die von der Datenbank vergebenen Projekt-IDs (1 … n)
    pub fn project_ids(&self) -> Vec<i32> {
        (1..=self.projects.len() as i32).collect()
    }
}

/// Der eingebettete Datensatz, einmal gelesen
pub fn dataset() -> Arc<Dataset> {
    Arc::new(serde_json::from_str(DATASET).expect("dataset.json"))
}

// =============================================================================== Anlegen

/// Legt den festen Datensatz über die Rust-Repositories an; jeder Abschnitt in seiner eigenen
/// Transaktion, Reihenfolge wie im Node-Skript
pub async fn seed_rust<D: BoardDatabase>(db: &D) -> Result<()> {
    let data = dataset();

    tx!(db, [data], |u| {
        for user in &data.users {
            u.users().upsert(user).await?;
        }
        for (index, project) in data.projects.iter().enumerate() {
            let angelegt = u
                .projects()
                .insert(&NewProject {
                    key: project.key.clone(),
                    name: project.name.clone(),
                    created_at: project.created_at.clone(),
                    created_by: project.created_by.clone(),
                })
                .await?;
            expect_id("Projekt", &project.key, angelegt.id, index as i32 + 1)?;
        }
        for (index, board) in data.boards.iter().enumerate() {
            let angelegt = u
                .boards()
                .insert(&NewBoard {
                    project_id: board.project_id,
                    owner_id: board.owner_id.clone(),
                    name: board.name.clone(),
                    repo_path: board.repo_path.clone(),
                    created_at: board.created_at.clone(),
                })
                .await?;
            expect_id("Board", &board.name, angelegt.id, index as i32 + 1)?;
        }
        Ok(())
    })
    .await?;

    tx!(db, [data], |u| {
        for ticket in &data.tickets {
            u.tickets().insert(ticket).await?;
        }
        for eintrag in &data.steps {
            u.steps()
                .replace(&eintrag.ticket_id, &eintrag.steps)
                .await?;
        }
        for eintrag in &data.steps_done {
            u.steps()
                .set_done(&eintrag.ticket_id, eintrag.position, eintrag.done)
                .await?;
        }
        for eintrag in &data.sessions {
            u.ticket_sessions()
                .add(&eintrag.ticket_id, &eintrag.session_id, &eintrag.at)
                .await?;
        }
        for comment in &data.comments {
            u.comments()
                .insert(&NewComment {
                    ticket_id: comment.ticket_id.clone(),
                    author_id: comment.author_id.clone(),
                    author_name: comment.author_name.clone(),
                    body: comment.body.clone(),
                    created_at: comment.created_at.clone(),
                    read_by_agent_at: comment.read_by_agent_at.clone(),
                })
                .await?;
        }
        u.history().insert(&data.history).await?;
        for note in &data.notes {
            u.notes()
                .insert(&NewNote {
                    board_id: note.board_id,
                    author_id: note.author_id.clone(),
                    author_name: note.author_name.clone(),
                    title: note.title.clone(),
                    body: note.body.clone(),
                    source: note.source.clone(),
                    labels: note.labels.clone(),
                    seen_count: note.seen_count,
                    seen_in: note.seen_in.clone(),
                    state: note.state,
                    ticket_id: note.ticket_id.clone(),
                    reason: note.reason.clone(),
                    created_at: note.created_at.clone(),
                })
                .await?;
        }
        for deletion in &data.deletions {
            u.deletions().insert(deletion).await?;
        }
        Ok(())
    })
    .await?;

    tx!(db, [data], |u| {
        for question in &data.questions {
            u.questions().insert(question).await?;
        }
        for research in &data.question_research {
            u.question_research()
                .insert(&NewQuestionResearch {
                    question_id: research.question_id.clone(),
                    at: research.at.clone(),
                    by_id: research.by_id.clone(),
                    by_name: research.by_name.clone(),
                    kind: research.kind,
                    context: research.context.clone(),
                    options: research.options.clone(),
                })
                .await?;
        }
        for comment in &data.question_comments {
            u.question_comments()
                .insert(&NewQuestionComment {
                    question_id: comment.question_id.clone(),
                    author_id: comment.author_id.clone(),
                    author_name: comment.author_name.clone(),
                    author_kind: comment.author_kind,
                    body: comment.body.clone(),
                    created_at: comment.created_at.clone(),
                })
                .await?;
        }
        u.question_history().insert(&data.question_history).await?;
        for run in &data.dispatch_runs {
            u.dispatch_runs().insert(run).await?;
        }
        for version in &data.config_versions {
            u.dispatch_runs().record_config_version(version).await?;
        }
        Ok(())
    })
    .await?;

    Ok(())
}

/// Die fortlaufenden IDs müssen auf beiden Seiten gleich sein – sonst stimmt die Reihenfolge
/// des Anlegens nicht
fn expect_id(art: &str, name: &str, ist: i32, erwartet: i32) -> Result<()> {
    if ist == erwartet {
        return Ok(());
    }
    Err(Error::unexpected_message(format!(
        "{art} {name} bekam Id {ist}, erwartet {erwartet}"
    )))
}

// =============================================================================== Auslesen

/// Liest alles über die Rust-Repositories und liefert dasselbe JSON wie `node … dump`
pub async fn dump_rust<D: BoardDatabase>(db: &D) -> Result<Value> {
    let data = dataset();

    let inhalt = tx!(db, [data], |u| {
        let ticket_ids = data.ticket_ids();
        let question_ids = data.question_ids();
        let board_ids = data.board_ids();
        let project_ids = data.project_ids();
        let filter = &data.dispatch_filter;
        let mut m = Map::new();

        m.insert("users".into(), json(u.users().list().await?)?);
        m.insert(
            "userById".into(),
            json(u.users().by_id(&data.users[0].id).await?)?,
        );
        m.insert("projects".into(), json(u.projects().list().await?)?);
        m.insert(
            "projectByKey".into(),
            json(u.projects().by_key(&data.projects[0].key).await?)?,
        );
        m.insert(
            "boards".into(),
            json(u.boards().list(&BoardFilter::default()).await?)?,
        );
        m.insert(
            "boardsOfProject".into(),
            json(
                u.boards()
                    .list(&BoardFilter {
                        project_id: Some(project_ids[0]),
                        owner_id: None,
                    })
                    .await?,
            )?,
        );
        m.insert(
            "boardsOfOwner".into(),
            json(
                u.boards()
                    .list(&BoardFilter {
                        project_id: None,
                        owner_id: Some(data.users[1].id.clone()),
                    })
                    .await?,
            )?,
        );
        m.insert(
            "tickets".into(),
            json(u.tickets().list(&TicketFilter::default()).await?)?,
        );
        m.insert(
            "ticketsWithoutTexts".into(),
            json(
                u.tickets()
                    .list(&TicketFilter {
                        without_texts: true,
                        ..TicketFilter::default()
                    })
                    .await?,
            )?,
        );
        m.insert(
            "ticketsOfBoard".into(),
            json(
                u.tickets()
                    .list(&TicketFilter {
                        board_ids: Some(vec![board_ids[0]]),
                        ..TicketFilter::default()
                    })
                    .await?,
            )?,
        );
        m.insert(
            "maxSortOrder".into(),
            json(
                u.tickets()
                    .max_sort_order(
                        board_ids[0],
                        nexus_core::board::basics::TicketStatus::InProgress,
                    )
                    .await?,
            )?,
        );
        m.insert(
            "openCountByBoard".into(),
            json(u.tickets().open_count_by_board(&board_ids).await?)?,
        );
        m.insert(
            "commentCountsByBoard".into(),
            json(u.comments().count_by_board(board_ids[0]).await?)?,
        );
        m.insert(
            "unreadComments".into(),
            json(u.comments().unread_by_agent(&ticket_ids).await?)?,
        );
        m.insert(
            "stepProgress".into(),
            json(u.steps().progress(&ticket_ids).await?)?,
        );
        m.insert(
            "sessionsByTicket".into(),
            json(u.ticket_sessions().by_tickets(&ticket_ids).await?)?,
        );
        m.insert(
            "ticketsOfSession".into(),
            json(u.ticket_sessions().tickets_of("sess-a").await?)?,
        );
        m.insert("noteById".into(), json(u.notes().by_id(1, false).await?)?);
        m.insert("deletions".into(), json(u.deletions().list().await?)?);
        m.insert(
            "questions".into(),
            json(u.questions().list(&BoardQuestionFilter::default()).await?)?,
        );
        m.insert(
            "questionsOfTicket".into(),
            json(
                u.questions()
                    .list(&BoardQuestionFilter {
                        ticket_ids: Some(vec![ticket_ids[0].clone()]),
                        ..BoardQuestionFilter::default()
                    })
                    .await?,
            )?,
        );
        m.insert(
            "openQuestions".into(),
            json(
                u.questions()
                    .list(&BoardQuestionFilter {
                        states: Some(vec![QuestionState::Open]),
                        ..BoardQuestionFilter::default()
                    })
                    .await?,
            )?,
        );
        m.insert(
            "questionIdsReferencingTicket".into(),
            json(u.questions().ids_referencing_ticket(&ticket_ids[0]).await?)?,
        );
        m.insert(
            "researchSummaries".into(),
            json(u.question_research().summaries(&question_ids).await?)?,
        );
        m.insert(
            "dispatchRuns".into(),
            json(u.dispatch_runs().list(filter).await?)?,
        );
        m.insert(
            "dispatchRunsAll".into(),
            json(u.dispatch_runs().list_all(&filter.from, &filter.to).await?)?,
        );
        m.insert(
            "firstStartedDispatcher".into(),
            json(
                u.dispatch_runs()
                    .first_started(filter.project_id, DispatchRunSource::Dispatcher)
                    .await?,
            )?,
        );
        m.insert(
            "firstStartedComments".into(),
            json(
                u.dispatch_runs()
                    .first_started(filter.project_id, DispatchRunSource::Comments)
                    .await?,
            )?,
        );
        let mut run_ids: Vec<String> = data.dispatch_runs.iter().map(|r| r.id.clone()).collect();
        run_ids.push("gibt-es-nicht".into());
        m.insert(
            "existingRuns".into(),
            json(u.dispatch_runs().existing(&run_ids).await?)?,
        );
        m.insert(
            "configVersions".into(),
            json(
                u.dispatch_runs()
                    .list_config_versions(&filter.from, &filter.to)
                    .await?,
            )?,
        );
        m.insert(
            "doneTicketIds".into(),
            json(u.dispatch_stats().done_ticket_ids(filter).await?)?,
        );
        m.insert(
            "statusHistory".into(),
            json(u.dispatch_stats().status_history(&ticket_ids).await?)?,
        );
        m.insert(
            "dispatchCommentCounts".into(),
            json(
                u.dispatch_stats()
                    .dispatch_comment_counts(&ticket_ids)
                    .await?,
            )?,
        );
        m.insert(
            "dispatchComments".into(),
            json(
                u.dispatch_stats()
                    .dispatch_comments(filter.project_id)
                    .await?,
            )?,
        );
        m.insert("search".into(), json(u.search().find(&data.search).await?)?);

        // Je Ticket, Board und Frage eine eigene Karte – wie im Node-Skript
        let mut ticket_by_id = Map::new();
        let mut dependents = Map::new();
        let mut comments = Map::new();
        let mut steps = Map::new();
        let mut history = Map::new();
        for id in &ticket_ids {
            ticket_by_id.insert(id.clone(), json(u.tickets().by_id(id, false).await?)?);
            dependents.insert(id.clone(), json(u.tickets().dependents(id).await?)?);
            comments.insert(id.clone(), json(u.comments().list(id).await?)?);
            steps.insert(id.clone(), json(u.steps().list(id).await?)?);
            history.insert(id.clone(), json(u.history().list(id).await?)?);
        }
        m.insert("ticketById".into(), Value::Object(ticket_by_id));
        m.insert("dependents".into(), Value::Object(dependents));
        m.insert("comments".into(), Value::Object(comments));
        m.insert("steps".into(), Value::Object(steps));
        m.insert("history".into(), Value::Object(history));

        let mut notes_by_board = Map::new();
        for id in &board_ids {
            notes_by_board.insert(
                id.to_string(),
                json(u.notes().list(&NoteFilter::of_board(*id)).await?)?,
            );
        }
        m.insert("notesByBoard".into(), Value::Object(notes_by_board));

        let mut question_by_id = Map::new();
        let mut research = Map::new();
        let mut question_comments = Map::new();
        let mut question_history = Map::new();
        for id in &question_ids {
            question_by_id.insert(id.clone(), json(u.questions().by_id(id, false).await?)?);
            research.insert(id.clone(), json(u.question_research().list(id).await?)?);
            question_comments.insert(id.clone(), json(u.question_comments().list(id).await?)?);
            question_history.insert(id.clone(), json(u.question_history().list(id).await?)?);
        }
        m.insert("questionById".into(), Value::Object(question_by_id));
        m.insert("research".into(), Value::Object(research));
        m.insert("questionComments".into(), Value::Object(question_comments));
        m.insert("questionHistory".into(), Value::Object(question_history));

        Ok(Value::Object(m))
    })
    .await?;

    // Die beiden Kennwerte lesen ohne eigene Transaktion – wie auf der TypeScript-Seite
    let mut inhalt = match inhalt {
        Value::Object(map) => map,
        _ => unreachable!("dump liefert ein Objekt"),
    };
    let mut change_marker = Map::new();
    for id in data.board_ids() {
        change_marker.insert(id.to_string(), Value::String(db.change_marker(id).await?));
    }
    inhalt.insert("changeMarker".into(), Value::Object(change_marker));
    let mut question_marker = Map::new();
    for id in data.project_ids() {
        question_marker.insert(id.to_string(), Value::String(db.question_marker(id).await?));
    }
    inhalt.insert("questionMarker".into(), Value::Object(question_marker));
    Ok(Value::Object(inhalt))
}

/// Ein Ergebnis als JSON; ein Fehler beim Serialisieren ist ein Programmfehler
fn json<T: serde::Serialize>(wert: T) -> Result<Value> {
    serde_json::to_value(wert).map_err(Error::unexpected)
}

// =============================================================================== Node

/// Wurzel des Repositorys (zwei Ebenen über `crates/nexus-db`)
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("Wurzel des Repositorys")
        .to_path_buf()
}

/// Das Node-Skript der Gegenprobe
fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/crosscheck/crosscheck.mjs")
}

/// Bricht mit einer klaren Meldung ab, solange `packages/db/dist` fehlt
fn require_dist() {
    let dist = repo_root().join("packages/db/dist/index.js");
    assert!(
        dist.exists(),
        "{} fehlt – vorher `npm run build` ausführen",
        dist.display()
    );
}

/// Startet das Node-Skript, ohne auf sein Ende zu warten
pub fn node_spawn(args: &[&str]) -> std::process::Child {
    require_dist();
    let mut command = Command::new("node");
    command
        .arg(script())
        .args(args)
        .current_dir(repo_root())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
        .spawn()
        .expect("node starten (braucht Node >= 22.13)")
}

/// Wartet auf das Node-Skript und liest sein JSON von stdout
pub fn node_result(child: std::process::Child, was: &str) -> Value {
    let ausgabe = child.wait_with_output().expect("node abwarten");
    assert!(
        ausgabe.status.success(),
        "node {was} scheiterte: {}",
        String::from_utf8_lossy(&ausgabe.stderr)
    );
    serde_json::from_slice(&ausgabe.stdout).unwrap_or_else(|error| {
        panic!(
            "node {was} lieferte kein JSON ({error}): {}",
            String::from_utf8_lossy(&ausgabe.stdout)
        )
    })
}

/// Führt das Node-Skript aus und liefert sein JSON
pub fn node(args: &[&str]) -> Value {
    node_result(node_spawn(args), args[0])
}

// =============================================================================== Vergleich

/// Zeigt die erste Abweichung zweier JSON-Bäume als Pfad an – sonst ist der Unterschied
/// zwischen zwei großen Abzügen kaum zu finden
fn first_difference(links: &Value, rechts: &Value, pfad: &str) -> Option<String> {
    match (links, rechts) {
        (Value::Object(a), Value::Object(b)) => {
            let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let pfad = format!("{pfad}.{key}");
                match (a.get(key), b.get(key)) {
                    (Some(x), Some(y)) => {
                        if let Some(treffer) = first_difference(x, y, &pfad) {
                            return Some(treffer);
                        }
                    }
                    (x, y) => {
                        return Some(format!("{pfad}: Rust {x:?} ≠ TypeScript {y:?}"));
                    }
                }
            }
            None
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return Some(format!(
                    "{pfad}: Rust hat {} Einträge, TypeScript {}",
                    a.len(),
                    b.len()
                ));
            }
            for (index, (x, y)) in a.iter().zip(b).enumerate() {
                if let Some(treffer) = first_difference(x, y, &format!("{pfad}[{index}]")) {
                    return Some(treffer);
                }
            }
            None
        }
        (a, b) if a == b => None,
        (a, b) => Some(format!("{pfad}: Rust {a} ≠ TypeScript {b}")),
    }
}

/// Vergleicht zwei Abzüge und zeigt bei Abweichung die erste Fundstelle
pub fn assert_gleich(rust: &Value, typescript: &Value, was: &str) {
    if let Some(unterschied) = first_difference(rust, typescript, was) {
        panic!("{was}: Abzüge unterscheiden sich – {unterschied}");
    }
}
