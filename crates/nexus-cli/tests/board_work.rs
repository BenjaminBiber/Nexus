//! Die Arbeitsbefehle des Boards von außen: `board start|done` gegen eine SQLite-Board-Datenbank
//! in einem eigenen `HARNESS_HOME` und `board merge|accept` gegen einen Fake-Server.
//! Portiert aus `packages/cli/tests/unit/BoardWorkCommands.test.ts` und
//! `packages/cli/tests/integration/board-work.test.ts`.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use common::temp_project::TempProject;

/// Ein eingerichtetes Projekt, dessen Board dem Repo zugeordnet ist
fn projekt_mit_board() -> (TempProject, i32) {
    let projekt = TempProject::new();
    assert_eq!(projekt.run(&["init"]).code, 0);
    let board = projekt.seed();
    projekt.assign_repo(board, &projekt.path.clone());
    (projekt, board)
}

#[test]
fn meldet_ohne_board_yaml_die_fehlende_konfig() {
    let projekt = TempProject::new();
    let home = tempfile::Builder::new()
        .prefix("harness-home-")
        .tempdir()
        .expect("Temp-Ordner");

    let lauf = projekt.run_in_home(home.path(), &["board", "start", "ARC-0001"]);

    assert_eq!(lauf.code, 1, "{}", lauf.stdout);
    assert!(lauf.stderr.contains("board.yaml"), "{}", lauf.stderr);
}

#[test]
fn startet_im_worktree_verknupft_die_session_und_schliesst_mit_abnahme_ab() {
    // Die Schritte des TypeScript-Falls (`board step`) kommen in einem eigenen Ticket
    let (projekt, _) = projekt_mit_board();
    projekt.run(&["board", "create", "Login bauen"]);

    let gestartet = projekt.run_in_session(
        "s-42",
        &["--json", "board", "start", "ARC-0001", "--by", "claude"],
    );
    assert_eq!(gestartet.code, 0, "{}", gestartet.stderr);
    let gestartet = gestartet.json();
    assert_eq!(gestartet["id"], "ARC-0001");
    assert_eq!(gestartet["branch"], "ticket/ARC-0001-login-bauen");
    assert_eq!(gestartet["joined"], false);
    let pfad = gestartet["path"].as_str().expect("Pfad");
    assert!(std::path::Path::new(pfad).exists(), "{pfad}");

    let ticket = projekt.json(&["board", "show", "ARC-0001"]);
    assert_eq!(ticket["status"], "in_progress");
    assert_eq!(ticket["branch"], "ticket/ARC-0001-login-bauen");
    assert_eq!(ticket["sessions"][0], "s-42");
    assert!(projekt
        .run(&["board", "show", "ARC-0001"])
        .stdout
        .contains("Branch: ticket/ARC-0001-login-bauen"));

    let ohne_abnahme = projekt.run(&["board", "done", "ARC-0001"]);
    assert_eq!(ohne_abnahme.code, 1, "{}", ohne_abnahme.stdout);
    assert!(
        ohne_abnahme.stderr.contains("Abnahme"),
        "{}",
        ohne_abnahme.stderr
    );

    let erledigt = projekt.json(&["board", "done", "ARC-0001", "--acceptance", "Test grün"]);
    assert_eq!(erledigt["id"], "ARC-0001");
    assert_eq!(erledigt["readyToMerge"], true);
    assert_eq!(
        projekt.json(&["board", "show", "ARC-0001"])["status"],
        "done"
    );
}

#[test]
fn verweigert_den_start_bei_offener_voraussetzung_ohne_force() {
    let (projekt, _) = projekt_mit_board();
    projekt.run(&["board", "create", "Basis"]);
    projekt.run(&["board", "create", "Folge", "--depends-on", "ARC-0001"]);

    let blockiert = projekt.run(&["board", "start", "ARC-0002"]);
    assert_eq!(blockiert.code, 1, "{}", blockiert.stdout);
    assert!(
        blockiert.stderr.contains("ARC-0001"),
        "{}",
        blockiert.stderr
    );

    let erzwungen = projekt.json(&["board", "start", "ARC-0002", "--force"]);
    assert_eq!(erzwungen["joined"], false);
}

