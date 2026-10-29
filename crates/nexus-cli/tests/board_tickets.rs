//! Die Ticketbefehle des Boards von außen: `board list|show|create|set|move|delete|claim|
//! release|comment` gegen eine SQLite-Board-Datenbank in einem eigenen `HARNESS_HOME`.
//! Portiert aus den passenden Fällen von `packages/cli/tests/integration/board-db.test.ts`.

mod common;

use common::temp_project::TempProject;

/// Home ohne `board.yaml`: ein Ordner, den kein Testprojekt eingerichtet hat
fn home_ohne_konfig() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("harness-home-")
        .tempdir()
        .expect("Temp-Ordner")
}

#[test]
fn meldet_die_fehlende_board_yaml_mit_exit_1() {
    let projekt = TempProject::new();
    let home = home_ohne_konfig();

    let lauf = projekt.run_in_home(home.path(), &["board", "list"]);

    assert_eq!(lauf.code, 1, "{}", lauf.stdout);
    assert!(lauf.stderr.contains("board.yaml fehlt"), "{}", lauf.stderr);
}

#[test]
fn nennt_den_fehlercode_im_klartext_in_klammern_und_mit_json_als_objekt_auf_stdout() {
    let projekt = TempProject::new();
    let home = home_ohne_konfig();

    let text = projekt.run_in_home(home.path(), &["board", "list"]);
    assert_eq!(text.code, 1, "{}", text.stdout);
    assert!(
        text.stderr.trim().ends_with("[board-config-missing]"),
        "{}",
        text.stderr
    );
    assert!(text.stderr.contains("fehlt – "), "{}", text.stderr);

    let json = projekt.run_in_home(home.path(), &["--json", "board", "list"]);
    assert_eq!(json.code, 1, "{}", json.stderr);
    let wert = json.json();
    assert_eq!(wert["error"]["code"], "board-config-missing");
    let meldung = wert["error"]["message"].as_str().expect("Meldung");
    assert!(meldung.contains("board.yaml fehlt"), "{meldung}");
    assert!(!meldung.contains("[board-config-missing]"), "{meldung}");
}

#[test]
fn weist_ein_unbekanntes_wort_ohne_datenbankzugriff_ab() {
    let projekt = TempProject::new();
    let home = home_ohne_konfig();

    let lauf = projekt.run_in_home(home.path(), &["board", "lst"]);

    assert_eq!(lauf.code, 1, "{}", lauf.stdout);
    assert!(lauf.stderr.contains("lst"), "{}", lauf.stderr);
    assert!(!lauf.stderr.contains("board.yaml"), "{}", lauf.stderr);
}

#[test]
fn verlangt_ein_aktives_board_fur_ticketbefehle() {
    // Konfig ohne aktives Board (kein `seed`), Projekt ohne `init`: keine Zuordnung greift
    let projekt = TempProject::new();

    let lauf = projekt.run(&["board", "list"]);

    assert_eq!(lauf.code, 1, "{}", lauf.stdout);
    assert!(
        lauf.stderr.contains("Kein aktives Board"),
        "{}",
        lauf.stderr
    );
}

