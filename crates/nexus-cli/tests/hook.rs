//! Der Hook-Ablauf: einmal unmittelbar über [`HookRunner`] mit einer Board-DB im Speicher und
//! einmal über das gebaute Binary `nexus hook …`.
//! Portiert aus `packages/cli/tests/unit/HookRunner.test.ts` und
//! `packages/cli/tests/integration/hook.test.ts`.

mod common;

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::temp_project::{TempProject, USER_ID};
use nexus_cli::hooks::{BoardAccess, BoardOpener, HookRunner, Launcher};
use nexus_cli::server::PortState;
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::database::{BoardDatabase, TransactionWork};
use nexus_core::board::in_memory::InMemoryBoardDatabase;
use nexus_core::board::repo_board_resolver::RepoBoardOwner;
use nexus_core::board::repo_paths::BoardRepoPaths;
use nexus_core::board::services::BoardServices;
use nexus_core::board::ticket_service::{NewTicketInput, TicketPatch, WriteOptions};
use nexus_core::clock::Clock;
use nexus_core::sessions::HookKind;
use nexus_core::testing::BoardFixture;
use nexus_core::{HarnessHome, HarnessProject};
use serde_json::{json, Value};

/// Zeitbudget der Board-DB in den Tests (wie `boardBudgetMs` in TypeScript)
const BUDGET: Duration = Duration::from_secs(2);

/// Eine geöffnete Board-Laufzeit im Speicher
struct SpeicherBoard<D: BoardDatabase> {
    services: BoardServices<D>,
}

impl<D: BoardDatabase> BoardAccess for SpeicherBoard<D> {
    type Db = D;

    fn services(&self) -> &BoardServices<D> {
        &self.services
    }

    fn owner(&self) -> RepoBoardOwner {
        RepoBoardOwner {
            user_id: USER_ID.to_string(),
            active_board: None,
        }
    }

    async fn close(&self) {}
}

/// Öffnet die Board-DB im Speicher; auf Wunsch gar nicht oder erst nach einer Wartezeit
#[derive(Clone)]
struct SpeicherOffner<D: BoardDatabase = InMemoryBoardDatabase> {
    db: Arc<D>,
    clock: Clock,
    paths: BoardRepoPaths,
    /// Wartezeit vor dem Öffnen (die hängende DB)
    delay: Option<Duration>,
    /// Keine Laufzeit, so wie ohne `board.yaml`
    leer: bool,
}

impl<D: BoardDatabase + 'static> BoardOpener for SpeicherOffner<D> {
    type Access = SpeicherBoard<D>;

    async fn open(&self, _home: &HarnessHome) -> Option<SpeicherBoard<D>> {
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        if self.leer {
            return None;
        }
        Some(SpeicherBoard {
            services: BoardServices::new(self.db.clone(), self.clock.clone(), self.paths.clone()),
        })
    }
}

/// Eine Datenbank, die zählt und ab einer bestimmten Transaktion die Verbindung verliert –
/// so wie eine DB, die mitten im Hook wegbricht
struct Bremse {
    inner: Arc<InMemoryBoardDatabase>,
    zahler: Arc<std::sync::atomic::AtomicUsize>,
    /// Ab dieser Transaktion (1-basiert) gibt es nur noch Fehler
    fehler_ab: Option<usize>,
}

