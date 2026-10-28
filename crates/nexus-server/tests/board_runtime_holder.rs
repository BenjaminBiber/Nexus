//! Die eine Board-Laufzeit des Servers; portiert aus
//! `packages/server/tests/unit/BoardRuntimeHolder.test.ts`.
//!
//! Die Fälle „im Server nach dem Speichern der Board-Konfig“ gehören zu der Routengruppe, die
//! `PUT /api/board-config` beantwortet (P4-3); sie stehen am Ende als `#[ignore]` mit Hinweis.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use common::MemoryBoardBackend;
use nexus_core::{Error, HarnessError};
use nexus_db::sqlite::error_map;
use nexus_server::board_runtime::{
    factory, BoardBackend, BoardBackendFactory, BoardBackendFuture, BoardRuntimeHolder,
};
use tokio::sync::oneshot;

/// Fehler von SQLite, wie der Treiber ihn wirft
fn sqlite_error(extended: i32, message: &str) -> Error {
    let ffi = rusqlite::ffi::Error {
        code: rusqlite::ffi::ErrorCode::Unknown,
        extended_code: extended,
    };
    error_map::wrap(rusqlite::Error::SqliteFailure(ffi, Some(message.into())))
}

/// Verbindungsverlust des SQL-Server-Treibers (Socket zu)
fn connection_lost() -> Error {
    Error::unexpected(tiberius::error::Error::Io {
        kind: std::io::ErrorKind::ConnectionReset,
        message: "Socket geschlossen".into(),
    })
}

/// Eine Fabrik, die zählt und der Reihe nach die vorgegebenen Antworten liefert; danach immer
/// eine neue Laufzeit im Speicher
struct Recorded {
    calls: Arc<AtomicUsize>,
    answers: Arc<Mutex<Vec<Answer>>>,
}

/// Eine Antwort der Fabrik
enum Answer {
    Backend(Arc<MemoryBoardBackend>),
    Failure(Box<dyn Fn() -> Error + Send + Sync>),
}

impl Recorded {
    /// Fabrik mit den vorgegebenen Antworten samt Zähler der Aufrufe
    fn build(answers: Vec<Answer>) -> (BoardBackendFactory, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let recorded = Recorded {
            calls: calls.clone(),
            answers: Arc::new(Mutex::new(answers)),
        };
        let answers = recorded.answers;
        let counted = recorded.calls;
        let build = factory(move || -> BoardBackendFuture {
            counted.fetch_add(1, Ordering::SeqCst);
            let next = answers.lock().expect("Sperre der Antworten").pop();
            Box::pin(async move {
                match next {
                    Some(Answer::Backend(backend)) => Ok(backend as Arc<dyn BoardBackend>),
                    Some(Answer::Failure(error)) => Err(error()),
                    None => Ok(MemoryBoardBackend::new("neu") as Arc<dyn BoardBackend>),
                }
            })
        });
        (build, calls)
    }
}

/// Die Antworten in der Reihenfolge des Aufrufs (die Fabrik nimmt sie von hinten)
fn in_order(mut answers: Vec<Answer>) -> Vec<Answer> {
    answers.reverse();
    answers
}

/// Der Fehler eines Aufbaus (die Laufzeit selbst lässt sich nicht als Text ausgeben)
fn build_error(result: nexus_core::Result<Arc<dyn BoardBackend>>) -> Error {
    match result {
        Ok(_) => panic!("Aufbau sollte scheitern"),
        Err(error) => error,
    }
}

/// Die Laufzeit als Zeiger, um Gleichheit zu prüfen
fn address(backend: &Arc<dyn BoardBackend>) -> usize {
    Arc::as_ptr(backend) as *const () as usize
}