#[test]
fn lehnt_start_und_done_fur_tickets_eines_anderen_boards_ab_und_nennt_beide_boards() {
    let (projekt, board) = projekt_mit_board();
    let fremd = projekt.create_board("Fremd");
    projekt.run(&[
        "board",
        "create",
        "Fremdes Ticket",
        "--board",
        &fremd.to_string(),
        "--acceptance",
        "ok",
    ]);

    let start = projekt.run(&["board", "start", "ARC-0001"]);
    assert_eq!(start.code, 1, "{}", start.stdout);
    assert!(
        start.stderr.contains(&format!("Board {fremd}")),
        "{}",
        start.stderr
    );
    assert!(
        start.stderr.contains(&format!("Board {board}")),
        "{}",
        start.stderr
    );
    assert!(
        start.stderr.contains("harness board boards repo"),
        "{}",
        start.stderr
    );
    assert_eq!(projekt.git(&["branch", "--list", "ticket/*"]), "");

    let done = projekt.run(&["board", "done", "ARC-0001"]);
    assert_eq!(done.code, 1, "{}", done.stdout);
    assert!(
        done.stderr.contains("gehört aber zu Board"),
        "{}",
        done.stderr
    );
}

#[test]
fn verlangt_fur_start_ein_dem_repo_zugeordnetes_board() {
    let projekt = TempProject::new();
    assert_eq!(projekt.run(&["init"]).code, 0);
    projekt.seed();
    projekt.run(&["board", "create", "Login bauen"]);

    let start = projekt.run(&["board", "start", "ARC-0001"]);

    assert_eq!(start.code, 1, "{}", start.stdout);
    assert!(
        start.stderr.contains("harness board boards repo"),
        "{}",
        start.stderr
    );
}

#[test]
fn meldet_einen_neu_angelegten_zielbranch_als_eigene_zeile() {
    let (projekt, _) = projekt_mit_board();
    projekt.run(&["board", "create", "A", "--target-branch", "release/1"]);

    let lauf = projekt.run(&["board", "start", "ARC-0001"]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    let zeilen: Vec<&str> = lauf.stdout.lines().collect();
    assert!(
        zeilen[0].contains("ARC-0001 läuft jetzt auf Branch ticket/ARC-0001-a"),
        "{}",
        lauf.stdout
    );
    assert!(
        zeilen.iter().any(
            |zeile| zeile.starts_with("Zielbranch release/1 neu angelegt von ")
                && zeile.ends_with('.')
        ),
        "{}",
        lauf.stdout
    );
}

#[test]
fn gibt_ohne_zielbranch_keine_zielbranch_zeile_aus() {
    let (projekt, _) = projekt_mit_board();
    projekt.run(&["board", "create", "A"]);

    let lauf = projekt.run(&["board", "start", "ARC-0001"]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert!(
        !lauf
            .stdout
            .lines()
            .any(|zeile| zeile.starts_with("Zielbranch")),
        "{}",
        lauf.stdout
    );
}

/// Eine aufgezeichnete Anfrage am Fake-Server
#[derive(Debug, Clone)]
struct Aufruf {
    method: String,
    url: String,
    body: String,
    harness: Option<String>,
}

/// Fake-Server, der auf `/api/health` antwortet und jede Dispatcher-Anfrage mitschreibt
struct FakeServer {
    port: u16,
    aufrufe: Receiver<Aufruf>,
}

impl FakeServer {
    /// Startet den Server auf einem freien Port
    fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("Port");
        let port = listener.local_addr().expect("Adresse").port();
        let (sender, aufrufe) = channel();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                if Self::handle(stream, &sender).is_err() {
                    break;
                }
            }
        });
        Self { port, aufrufe }
    }

    /// Alle bisher aufgezeichneten Anfragen (ohne `/api/health`)
    fn aufrufe(&self) -> Vec<Aufruf> {
        self.aufrufe.try_iter().collect()
    }

    /// Liest eine Anfrage, zeichnet sie auf und antwortet mit einer Dispatcher-Übersicht
    fn handle(mut stream: TcpStream, sender: &Sender<Aufruf>) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut start = String::new();
        reader.read_line(&mut start)?;
        let mut teile = start.split_whitespace();
        let method = teile.next().unwrap_or_default().to_string();
        let url = teile.next().unwrap_or_default().to_string();

        let mut harness = None;
        let mut laenge = 0usize;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 || line == "\r\n" {
                break;
            }
            let lower = line.to_ascii_lowercase();
            if let Some(value) = lower.strip_prefix("x-harness:") {
                harness = Some(value.trim().to_string());
            }
            if let Some(value) = lower.strip_prefix("content-length:") {
                laenge = value.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0u8; laenge];
        reader.read_exact(&mut body)?;
        let body = String::from_utf8_lossy(&body).to_string();

        let rumpf = if url == "/api/health" {
            "{\"name\":\"ai-harness\"}".to_string()
        } else {
            let _ = sender.send(Aufruf {
                method,
                url: url.clone(),
                body,
                harness,
            });
            "{\"state\":\"off\",\"message\":\"Dispatcher ist ausgeschaltet\",\"result\":{\"outcome\":\"merged\",\"text\":\"ARC-0009 gemergt in main (abc1234).\"}}".to_string()
        };
        let antwort = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{rumpf}",
            rumpf.len()
        );
        stream.write_all(antwort.as_bytes())?;
        stream.flush()
    }
}