impl Bremse {
    /// Dieselbe Datenbank, die nur zählt
    fn zahlend(inner: Arc<InMemoryBoardDatabase>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            zahler: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            fehler_ab: None,
        })
    }

    /// Dieselbe Datenbank, die ab der `ab`-ten Transaktion scheitert
    fn scheiternd(inner: Arc<InMemoryBoardDatabase>, ab: usize) -> Arc<Self> {
        Arc::new(Self {
            inner,
            zahler: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            fehler_ab: Some(ab),
        })
    }

    /// Zahl der bisherigen Transaktionen
    fn transaktionen(&self) -> usize {
        self.zahler.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl BoardDatabase for Bremse {
    fn transaction<'a, T, W>(
        &'a self,
        work: W,
    ) -> impl std::future::Future<Output = nexus_core::Result<T>> + Send + 'a
    where
        T: Send + 'static,
        W: TransactionWork<T> + 'static,
    {
        let nummer = self
            .zahler
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        async move {
            if self.fehler_ab.is_some_and(|ab| nummer >= ab) {
                return Err(
                    nexus_core::HarnessError::new("db-weg", "Verbindung unterbrochen").into(),
                );
            }
            self.inner.transaction(work).await
        }
    }

    fn change_marker(
        &self,
        board_id: i32,
    ) -> impl std::future::Future<Output = nexus_core::Result<String>> + Send + '_ {
        self.inner.change_marker(board_id)
    }

    fn question_marker(
        &self,
        project_id: i32,
    ) -> impl std::future::Future<Output = nexus_core::Result<String>> + Send + '_ {
        self.inner.question_marker(project_id)
    }

    fn close(&self) -> impl std::future::Future<Output = nexus_core::Result<()>> + Send + '_ {
        self.inner.close()
    }
}

/// Merkt sich die geprüften Ports und meldet immer ein fremdes Programm: kein Server wird
/// gestartet, keine Meldung geht hinaus
#[derive(Clone, Default)]
struct TestStarter {
    probes: Arc<Mutex<Vec<u16>>>,
}

impl Launcher for TestStarter {
    fn probe(&self, port: u16) -> PortState {
        self.probes.lock().unwrap().push(port);
        PortState::Foreign
    }

    fn start(&self, _port: u16, _wait: Duration) {
        panic!("ein fremder Port wird nie gestartet");
    }
}

/// Der Mensch, dessen Kommentare eingespeist werden (die eigenen gelten als gelesen)
fn human() -> nexus_core::board::basics::BoardActor {
    nexus_core::board::basics::BoardActor {
        user_id: USER_ID.to_string(),
        name: "Florian".to_string(),
    }
}

/// Die Umgebung, die der Hook sieht (Home des Testprojekts, fremder Port, Meldungen an)
fn umgebung(projekt: &TempProject, weitere: &[(&str, &str)]) -> HashMap<String, String> {
    let mut env = HashMap::from([
        (
            "HARNESS_HOME".to_string(),
            projekt.home().display().to_string(),
        ),
        ("HARNESS_PORT".to_string(), "59999".to_string()),
        ("HARNESS_DEBUG".to_string(), "1".to_string()),
    ]);
    for (name, wert) in weitere {
        env.insert((*name).to_string(), (*wert).to_string());
    }
    env
}

/// Die Testumgebung: Projekt mit Worktree, Board im Speicher und ein Ticket „Login“ in Arbeit
struct Umgebung {
    projekt: TempProject,
    fixture: BoardFixture,
    worktree: String,
    starter: TestStarter,
    runtime: tokio::runtime::Runtime,
}

impl Umgebung {
    /// Legt Projekt, Worktree, Board und das Ticket ARC-0001 an
    fn new() -> Self {
        let projekt = TempProject::new();
        let project = HarnessProject::init(&projekt.path).expect("Projekt einrichten");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Laufzeit");
        let fixture = runtime.block_on(BoardFixture::create());
        runtime.block_on(async {
            fixture
                .boards
                .set_repo_path(fixture.board_id, Some(&project.root))
                .await
                .expect("Repo-Pfad");
        });
        let worktree = projekt
            .add_worktree("wt-ARC-0001")
            .display()
            .to_string()
            .replace('\\', "/");
        runtime.block_on(async {
            fixture
                .tickets
                .create(
                    fixture.board_id,
                    &NewTicketInput {
                        spec: Some("001-x".into()),
                        ..NewTicketInput::titled("Login")
                    },
                    &fixture.actor,
                    &WriteOptions::default(),
                )
                .await
                .expect("Ticket");
            fixture
                .tickets
                .update(
                    "ARC-0001",
                    &TicketPatch {
                        status: Some(TicketStatus::InProgress),
                        worktree: Some(Some(worktree.clone())),
                        ..TicketPatch::default()
                    },
                    &fixture.actor,
                    &WriteOptions::default(),
                )
                .await
                .expect("Ticket in Arbeit");
        });
        Self {
            projekt,
            fixture,
            worktree,
            starter: TestStarter::default(),
            runtime,
        }
    }