#[test]
fn bildet_den_ticketablauf_des_alten_boards_ab() {
    let projekt = TempProject::new();
    projekt.seed();

    assert!(projekt
        .run(&[
            "board",
            "create",
            "Login bauen",
            "--label",
            "ui",
            "--by",
            "claude"
        ])
        .stdout
        .contains("ARC-0001"));
    projekt.run(&[
        "board",
        "create",
        "Token prüfen",
        "--depends-on",
        "ARC-0001",
        "--priority",
        "high",
    ]);

    // Gleicher Titel in anderer Schreibweise: abgewiesen, Meldung wortgleich wie in TypeScript
    let doppelt = projekt.run(&["board", "create", "login BAUEN"]);
    assert_eq!(doppelt.code, 1, "{}", doppelt.stdout);
    assert_eq!(
        doppelt.stderr.trim(),
        "Ein offenes Ticket mit diesem Titel gibt es schon: ARC-0001. Mit --force trotzdem anlegen. [ticket-duplicate]"
    );

    assert!(projekt
        .run(&["board", "list", "--board", "99"])
        .stderr
        .contains("Board 99 gibt es nicht. [board-not-found]"));
    assert!(projekt
        .run(&["board", "list"])
        .stdout
        .contains("Wartet (1)"));
    assert!(projekt
        .run(&["board", "list", "--status", "wartet", "--md"])
        .stdout
        .contains("Wartet auf: ARC-0001"));

    // Erledigt braucht eine Abnahme
    let ohne_abnahme = projekt.run(&["board", "move", "ARC-0001", "erledigt"]);
    assert_eq!(ohne_abnahme.code, 1, "{}", ohne_abnahme.stdout);
    assert!(
        ohne_abnahme.stderr.contains("Abnahme"),
        "{}",
        ohne_abnahme.stderr
    );
    assert_eq!(
        projekt
            .run(&[
                "board",
                "move",
                "ARC-0001",
                "erledigt",
                "--acceptance",
                "Test grün",
                "--by",
                "claude"
            ])
            .code,
        0
    );

    // Die Voraussetzung ist erfüllt: ARC-0002 wartet nicht mehr
    let detail = projekt.json(&["board", "show", "ARC-0002"]);
    assert_eq!(detail["dependsOpen"], serde_json::json!([]));
    assert_eq!(detail["waiting"], serde_json::json!(false));

    assert_eq!(
        projekt
            .run(&["board", "claim", "ARC-0002", "--agent", "claude"])
            .code,
        0
    );
    // Schon zugewiesen: auch ein anderer Agent kommt nicht dran
    assert_eq!(
        projekt
            .run(&["board", "claim", "ARC-0002", "--agent", "andere"])
            .code,
        1
    );
    assert_eq!(
        projekt
            .run(&[
                "board",
                "comment",
                "ARC-0002",
                "Zwischenstand",
                "--author",
                "claude"
            ])
            .code,
        0
    );
    assert!(projekt
        .run(&["board", "show", "ARC-0002"])
        .stdout
        .contains("claude: Zwischenstand"));
    assert_eq!(
        projekt
            .run(&["board", "show", "ARC-0001", "--field", "acceptance"])
            .stdout
            .trim(),
        "Test grün"
    );

    // `--rev` schreibt nur, solange das Ticket noch diese Version hat
    let rev = projekt.json(&["board", "show", "ARC-0002"])["rev"]
        .as_i64()
        .expect("rev")
        .to_string();
    assert_eq!(
        projekt
            .run(&["board", "set", "ARC-0002", "--title", "Neu", "--rev", &rev])
            .code,
        0
    );
    assert_eq!(
        projekt
            .run(&["board", "set", "ARC-0002", "--title", "Zu spät", "--rev", &rev])
            .code,
        1
    );
    assert_eq!(
        projekt
            .run(&["board", "release", "ARC-0002", "--status", "planned"])
            .code,
        0
    );
    // „nicht mehr nötig“ ist für ein Ticket mit erfüllten Voraussetzungen gesperrt
    assert_eq!(
        projekt
            .run(&["board", "move", "ARC-0002", "nicht mehr nötig"])
            .code,
        1
    );
}

#[test]
fn loscht_ein_ticket_endgultig() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&["board", "create", "Weg damit"]);

    let gelöscht = projekt.run(&["board", "delete", "ARC-0001", "--by", "claude"]);

    assert_eq!(gelöscht.code, 0, "{}", gelöscht.stderr);
    assert!(
        gelöscht.stdout.contains("ARC-0001 gelöscht."),
        "{}",
        gelöscht.stdout
    );
    assert!(projekt
        .run(&["board", "list"])
        .stdout
        .contains("(keine Tickets gefunden)"));
    assert_eq!(projekt.run(&["board", "show", "ARC-0001"]).code, 1);
}

#[test]
fn nimmt_im_eingerichteten_repo_das_board_des_repos_und_board_hat_vorrang() {
    let projekt = TempProject::new();
    let aktiv = projekt.seed();
    let repo_board = projekt.create_board("Repo");
    projekt.assign_repo(repo_board, &projekt.path);

    // Noch nicht eingerichtet: das aktive Board, auch wenn ein Board dem Ordner zugeordnet ist
    assert_eq!(
        projekt.json(&["board", "create", "Vor init"])["boardId"],
        serde_json::json!(aktiv)
    );
    assert_eq!(projekt.run(&["init"]).code, 0);
    assert_eq!(
        projekt.json(&["board", "create", "Im Repo"])["boardId"],
        serde_json::json!(repo_board)
    );
    assert_eq!(
        projekt.json(&[
            "board",
            "create",
            "Ausdrücklich",
            "--board",
            &aktiv.to_string()
        ])["boardId"],
        serde_json::json!(aktiv)
    );

    let im_repo = projekt.run(&["board", "list", "--md"]).stdout;
    assert!(im_repo.contains("Im Repo"), "{im_repo}");
    assert!(!im_repo.contains("Vor init"), "{im_repo}");

    // Außerhalb eines Repos (das Home ist kein Git-Ordner): weiterhin das aktive Board
    let home = projekt.home();
    let draussen = projekt.run_in(&home, &["board", "list", "--md"]).stdout;
    assert!(draussen.contains("Vor init"), "{draussen}");

    // Mit „all“ stehen beide Boards unter einer Überschrift
    let alle = projekt
        .run(&["board", "list", "--board", "all", "--md"])
        .stdout;
    assert!(alle.starts_with("## Alle Boards"), "{alle}");
    assert!(
        alle.contains("Vor init") && alle.contains("Im Repo"),
        "{alle}"
    );
}

