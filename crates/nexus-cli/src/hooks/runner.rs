//! Ablauf eines Hooks: zuordnen, Kontext einspeisen, Umgebung setzen, an den Server melden.
//! Portiert aus `packages/cli/src/hooks/HookRunner.ts`.
//!
//! Zuordnung und Kontext kommen aus der Board-DB; ohne DB, ohne Board des Repos oder wenn die
//! DB nicht innerhalb des Zeitbudgets antwortet, bleiben sie leer. Umgebung, Session-Name und
//! Meldung an den Server hängen nicht von der DB ab. Der Hook wirft nie.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use nexus_core::board::comment_service::CommentService;
use nexus_core::board::repo_board_resolver::RepoBoardOwner;
use nexus_core::board::ticket_service::TicketService;
use nexus_core::clock::Clock;
use nexus_core::hooks::{
    AssignInput, ContextBuilder, ContextKind, EnvFileWriter, HookEventFactory,
    QuestionContextMarks, TicketAssigner,
};
use nexus_core::project::{HarnessLayout, TokenStore};
use nexus_core::registry::DEFAULT_SERVER_PORT;
use nexus_core::sessions::{HookEvent, HookKind, SessionTitle};
use nexus_core::workflow::{WorkflowPhase, WorkflowSource};
use nexus_core::{HarnessHome, HarnessProject};
use serde_json::{json, Value};

use crate::hooks::board_access::{BoardAccess, BoardOpener};
use crate::server::{PortState, ServerClient, ServerLauncher};

/// Zeit, die Zuordnung, Kontext und das Markieren der Kommentare zusammen höchstens brauchen
/// dürfen (`DEFAULT_BOARD_BUDGET_MS` in TypeScript)
pub const DEFAULT_BOARD_BUDGET_MS: u64 = 2000;

/// Wartezeit auf einen vom Hook gestarteten Server (`DEFAULT_SERVER_START_MS`)
pub const DEFAULT_SERVER_START_MS: u64 = 2500;

/// Name des Ereignisses in der Ausgabe an Claude Code; die übrigen Ereignisse geben nichts aus
fn output_name(kind: HookKind) -> Option<&'static str> {
    match kind {
        HookKind::SessionStart => Some("SessionStart"),
        HookKind::UserPrompt => Some("UserPromptSubmit"),
        _ => None,
    }
}

/// Startet den Server und prüft den Port; in Tests ein Doppelgänger
pub trait Launcher {
    /// Zustand des Ports
    fn probe(&self, port: u16) -> PortState;

    /// Startet den Server und wartet höchstens `wait` auf ihn
    fn start(&self, port: u16, wait: Duration);
}

impl Launcher for ServerLauncher {
    fn probe(&self, port: u16) -> PortState {
        ServerLauncher::probe(self, port)
    }

    fn start(&self, port: u16, wait: Duration) {
        // Startet der Server nicht, bleibt die Meldung aus – ein Fehler ist das nicht
        let _ = self.start_waiting(port, wait);
    }
}

/// Was die Board-DB zum Hook beiträgt
#[derive(Debug, Clone, Default)]
struct BoardPart {
    tickets: Vec<String>,
    text: Option<String>,
}

/// Alles, was der Board-Thread für seine Arbeit braucht
struct BoardJob {
    home: HarnessHome,
    layout: HarnessLayout,
    kind: HookKind,
    root: String,
    cwd: String,
    session_id: String,
    env: HashMap<String, String>,
    debug: bool,
}

/// Die Verbindung zum Board-Thread: sein Ergebnis, das Zeichen nach der Ausgabe und sein Ende
struct BoardThread {
    part: Receiver<BoardPart>,
    finish: Option<Sender<()>>,
    done: Receiver<()>,
}

/// Führt einen Hook aus; die Board-Arbeit läuft in einem eigenen Thread mit Zeitbudget.
pub struct HookRunner<O: BoardOpener, L: Launcher> {
    opener: O,
    launcher: L,
    board_budget: Duration,
}