    /// Öffner auf die Board-DB dieser Umgebung
    fn offner(&self) -> SpeicherOffner {
        SpeicherOffner {
            db: self.fixture.db.clone(),
            clock: self.fixture.clock.clone(),
            paths: self.fixture.repo_paths.clone(),
            delay: None,
            leer: false,
        }
    }

    /// Öffner auf eine andere Datenbank mit derselben Uhr und denselben Repo-Pfaden
    fn offner_mit<D: BoardDatabase>(&self, db: Arc<D>) -> SpeicherOffner<D> {
        SpeicherOffner {
            db,
            clock: self.fixture.clock.clone(),
            paths: self.fixture.repo_paths.clone(),
            delay: None,
            leer: false,
        }
    }

    /// Führt einen Hook aus und liefert die letzte Ausgabe als `hookSpecificOutput`
    fn hook(&self, kind: HookKind, payload: Value, weitere: &[(&str, &str)]) -> Option<Value> {
        self.hook_mit(self.offner(), BUDGET, kind, payload, weitere)
            .0
    }

    /// Wie [`Umgebung::hook`], dazu eigener Öffner und eigenes Budget; liefert Ausgabe und Dauer
    fn hook_mit<D: BoardDatabase + 'static>(
        &self,
        offner: SpeicherOffner<D>,
        budget: Duration,
        kind: HookKind,
        payload: Value,
        weitere: &[(&str, &str)],
    ) -> (Option<Value>, Duration) {
        let mut ausgaben: Vec<String> = Vec::new();
        let begonnen = Instant::now();
        HookRunner::new(offner, self.starter.clone(), budget).run(
            kind,
            &payload,
            &umgebung(&self.projekt, weitere),
            &mut |text| ausgaben.push(text.to_string()),
        );
        let dauer = begonnen.elapsed();
        let letzte = ausgaben.last().map(|text| {
            serde_json::from_str::<Value>(text).expect("die Ausgabe ist JSON")["hookSpecificOutput"]
                .clone()
        });
        (letzte, dauer)
    }

    /// Text des eingespeisten Kontexts
    fn kontext(ausgabe: &Option<Value>) -> String {
        ausgabe
            .as_ref()
            .and_then(|out| out["additionalContext"].as_str())
            .unwrap_or_default()
            .to_string()
    }
}

#[test]
fn session_start_im_worktree_speist_kontext_ein_und_setzt_die_umgebungsvariablen() {
    let u = Umgebung::new();
    let env_datei = u.projekt.base().join("claude-env.sh");
    let ausgabe = u.hook(
        HookKind::SessionStart,
        json!({ "session_id": "s1", "cwd": u.worktree }),
        &[("CLAUDE_ENV_FILE", &env_datei.display().to_string())],
    );

    assert_eq!(ausgabe.as_ref().unwrap()["hookEventName"], "SessionStart");
    assert!(
        Umgebung::kontext(&ausgabe).contains("ARC-0001 „Login“ (Spec 001-x)"),
        "{ausgabe:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&env_datei).unwrap(),
        "export HARNESS_SESSION='s1'\nexport HARNESS_TICKET='ARC-0001'\n"
    );
}