#[tokio::test]
async fn baut_die_laufzeit_erst_bei_bedarf_und_fur_gleichzeitige_anfragen_nur_einmal_auf() {
    let (build, calls) = Recorded::build(Vec::new());
    let holder = BoardRuntimeHolder::new(build);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let (a, b) = tokio::join!(holder.get(), holder.get());
    assert_eq!(address(&a.unwrap()), address(&b.unwrap()));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn merkt_sich_einen_fehlgeschlagenen_aufbau_nicht() {
    let (build, calls) = Recorded::build(in_order(vec![Answer::Failure(Box::new(|| {
        HarnessError::new("board-db-unreachable", "Server weg").into()
    }))]));
    let holder = BoardRuntimeHolder::new(build);
    assert_eq!(
        build_error(holder.get().await).code(),
        Some("board-db-unreachable")
    );
    assert!(holder.get().await.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn meldet_eine_kaputte_board_yaml_als_board_config_broken() {
    let (build, _calls) = Recorded::build(in_order(vec![Answer::Failure(Box::new(|| {
        HarnessError::new("board-config-invalid", "board.yaml ist kein gültiges YAML.").into()
    }))]));
    let holder = BoardRuntimeHolder::new(build);
    let error = build_error(holder.get().await);
    assert_eq!(error.code(), Some("board-config-broken"));
    assert!(error.to_string().contains("Einstellungen"), "{error}");
}

#[tokio::test]
async fn verwirft_die_laufzeit_nach_einem_verbindungsfehler_und_baut_beim_nachsten_mal_neu_auf() {
    let first = MemoryBoardBackend::new("erste");
    let (build, _calls) = Recorded::build(in_order(vec![Answer::Backend(first.clone())]));
    let holder = BoardRuntimeHolder::new(build);
    let error = holder
        .run(|_backend| async { Err::<(), _>(connection_lost()) })
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("board-db-unreachable"));
    assert!(first.closed());
    let neu = holder.get().await.unwrap();
    assert_ne!(
        address(&neu),
        address(&(first.clone() as Arc<dyn BoardBackend>))
    );
}

#[tokio::test]
async fn lasst_fachliche_fehler_durch_ohne_neu_aufzubauen() {
    let (build, calls) = Recorded::build(Vec::new());
    let holder = BoardRuntimeHolder::new(build);
    let error = holder
        .run(|_backend| async { Err::<(), _>(HarnessError::new("title-empty", "leer").into()) })
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("title-empty"));
    holder.get().await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn schliesst_eine_frisch_gebaute_laufzeit_nicht_wenn_ein_spater_fehler_der_alten_eintrifft() {
    let a = MemoryBoardBackend::new("a");
    let b = MemoryBoardBackend::new("b");
    let (build, calls) = Recorded::build(in_order(vec![
        Answer::Backend(a.clone()),
        Answer::Backend(b.clone()),
    ]));
    let holder = Arc::new(BoardRuntimeHolder::new(build));
    let (release, gate) = oneshot::channel::<()>();

    let first = holder
        .run(|_backend| async { Err::<(), _>(connection_lost()) })
        .await;
    assert_eq!(first.unwrap_err().code(), Some("board-db-unreachable"));
    assert!(a.closed());

    // Die neue Laufzeit steht; der Fehler der alten darf sie nicht mehr treffen
    let zweite = holder.clone();
    let second = tokio::spawn(async move {
        zweite
            .run(|_backend| async move {
                let _ = gate.await;
                Err::<(), _>(connection_lost())
            })
            .await
    });
    assert_eq!(
        address(&holder.get().await.unwrap()),
        address(&(b.clone() as Arc<dyn BoardBackend>))
    );
    release.send(()).unwrap();
    let error = second.await.unwrap().unwrap_err();
    assert_eq!(error.code(), Some("board-db-unreachable"));
    // Die zweite Laufzeit wurde selbst verworfen – sie war die aktuelle, als der Fehler kam
    assert!(b.closed());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn behandelt_einen_abfrage_timeout_nicht_als_verbindungsverlust() {
    let first = MemoryBoardBackend::new("erste");
    let (build, calls) = Recorded::build(in_order(vec![Answer::Backend(first.clone())]));
    let holder = BoardRuntimeHolder::new(build);
    // Ein Fehler des Servers selbst (mit Fehlernummer) ist kein Verbindungsverlust
    let timeout = Error::unexpected(tiberius::error::Error::Conversion(
        "Timeout: Request failed to complete".into(),
    ));
    let error = holder
        .run(|_backend| async { Err::<(), _>(timeout) })
        .await
        .unwrap_err();
    assert_eq!(error.code(), None);
    assert!(error.to_string().contains("Timeout"), "{error}");
    assert!(!first.closed());
    holder.get().await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn schliesst_bei_reset_und_baut_danach_neu_auf() {
    let first = MemoryBoardBackend::new("erste");
    let (build, calls) = Recorded::build(in_order(vec![Answer::Backend(first.clone())]));
    let holder = BoardRuntimeHolder::new(build);
    holder.get().await.unwrap();
    holder.reset().await;
    assert!(first.closed());
    let neu = holder.get().await.unwrap();
    assert_ne!(
        address(&neu),
        address(&(first.clone() as Arc<dyn BoardBackend>))
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn stuft_sqlite_datei_und_zugriffsfehler_als_verbindungsfehler_ein_busy_und_eindeutigkeit_nicht() {
    for code in [14, 8, 26, 3, 1038] {
        assert!(
            BoardRuntimeHolder::is_connection_error(&sqlite_error(code, "unable to open")),
            "{code}"
        );
    }
    // Eine geschlossene Datenbank zählt wie eine verlorene Verbindung
    assert!(BoardRuntimeHolder::is_connection_error(
        &HarnessError::new("board-db-closed", "schon geschlossen").into()
    ));
    assert!(BoardRuntimeHolder::is_connection_error(&connection_lost()));

    for code in [5, 517, 6, 2067, 10, 13] {
        assert!(
            !BoardRuntimeHolder::is_connection_error(&sqlite_error(code, "x")),
            "{code}"
        );
    }
    assert!(!BoardRuntimeHolder::is_connection_error(
        &HarnessError::new("board-db-storage", "voll").into()
    ));
    assert!(!BoardRuntimeHolder::is_connection_error(
        &Error::unexpected_message("Programmfehler")
    ));
}

#[tokio::test]
async fn verwirft_die_laufzeit_nach_einem_sqlite_dateifehler() {
    let first = MemoryBoardBackend::new("erste");
    let (build, calls) = Recorded::build(in_order(vec![Answer::Backend(first.clone())]));
    let holder = BoardRuntimeHolder::new(build);
    let error = holder
        .run(|_backend| async { Err::<(), _>(sqlite_error(14, "unable to open database file")) })
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("board-db-unreachable"));
    assert!(first.closed());
    holder.get().await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn meldet_sqlite_busy_als_conflict_und_baut_nicht_neu_auf() {
    let first = MemoryBoardBackend::new("erste");
    let (build, calls) = Recorded::build(in_order(vec![Answer::Backend(first.clone())]));
    let holder = BoardRuntimeHolder::new(build);
    let error = holder
        .run(|_backend| async { Err::<(), _>(sqlite_error(5, "database is locked")) })
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("conflict"));
    assert!(!first.closed());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn meldet_eine_nicht_offenbare_board_datei_beim_aufbau_als_board_db_unreachable() {
    let (build, calls) = Recorded::build(in_order(vec![Answer::Failure(Box::new(|| {
        sqlite_error(14, "unable to open database file")
    }))]));
    let holder = BoardRuntimeHolder::new(build);
    let error = build_error(holder.get().await);
    assert_eq!(error.code(), Some("board-db-unreachable"));
    assert!(error.to_string().contains("Board-Datei"), "{error}");
    // Der Fehlschlag wird nicht gemerkt
    assert!(holder.get().await.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn meldet_einen_nicht_anlegbaren_ordner_beim_aufbau_als_board_db_unreachable() {
    // Die Datenzugriffsschicht übersetzt das schon beim Öffnen (SqliteBoardDatabase::connect)
    let (build, _calls) = Recorded::build(in_order(vec![Answer::Failure(Box::new(|| {
        HarnessError::new(
            "board-db-unreachable",
            "Board-Datei /x/board.db lässt sich nicht öffnen: Permission denied",
        )
        .into()
    }))]));
    let holder = BoardRuntimeHolder::new(build);
    let error = build_error(holder.get().await);
    assert_eq!(error.code(), Some("board-db-unreachable"));
}

#[tokio::test]
async fn lasst_einen_fremden_fehler_beim_offnen_unverandert_durch() {
    let (build, _calls) = Recorded::build(in_order(vec![Answer::Failure(Box::new(|| {
        Error::unexpected_message("EACCES: permission denied, open 'X:/x'")
    }))]));
    let holder = BoardRuntimeHolder::new(build);
    let error = build_error(holder.get().await);
    assert_eq!(error.code(), None);
    assert_eq!(error.to_string(), "EACCES: permission denied, open 'X:/x'");
}

#[tokio::test]
async fn reicht_board_db_storage_durch_ohne_neu_aufzubauen() {
    let first = MemoryBoardBackend::new("erste");
    let (build, calls) = Recorded::build(in_order(vec![Answer::Backend(first.clone())]));
    let holder = BoardRuntimeHolder::new(build);
    let error = holder
        .run(|_backend| async { Err::<(), _>(HarnessError::new("board-db-storage", "x").into()) })
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("board-db-storage"));
    assert!(!first.closed());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn gibt_den_handelnden_der_laufzeit_weiter() {
    let holder = BoardRuntimeHolder::new(common::memory_board());
    let backend = holder.get().await.unwrap();
    assert_eq!(backend.actor(Some("claude")).name, "claude");
    assert_eq!(backend.actor(None).name, "Vertrag");
}

/// Eine gültige Konfig, wie die Oberfläche sie an `PUT /api/board-config` schickt
const BOARD_CONFIG_INPUT: &str = r#"{"server":"127.0.0.1","port":1,"database":"AiHarness","user":"harness","password":"geheim","encrypt":false,"trustServerCertificate":true,"userId":"11111111-1111-1111-1111-111111111111","userName":"Florian"}"#;

#[tokio::test]
async fn baut_im_server_nach_dem_speichern_der_board_konfig_neu_auf() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let server = common::TestServer::with(common::TestServerOptions {
        board: Some(factory(move || -> BoardBackendFuture {
            counted.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(MemoryBoardBackend::new("neu") as Arc<dyn BoardBackend>) })
        })),
        ..Default::default()
    });

    server.state.board.get().await.unwrap();
    assert_eq!(
        server
            .put("/api/board-config", BOARD_CONFIG_INPUT)
            .await
            .status,
        200
    );
    server.state.board.get().await.unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn schliesst_im_server_die_alte_laufzeit_nach_dem_speichern_der_board_konfig() {
    let backend = MemoryBoardBackend::new("alt");
    let geteilt = backend.clone();
    let server = common::TestServer::with(common::TestServerOptions {
        board: Some(factory(move || -> BoardBackendFuture {
            let backend = geteilt.clone();
            Box::pin(async move { Ok(backend as Arc<dyn BoardBackend>) })
        })),
        ..Default::default()
    });

    server.state.board.get().await.unwrap();
    assert!(!backend.closed());
    assert_eq!(
        server
            .put("/api/board-config", BOARD_CONFIG_INPUT)
            .await
            .status,
        200
    );

    assert!(backend.closed());
}