impl<O: BoardOpener, L: Launcher> HookRunner<O, L> {
    /// Hook über diesem Öffner und diesem Starter
    pub fn new(opener: O, launcher: L, board_budget: Duration) -> Self {
        Self {
            opener,
            launcher,
            board_budget,
        }
    }

    /// Führt den Hook aus. Die Ausgabe für Claude Code (JSON-Text) geht über `emit`, bevor der
    /// Server gemeldet wird, damit sie nicht von dessen Erreichbarkeit abhängt.
    pub fn run(
        self,
        kind: HookKind,
        payload: &Value,
        env: &HashMap<String, String>,
        emit: &mut dyn FnMut(&str),
    ) {
        let debug = debug_enabled(env);
        if let Err(error) = self.execute(kind, payload, env, emit) {
            if debug {
                eprintln!("Hook {}: {error}", kind.as_str());
            }
        }
    }

    /// Die eigentliche Arbeit; Fehler fängt [`HookRunner::run`]
    fn execute(
        self,
        kind: HookKind,
        payload: &Value,
        env: &HashMap<String, String>,
        emit: &mut dyn FnMut(&str),
    ) -> nexus_core::Result<()> {
        let Some(object) = payload.as_object() else {
            return Ok(());
        };
        let cwd = object
            .get("cwd")
            .and_then(Value::as_str)
            .filter(|wert| !wert.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let session_id = object
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if session_id.is_empty() {
            return Ok(());
        }
        let debug = debug_enabled(env);
        let Self {
            opener,
            launcher,
            board_budget,
        } = self;
        let deadline = Instant::now() + board_budget;
        // Ohne `harness init` endet der Hook still
        let project = HarnessProject::open(&cwd)?;
        let cwd_text = cwd.to_string_lossy().into_owned();

        let mut board = start_board(
            opener,
            BoardJob {
                home: HarnessHome::new(env),
                layout: project.layout.clone(),
                kind,
                root: project.root.clone(),
                cwd: cwd_text.clone(),
                session_id: session_id.clone(),
                env: env.clone(),
                debug,
            },
        );
        let part = match board.part.recv_timeout(remaining(deadline)) {
            Ok(part) => part,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if debug {
                    eprintln!("Hook: Board-DB antwortet nicht rechtzeitig");
                }
                BoardPart::default()
            }
            // Kein Board: keine Konfig, keine erreichbare DB oder kein Board dieses Repos
            Err(mpsc::RecvTimeoutError::Disconnected) => BoardPart::default(),
        };

        let Some(mut event) =
            HookEventFactory::create(kind, payload, &cwd_text, part.tickets.clone())
        else {
            return Ok(());
        };
        let title = Self::name_session(kind, &project, &session_id, &mut event, payload)?;
        if kind == HookKind::SessionStart {
            // Beide Namen: `NEXUS_*` für Nexus, `HARNESS_*` für alte Skills und den Node-Server
            let mut vars = Vec::new();
            for name in nexus_core::env::names("SESSION") {
                vars.push((name, session_id.clone()));
            }
            if let Some(ticket) = part.tickets.first() {
                for name in nexus_core::env::names("TICKET") {
                    vars.push((name, ticket.clone()));
                }
            }
            EnvFileWriter::append(env.get("CLAUDE_ENV_FILE").map(Path::new), &vars, env)?;
        }
        if let Some(name) = output_name(kind) {
            if part.text.is_some() || title.is_some() {
                let mut output = serde_json::Map::new();
                output.insert("hookEventName".into(), json!(name));
                if let Some(text) = &part.text {
                    output.insert("additionalContext".into(), json!(text));
                }
                if let Some(title) = &title {
                    output.insert("sessionTitle".into(), json!(title));
                }
                emit(&json!({ "hookSpecificOutput": output }).to_string());
            }
        }
        // Erst nach der Ausgabe als gelesen markieren und schließen; was nicht mehr in das
        // Budget passt, geschieht nicht – die Kommentare kommen dann beim nächsten Mal
        if let Some(signal) = board.finish.take() {
            let _ = signal.send(());
        }
        if board.done.recv_timeout(remaining(deadline)).is_err() && debug {
            eprintln!("Hook {}: Board-DB nicht rechtzeitig fertig", kind.as_str());
        }
        Self::notify(&launcher, kind, &project, &event, env);
        Ok(())
    }