#[test]
fn user_prompt_speist_einen_kommentar_genau_einmal_ein() {
    let u = Umgebung::new();
    u.runtime.block_on(async {
        u.fixture
            .comments
            .add("ARC-0001", "Bitte Tastatur beachten", &human(), false)
            .await
            .expect("Kommentar");
    });

    let erster = u.hook(
        HookKind::UserPrompt,
        json!({ "session_id": "s1", "cwd": u.worktree }),
        &[],
    );
    assert!(
        Umgebung::kontext(&erster).contains("Bitte Tastatur beachten"),
        "{erster:?}"
    );

    let zweiter = u.hook(
        HookKind::UserPrompt,
        json!({ "session_id": "s1", "cwd": u.worktree }),
        &[],
    );
    assert!(
        !Umgebung::kontext(&zweiter).contains("Bitte Tastatur beachten"),
        "{zweiter:?}"
    );
}

#[test]
fn beachtet_harness_ticket_und_verknupfte_sessions_ausserhalb_des_worktrees() {
    let u = Umgebung::new();
    u.runtime.block_on(async {
        u.fixture
            .tickets
            .create(
                u.fixture.board_id,
                &NewTicketInput::titled("Zweites"),
                &u.fixture.actor,
                &WriteOptions::default(),
            )
            .await
            .expect("zweites Ticket");
    });
    let repo = u.projekt.path.display().to_string();

    let mit_variable = u.hook(
        HookKind::SessionStart,
        json!({ "session_id": "s2", "cwd": repo }),
        &[("HARNESS_TICKET", "ARC-0002")],
    );
    assert!(
        Umgebung::kontext(&mit_variable).contains("ARC-0002 „Zweites“"),
        "{mit_variable:?}"
    );

    u.runtime.block_on(async {
        u.fixture
            .tickets
            .link_session("ARC-0002", "s3")
            .await
            .expect("Session verknüpfen");
    });
    let verknupft = u.hook(
        HookKind::SessionStart,
        json!({ "session_id": "s3", "cwd": repo }),
        &[],
    );
    assert!(
        Umgebung::kontext(&verknupft).contains("ARC-0002"),
        "{verknupft:?}"
    );
}

#[test]
fn weist_ohne_ticket_auf_claim_hin() {
    let u = Umgebung::new();
    let ausgabe = u.hook(
        HookKind::SessionStart,
        json!({ "session_id": "s4", "cwd": u.projekt.path.display().to_string() }),
        &[],
    );
    assert!(
        Umgebung::kontext(&ausgabe).contains("harness board claim"),
        "{ausgabe:?}"
    );
}

#[test]
fn bleibt_ohne_board_des_repos_und_ohne_erreichbare_db_still() {
    let u = Umgebung::new();
    u.runtime.block_on(async {
        u.fixture
            .boards
            .set_repo_path(u.fixture.board_id, None)
            .await
            .expect("Repo-Pfad entfernen");
    });

    let ohne_board = u.hook(
        HookKind::SessionStart,
        json!({ "session_id": "s5", "cwd": u.worktree }),
        &[],
    );
    assert_eq!(ohne_board, None);

    let ohne_db = u
        .hook_mit(
            SpeicherOffner {
                leer: true,
                ..u.offner()
            },
            BUDGET,
            HookKind::UserPrompt,
            json!({ "session_id": "s5", "cwd": u.worktree, "prompt": "Login fertig machen" }),
            &[],
        )
        .0;
    assert_eq!(ohne_db.as_ref().unwrap().get("additionalContext"), None);
    assert!(
        ohne_db.as_ref().unwrap()["sessionTitle"]
            .as_str()
            .is_some_and(|titel| titel.starts_with("SE-")),
        "{ohne_db:?}"
    );
}