#[test]
fn zeigt_texte_schritte_und_kommentare_kompakt_und_mit_allen_optionen() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&[
        "board",
        "create",
        "Export bauen",
        "--desc",
        "Kontext",
        "--step",
        "Test schreiben",
        "--step",
        "Umsetzen",
        "--acceptance",
        "Test deckt den Fall ab",
    ]);
    projekt.run(&["board", "set", "ARC-0001", "--plan", "## Ziel"]);

    let kompakt = projekt.run(&["board", "show", "ARC-0001"]);

    assert_eq!(kompakt.code, 0, "{}", kompakt.stderr);
    for zeile in [
        "ARC-0001  Export bauen",
        "Abnahme: Test deckt den Fall ab",
        "## Beschreibung",
        "## Plan",
        "## Schritte (0/2)",
        "  [ ] 1. Test schreiben",
        "  [ ] 2. Umsetzen",
        "## Kommentare",
        "(keine)",
    ] {
        assert!(
            kompakt.stdout.contains(zeile),
            "{zeile}\n{}",
            kompakt.stdout
        );
    }
    // Ohne `--history` nur der Hinweis
    assert!(!kompakt.stdout.contains("## Verlauf"), "{}", kompakt.stdout);
    assert!(
        kompakt.stdout.contains("--history zeigt sie"),
        "{}",
        kompakt.stdout
    );

    let voll = projekt.run(&["board", "show", "ARC-0001", "--history", "--all-comments"]);
    assert!(voll.stdout.contains("## Verlauf"), "{}", voll.stdout);

    // Ein unbekanntes Feld wird abgewiesen
    let feld = projekt.run(&["board", "show", "ARC-0001", "--field", "quatsch"]);
    assert_eq!(feld.code, 1, "{}", feld.stdout);
    assert!(feld.stderr.contains("[field-invalid]"), "{}", feld.stderr);
    assert_eq!(
        projekt
            .run(&["board", "show", "ARC-0001", "--field", "plan"])
            .stdout
            .trim(),
        "## Ziel"
    );
}

#[test]
fn liest_texte_aus_datei_und_von_der_standardeingabe() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&["board", "create", "Mit Dateien"]);
    let datei = projekt.base().join("plan.md");
    std::fs::write(&datei, "## Aus der Datei\n").expect("plan.md");

    assert_eq!(
        projekt
            .run(&[
                "board",
                "set",
                "ARC-0001",
                "--plan-file",
                datei.to_str().expect("Pfad")
            ])
            .code,
        0
    );
    assert_eq!(
        projekt
            .run(&["board", "show", "ARC-0001", "--field", "plan"])
            .stdout
            .trim(),
        "## Aus der Datei"
    );

    // „-“ liest die Standardeingabe
    let lauf = projekt.run_with_input(
        &["board", "comment", "ARC-0001", "--body-file", "-"],
        "Aus stdin\n",
    );
    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert!(projekt
        .run(&["board", "show", "ARC-0001"])
        .stdout
        .contains("Aus stdin"));

    // Ohne Text und ohne Datei fehlt der Kommentar
    let ohne = projekt.run(&["board", "comment", "ARC-0001"]);
    assert_eq!(ohne.code, 1, "{}", ohne.stdout);
    assert!(
        ohne.stderr.contains("Kommentartext fehlt"),
        "{}",
        ohne.stderr
    );
}

#[test]
fn lehnt_depends_on_zusammen_mit_add_oder_rm_ab() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&["board", "create", "Eins"]);
    projekt.run(&["board", "create", "Zwei"]);

    let lauf = projekt.run(&[
        "board",
        "set",
        "ARC-0002",
        "--depends-on",
        "ARC-0001",
        "--depends-on-add",
        "ARC-0001",
    ]);

    assert_eq!(lauf.code, 1, "{}", lauf.stdout);
    assert!(
        lauf.stderr.contains(
            "--depends-on lässt sich nicht mit --depends-on-add/--depends-on-rm kombinieren."
        ),
        "{}",
        lauf.stderr
    );
}

#[test]
fn leert_zuweisung_zielbranch_und_branch_mit_none_und_clear_branch() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&[
        "board",
        "create",
        "Aufräumen",
        "--target-branch",
        "release/1",
    ]);
    projekt.run(&["board", "claim", "ARC-0001", "--agent", "claude"]);

    assert!(projekt
        .run(&["board", "show", "ARC-0001"])
        .stdout
        .contains("Zielbranch: release/1"));

    assert_eq!(
        projekt
            .run(&[
                "board",
                "set",
                "ARC-0001",
                "--assignee",
                "none",
                "--target-branch",
                "none",
                "--clear-branch"
            ])
            .code,
        0
    );

    let gezeigt = projekt.run(&["board", "show", "ARC-0001"]).stdout;
    assert!(gezeigt.contains("nicht zugewiesen"), "{gezeigt}");
    assert!(!gezeigt.contains("Zielbranch:"), "{gezeigt}");
    assert!(!gezeigt.contains("Branch:"), "{gezeigt}");
}