    /// Nummeriert die Session („SE-001: Titel“). Liefert den Namen, der an Claude Code
    /// zurückgeht, sonst nichts.
    ///
    /// Den automatisch erzeugten Titel von Claude Code sehen Hooks nicht: Beim Start wird nur
    /// ein vorhandener eigener Titel ergänzt, beim ersten Prompt wird der Titel aus dem Prompt
    /// abgeleitet. Die Oberfläche zeigt immer mindestens „SE-001“.
    fn name_session(
        kind: HookKind,
        project: &HarnessProject,
        session_id: &str,
        event: &mut HookEvent,
        payload: &Value,
    ) -> nexus_core::Result<Option<String>> {
        if kind != HookKind::SessionStart && kind != HookKind::UserPrompt {
            return Ok(None);
        }
        let number = project.session_numbers().assign(session_id)?;
        let existing = event.title.clone();
        let name = match &existing {
            Some(title) => Some(SessionTitle::format(number, Some(title))),
            None if kind == HookKind::UserPrompt => {
                let prompt = payload.get("prompt").and_then(Value::as_str);
                let derived = SessionTitle::from_prompt(prompt);
                (!derived.is_empty()).then(|| SessionTitle::format(number, Some(&derived)))
            }
            None => None,
        };
        event.title = Some(
            name.clone()
                .unwrap_or_else(|| SessionTitle::format(number, None)),
        );
        // Unverändert → Claude Code nicht erneut umbenennen
        Ok(if name != existing { name } else { None })
    }

    /// Meldet das Ereignis an den Server; nur beim Sessionbeginn wird er bei Bedarf gestartet
    fn notify(
        launcher: &L,
        kind: HookKind,
        project: &HarnessProject,
        event: &HookEvent,
        env: &HashMap<String, String>,
    ) {
        let port = nexus_core::env::var_in(env, "PORT")
            .and_then(|raw| raw.parse::<u16>().ok())
            .filter(|port| *port >= 1)
            .unwrap_or(DEFAULT_SERVER_PORT);
        match launcher.probe(port) {
            PortState::Foreign => return,
            PortState::Free => {
                if kind != HookKind::SessionStart {
                    return;
                }
                let wait = nexus_core::env::var_in(env, "HOOK_SERVER_WAIT_MS")
                    .and_then(|raw| raw.parse::<u64>().ok())
                    .filter(|ms| *ms >= 1)
                    .unwrap_or(DEFAULT_SERVER_START_MS);
                launcher.start(port, Duration::from_millis(wait));
            }
            PortState::Ours => {}
        }
        let client = ServerClient::new(port);
        client.register_project(&project.root);
        let token = TokenStore::new(project.layout.clone())
            .read()
            .unwrap_or_default();
        let event = serde_json::to_value(event).unwrap_or(Value::Null);
        client.send_event(&project.root, &token, &event);
    }
}

/// Rest des Zeitbudgets; nie negativ
fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

/// Startet den Board-Thread; er öffnet die DB, ordnet zu, baut den Kontext und wartet danach
/// auf das Zeichen, die Kommentare als gelesen zu markieren
fn start_board<O: BoardOpener>(opener: O, job: BoardJob) -> BoardThread {
    let (part_tx, part_rx) = mpsc::channel();
    let (finish_tx, finish_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::spawn(move || board_work(opener, job, &part_tx, &finish_rx, &done_tx));
    BoardThread {
        part: part_rx,
        finish: Some(finish_tx),
        done: done_rx,
    }
}

/// Die Board-Arbeit in ihrem eigenen Thread mit eigener, einfädiger Laufzeit
fn board_work<O: BoardOpener>(
    opener: O,
    job: BoardJob,
    part: &Sender<BoardPart>,
    finish: &Receiver<()>,
    done: &Sender<()>,
) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    runtime.block_on(async {
        let Some(board) = opener.open(&job.home).await else {
            return;
        };
        let (found, mark) = collect(&board, &job).await;
        if part.send(found).is_err() {
            // Der Hook wartet nicht mehr; nur noch sauber schließen
            board.close().await;
            return;
        }
        // Auf das Zeichen nach der Ausgabe warten; gibt der Hook auf, endet das Warten mit
        // dem geschlossenen Kanal
        let _ = finish.recv();
        if let Some((builder, ids)) = mark {
            if !ids.is_empty() {
                if let Err(error) = builder.mark_read(&ids).await {
                    debug(
                        &job,
                        &format!("Kommentare nicht als gelesen markiert: {error}"),
                    );
                }
            }
        }
        board.close().await;
        let _ = done.send(());
    });
}