#[test]
fn wartet_nicht_auf_eine_hangende_db() {
    let u = Umgebung::new();
    let env_datei = u.projekt.base().join("claude-env.sh");
    let hangend = || SpeicherOffner {
        delay: Some(Duration::from_secs(60)),
        ..u.offner()
    };
    let budget = Duration::from_millis(300);

    let (start, dauer) = u.hook_mit(
        hangend(),
        budget,
        HookKind::SessionStart,
        json!({ "session_id": "s6", "cwd": u.worktree }),
        &[("CLAUDE_ENV_FILE", &env_datei.display().to_string())],
    );
    assert!(dauer < Duration::from_secs(2), "{dauer:?}");
    assert_eq!(start, None);
    assert_eq!(
        std::fs::read_to_string(&env_datei).unwrap(),
        "export HARNESS_SESSION='s6'\n"
    );

    let (prompt, dauer) = u.hook_mit(
        hangend(),
        budget,
        HookKind::UserPrompt,
        json!({ "session_id": "s6", "cwd": u.worktree, "prompt": "Login fertig machen" }),
        &[],
    );
    assert!(dauer < Duration::from_secs(2), "{dauer:?}");
    assert_eq!(prompt.as_ref().unwrap().get("additionalContext"), None);
    assert!(
        prompt.as_ref().unwrap()["sessionTitle"]
            .as_str()
            .is_some_and(|titel| titel.starts_with("SE-")),
        "{prompt:?}"
    );
    assert_eq!(u.starter.probes.lock().unwrap().len(), 2);
}

#[test]
fn behalt_bei_einem_db_fehler_im_kontext_die_zuordnung_ohne_kontext_einzuspeisen() {
    let u = Umgebung::new();
    // `stop` macht nur die Zuordnung: so viele Transaktionen braucht sie
    let zahlend = Bremse::zahlend(u.fixture.db.clone());
    u.hook_mit(
        u.offner_mit(zahlend.clone()),
        BUDGET,
        HookKind::Stop,
        json!({ "session_id": "s8", "cwd": u.worktree }),
        &[],
    );
    let nach_der_zuordnung = zahlend.transaktionen();

    // Beim Kontext bricht die Verbindung weg; die Zuordnung steht da schon fest
    let scheiternd = Bremse::scheiternd(u.fixture.db.clone(), nach_der_zuordnung + 1);
    let env_datei = u.projekt.base().join("claude-env.sh");
    let ausgabe = u
        .hook_mit(
            u.offner_mit(scheiternd),
            BUDGET,
            HookKind::SessionStart,
            json!({ "session_id": "s8", "cwd": u.worktree }),
            &[("CLAUDE_ENV_FILE", &env_datei.display().to_string())],
        )
        .0;

    assert_eq!(
        ausgabe
            .as_ref()
            .and_then(|out| out.get("additionalContext")),
        None,
        "{ausgabe:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&env_datei).unwrap(),
        "export HARNESS_SESSION='s8'\nexport HARNESS_TICKET='ARC-0001'\n"
    );
}

#[test]
fn markiert_kommentare_erst_nach_der_ausgabe_als_gelesen() {
    let u = Umgebung::new();
    u.runtime.block_on(async {
        u.fixture
            .comments
            .add("ARC-0001", "Bitte Tastatur beachten", &human(), false)
            .await
            .expect("Kommentar");
    });
    let ungelesen = || {
        u.runtime.block_on(async {
            u.fixture
                .comments
                .unread_for_agent(&["ARC-0001".to_string()])
                .await
                .expect("ungelesene Kommentare")
                .len()
        })
    };

    let mut beim_ausgeben = None;
    HookRunner::new(u.offner(), u.starter.clone(), BUDGET).run(
        HookKind::UserPrompt,
        &json!({ "session_id": "s1", "cwd": u.worktree }),
        &umgebung(&u.projekt, &[]),
        &mut |_text| beim_ausgeben = Some(ungelesen()),
    );

    assert_eq!(beim_ausgeben, Some(1), "vor der Ausgabe noch ungelesen");
    assert_eq!(ungelesen(), 0, "nach der Ausgabe gelesen");
}