#[test]
fn merge_ruft_die_merge_route_mit_pflichtkopf_auf_und_gibt_das_ergebnis_aus() {
    let projekt = TempProject::new();
    let server = FakeServer::start();

    let lauf = projekt.run(&[
        "board",
        "--port",
        &server.port.to_string(),
        "merge",
        "ARC-0009",
    ]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert_eq!(lauf.stdout.trim(), "ARC-0009 gemergt in main (abc1234).");
    let aufrufe = server.aufrufe();
    assert_eq!(aufrufe.len(), 1, "{aufrufe:?}");
    assert_eq!(aufrufe[0].method, "POST");
    assert_eq!(aufrufe[0].url, "/api/dispatch/merge/ARC-0009");
    assert_eq!(aufrufe[0].harness.as_deref(), Some("1"));
    assert_eq!(aufrufe[0].body, "{}");
}

#[test]
fn accept_schickt_den_menschen_im_koerper_mit() {
    let projekt = TempProject::new();
    let server = FakeServer::start();

    let lauf = projekt.run(&[
        "board",
        "--port",
        &server.port.to_string(),
        "accept",
        "ARC-0009",
        "--by",
        "Florian",
    ]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    let aufrufe = server.aufrufe();
    assert_eq!(aufrufe.len(), 1, "{aufrufe:?}");
    assert_eq!(aufrufe[0].url, "/api/dispatch/accept/ARC-0009");
    assert_eq!(aufrufe[0].harness.as_deref(), Some("1"));
    assert_eq!(aufrufe[0].body, "{\"by\":\"Florian\"}");
}

#[test]
fn accept_aus_einer_session_wird_abgelehnt() {
    let projekt = TempProject::new();
    let server = FakeServer::start();

    let lauf = projekt.run_in_session(
        "s-1",
        &[
            "board",
            "--port",
            &server.port.to_string(),
            "accept",
            "ARC-0009",
            "--by",
            "Florian",
        ],
    );

    assert_eq!(lauf.code, 1, "{}", lauf.stdout);
    assert!(lauf.stderr.contains("[human-only]"), "{}", lauf.stderr);
    assert!(server.aufrufe().is_empty());
}