/// Tickets der Session und Kontext aus der Board-DB. Fehler ergeben „nichts zugeordnet, kein
/// Kontext“; ein Fehler beim Kontext lässt die Zuordnung bestehen.
#[allow(clippy::type_complexity)]
async fn collect<A: BoardAccess>(
    board: &A,
    job: &BoardJob,
) -> (BoardPart, Option<(ContextBuilder<A::Db>, Vec<i64>)>) {
    let services = board.services();
    let owner: RepoBoardOwner = board.owner();
    let repo_board = match services.boards.for_repo(&job.root, &owner).await {
        Ok(Some(found)) => found,
        Ok(None) => return (BoardPart::default(), None),
        Err(error) => {
            debug(job, &error.to_string());
            return (BoardPart::default(), None);
        }
    };
    // Die Dienste des Boards tragen keine `Clone`-Schranke auf der Datenbank, deshalb hier
    // eigene Dienste über derselben Verbindung
    let assigner = TicketAssigner::new(
        services.db.clone(),
        TicketService::new(services.db.clone(), Clock::system()),
    );
    let input = AssignInput {
        cwd: job.cwd.clone(),
        session_id: job.session_id.clone(),
        env: job.env.clone(),
        board_id: Some(repo_board.board.id),
    };
    let tickets = match assigner.resolve(&input).await {
        Ok(tickets) => tickets,
        Err(error) => {
            debug(job, &error.to_string());
            return (BoardPart::default(), None);
        }
    };
    let kind = match job.kind {
        HookKind::SessionStart => ContextKind::SessionStart,
        HookKind::UserPrompt => ContextKind::UserPrompt,
        _ => {
            return (
                BoardPart {
                    tickets,
                    text: None,
                },
                None,
            )
        }
    };
    // Angewendeter Workflow (Anpassungen), sonst der Standard
    let phases: Vec<WorkflowPhase> =
        serde_json::from_value(WorkflowSource::new(job.home.clone()).read().phases)
            .unwrap_or_default();
    let builder = ContextBuilder::with_phases(
        services.db.clone(),
        TicketService::new(services.db.clone(), Clock::system()),
        CommentService::new(services.db.clone(), Clock::system()),
        Some(QuestionContextMarks::new(
            job.layout.clone(),
            Clock::system(),
        )),
        Clock::system(),
        phases,
    );
    match builder.build(kind, &tickets, Some(&job.session_id)).await {
        Ok(context) => (
            BoardPart {
                tickets,
                text: context.text,
            },
            Some((builder, context.comment_ids)),
        ),
        Err(error) => {
            debug(job, &error.to_string());
            (
                BoardPart {
                    tickets,
                    text: None,
                },
                None,
            )
        }
    }
}

/// Soll der Hook seine Fehler melden? `NEXUS_DEBUG=1`, sonst `HARNESS_DEBUG=1`
fn debug_enabled(env: &HashMap<String, String>) -> bool {
    nexus_core::env::var_in(env, "DEBUG").as_deref() == Some("1")
}

/// Meldung auf stderr, aber nur mit `NEXUS_DEBUG=1` (oder `HARNESS_DEBUG=1`)
fn debug(job: &BoardJob, message: &str) {
    if job.debug {
        eprintln!("Hook {}: Board-DB: {message}", job.kind.as_str());
    }
}