#[test]
fn stop_legt_keine_tickets_an_und_gibt_nichts_aus() {
    let u = Umgebung::new();
    let vorher = u.runtime.block_on(async {
        u.fixture
            .tickets
            .list(&Default::default())
            .await
            .expect("Tickets")
            .len()
    });

    let ausgabe = u.hook(
        HookKind::Stop,
        json!({ "session_id": "s1", "cwd": u.projekt.path.display().to_string() }),
        &[],
    );

    assert_eq!(ausgabe, None);
    let nachher = u.runtime.block_on(async {
        u.fixture
            .tickets
            .list(&Default::default())
            .await
            .expect("Tickets")
            .len()
    });
    assert_eq!(nachher, vorher);
}

// ---------------------------------------------------------------------------------------------
// Das gebaute Binary (portiert aus `packages/cli/tests/integration/hook.test.ts`)
// ---------------------------------------------------------------------------------------------

/// Ein Port, auf dem ein fremdes Programm hört: der Hook meldet dort nichts und startet
/// keinen eigenen Server
struct FremderPort {
    port: u16,
}

impl FremderPort {
    /// Nimmt Verbindungen an und schließt sie sofort wieder
    fn new() -> Self {
        let listener =
            TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).expect("freier Port");
        let port = listener.local_addr().expect("Adresse").port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                drop(stream);
            }
        });
        Self { port }
    }

    /// Die Umgebung eines Hook-Aufrufs: fremder Port und eine großzügige Frist
    fn env(&self) -> Vec<(String, String)> {
        vec![
            ("HARNESS_PORT".to_string(), self.port.to_string()),
            ("HARNESS_HOOK_DEADLINE_MS".to_string(), "60000".to_string()),
        ]
    }
}

/// Ruft `nexus hook <ereignis>` im Ordner auf
fn hook_binary(
    projekt: &TempProject,
    cwd: &Path,
    ereignis: &str,
    eingabe: &str,
    port: &FremderPort,
    weitere: &[(&str, &str)],
) -> common::temp_project::Lauf {
    let eigene = port.env();
    let mut env: Vec<(&str, &str)> = eigene
        .iter()
        .map(|(name, wert)| (name.as_str(), wert.as_str()))
        .collect();
    env.extend_from_slice(weitere);
    projekt.run_with_env(cwd, &["hook", ereignis], &env, Some(eingabe))
}

#[test]
fn das_binary_ordnet_im_worktree_zu_und_speist_den_kontext_ein() {
    let projekt = TempProject::new();
    let board = projekt.seed();
    let project = HarnessProject::init(&projekt.path).expect("Projekt einrichten");
    projekt.assign_repo(board, Path::new(&project.root));
    let worktree = projekt.add_worktree("wt-ARC-0001");
    let worktree_text = worktree.display().to_string().replace('\\', "/");
    projekt.am_board(async |rt| {
        let actor = rt.actor(Some("claude"));
        rt.services
            .tickets
            .create(
                board,
                &NewTicketInput::titled("Login bauen"),
                &actor,
                &WriteOptions::default(),
            )
            .await
            .expect("Ticket");
        rt.services
            .tickets
            .update(
                "ARC-0001",
                &TicketPatch {
                    status: Some(TicketStatus::InProgress),
                    worktree: Some(Some(worktree_text.clone())),
                    ..TicketPatch::default()
                },
                &actor,
                &WriteOptions::default(),
            )
            .await
            .expect("Ticket in Arbeit");
    });
    let port = FremderPort::new();
    let env_datei = projekt.base().join("claude-env.sh");

    let start = hook_binary(
        &projekt,
        &worktree,
        "session-start",
        &json!({ "session_id": "s1", "cwd": worktree_text }).to_string(),
        &port,
        &[("CLAUDE_ENV_FILE", &env_datei.display().to_string())],
    );

    assert_eq!(start.code, 0, "{}", start.stderr);
    assert_eq!(start.stderr, "");
    let ausgabe: Value = serde_json::from_str(&start.stdout).expect("JSON auf stdout");
    assert!(
        ausgabe["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .is_some_and(|text| text.contains("ARC-0001 „Login bauen“")),
        "{}",
        start.stdout
    );
    assert_eq!(
        std::fs::read_to_string(&env_datei).unwrap(),
        "export HARNESS_SESSION='s1'\nexport HARNESS_TICKET='ARC-0001'\n"
    );
}

#[test]
fn das_binary_bleibt_ohne_harness_init_wirkungslos() {
    let projekt = TempProject::new();
    let port = FremderPort::new();
    let env_datei = projekt.base().join("env.sh");

    let lauf = hook_binary(
        &projekt,
        &projekt.path,
        "session-start",
        &json!({ "session_id": "x", "cwd": projekt.path.display().to_string() }).to_string(),
        &port,
        &[("CLAUDE_ENV_FILE", &env_datei.display().to_string())],
    );

    assert_eq!(lauf.code, 0);
    assert_eq!(lauf.stdout, "");
    assert_eq!(lauf.stderr, "");
    assert!(!projekt.path.join(".git/harness").exists());
    assert!(!env_datei.exists());
}

#[test]
fn das_binary_ubersteht_ungultige_leere_und_fremde_eingaben_still() {
    let projekt = TempProject::new();
    HarnessProject::init(&projekt.path).expect("Projekt einrichten");
    let port = FremderPort::new();
    let ohne_sitzung = json!({ "cwd": projekt.path.display().to_string() }).to_string();
    let falscher_typ = json!({ "session_id": 5 }).to_string();

    for eingabe in ["{kaputt", "", "[]", "null", &ohne_sitzung, &falscher_typ] {
        let lauf = hook_binary(
            &projekt,
            &projekt.path,
            "session-start",
            eingabe,
            &port,
            &[],
        );
        assert_eq!(lauf.code, 0, "{eingabe}");
        assert_eq!(lauf.stdout, "", "{eingabe}");
        assert_eq!(lauf.stderr, "", "{eingabe}");
    }

    let unbekannt = hook_binary(
        &projekt,
        &projekt.path,
        "quatsch",
        &json!({ "session_id": "s", "cwd": projekt.path.display().to_string() }).to_string(),
        &port,
        &[],
    );
    assert_eq!(unbekannt.code, 0);
    assert_eq!(unbekannt.stdout, "");
}

#[test]
fn das_binary_endet_bei_hangender_db_still_innerhalb_der_frist() {
    let projekt = TempProject::new();
    HarnessProject::init(&projekt.path).expect("Projekt einrichten");
    // Eine Konfig auf eine Datenbank, die es nicht gibt: das Öffnen scheitert, der Hook bleibt
    // still und endet mit Code 0
    std::fs::write(
        projekt.home().join("board.yaml"),
        "provider: mssql\nserver: 203.0.113.1\ndatabase: nope\nuser: u\npassword: p\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: Test\n",
    )
    .expect("board.yaml");
    let port = FremderPort::new();
    let env_datei = projekt.base().join("claude-env.sh");
    let begonnen = Instant::now();

    let lauf = hook_binary(
        &projekt,
        &projekt.path,
        "session-start",
        &json!({ "session_id": "s9", "cwd": projekt.path.display().to_string() }).to_string(),
        &port,
        &[
            ("CLAUDE_ENV_FILE", &env_datei.display().to_string()),
            ("HARNESS_HOOK_DEADLINE_MS", "3000"),
        ],
    );

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert_eq!(lauf.stdout, "");
    assert!(begonnen.elapsed() < Duration::from_secs(5));
    assert_eq!(
        std::fs::read_to_string(&env_datei).unwrap(),
        "export HARNESS_SESSION='s9'\n"
    );
}
