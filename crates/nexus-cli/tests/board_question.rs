//! Die Fragenbefehle des Boards von außen: `board question add|research|comment|list|show|
//! answer|discard|reopen|link|depends` gegen eine SQLite-Board-Datenbank in einem eigenen
//! `HARNESS_HOME`.
//! Portiert aus `packages/cli/tests/integration/board-question.test.ts`.

mod common;

use common::temp_project::TempProject;

/// Home ohne `board.yaml`: ein Ordner, den kein Testprojekt eingerichtet hat
fn home_ohne_konfig() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("harness-home-")
        .tempdir()
        .expect("Temp-Ordner")
}

/// Projekt `ARC` mit aktivem Board `Sprint` und dem Ticket `ARC-0001`
fn projekt_mit_ticket() -> TempProject {
    let projekt = TempProject::new();
    projekt.seed();
    let lauf = projekt.run(&["board", "create", "Login bauen", "--acceptance", "geht"]);
    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    projekt
}

#[test]
fn zeigt_die_unterbefehle_in_der_hilfe() {
    let projekt = TempProject::new();

    let lauf = projekt.run(&["board", "question", "--help"]);

    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    for wort in [
        "add", "list", "show", "research", "answer", "discard", "reopen", "link", "depends",
        "comment",
    ] {
        assert!(lauf.stdout.contains(wort), "{wort}: {}", lauf.stdout);
    }
}

#[test]
fn der_hilfe_befehl_zeigt_die_gruppe_und_einzelne_befehle() {
    let projekt = TempProject::new();

    let gruppe = projekt.run(&["board", "question", "help"]);
    assert_eq!(gruppe.code, 0, "{}", gruppe.stderr);
    assert!(
        gruppe.stdout.starts_with("Aufruf: nexus board question"),
        "{}",
        gruppe.stdout
    );
    assert!(
        gruppe.stdout.contains("help [befehl]"),
        "der Hilfe-Befehl steht in der Liste: {}",
        gruppe.stdout
    );

    let blatt = projekt.run(&["board", "question", "help", "add"]);
    assert_eq!(blatt.code, 0, "{}", blatt.stderr);
    assert!(
        blatt
            .stdout
            .starts_with("Aufruf: nexus board question add [options] <titel>"),
        "{}",
        blatt.stdout
    );
}

#[test]
fn der_hilfe_befehl_zeigt_bei_unbekanntem_namen_die_gruppe_auf_stderr() {
    let projekt = TempProject::new();

    let lauf = projekt.run(&["board", "question", "help", "quatsch"]);

    assert_eq!(lauf.code, 1);
    assert_eq!(lauf.stdout, "");
    assert!(
        lauf.stderr.starts_with("Aufruf: nexus board question"),
        "{}",
        lauf.stderr
    );
}

#[test]
fn verweigert_das_entscheiden_innerhalb_einer_session_ohne_datenbankzugriff() {
    let projekt = TempProject::new();
    let home = home_ohne_konfig();

    for args in [
        vec![
            "board",
            "question",
            "answer",
            "ARC-F0001",
            "--text",
            "x",
            "--reason",
            "y",
        ],
        vec!["board", "question", "discard", "ARC-F0001", "--reason", "y"],
        vec!["board", "question", "reopen", "ARC-F0001"],
    ] {
        let lauf = projekt.run_in_session_in_home(home.path(), "s-1", &args);

        assert_eq!(lauf.code, 1, "{}: {}", args.join(" "), lauf.stdout);
        assert!(lauf.stderr.contains("trifft ein Mensch"), "{}", lauf.stderr);
        assert!(!lauf.stderr.contains("board.yaml"), "{}", lauf.stderr);
    }
}

#[test]
fn fragt_recherchiert_sperrt_das_ticket_und_gibt_es_nach_der_entscheidung_frei() {
    let projekt = projekt_mit_ticket();

    let angelegt = projekt.json(&[
        "board",
        "question",
        "add",
        "Welcher Speicher?",
        "--ticket",
        "ARC-0001",
        "--tag",
        "speicher",
        "--kind",
        "session",
        "--author",
        "claude",
        "--session",
        "s-1",
    ]);
    assert_eq!(angelegt["id"], "ARC-F0001");
    assert_eq!(angelegt["ticketIds"][0], "ARC-0001");
    assert_eq!(angelegt["originKind"], "session");
    assert_eq!(angelegt["originName"], "claude");
    assert_eq!(angelegt["originSession"], "s-1");

    let liste = projekt.run(&["board", "list"]);
    assert!(
        liste.stdout.contains("[Frage ARC-F0001]"),
        "{}",
        liste.stdout
    );

    let gesperrt = projekt.run(&["board", "claim", "ARC-0001", "--agent", "claude"]);
    assert_eq!(gesperrt.code, 1, "{}", gesperrt.stdout);
    assert!(
        gesperrt.stderr.contains("wartet auf Frage ARC-F0001"),
        "{}",
        gesperrt.stderr
    );
    // Den --force-Hinweis ergänzt die CLI (die Kernmeldung ist neutral)
    assert!(
        gesperrt.stderr.contains("Mit --force übergehen."),
        "{}",
        gesperrt.stderr
    );

    let recherchiert = projekt.run_with_input(
        &[
            "board",
            "question",
            "research",
            "ARC-F0001",
            "--from",
            "-",
            "--kind",
            "session",
            "--author",
            "claude",
        ],
        r#"{"context":"Heute Datei","options":[{"title":"Datei","text":"JSON"},{"title":"DB","text":"SQL","recommended":true}]}"#,
    );
    assert_eq!(recherchiert.code, 0, "{}", recherchiert.stderr);
    assert!(
        recherchiert
            .stdout
            .contains("2 Optionen (O1 bis O2), O2 empfohlen"),
        "{}",
        recherchiert.stdout
    );

    assert!(projekt
        .run(&["board", "question", "list"])
        .stdout
        .contains("ARC-F0001"));
    assert!(projekt
        .run(&["board", "question", "list", "--md"])
        .stdout
        .contains("Betrifft: ARC-0001"));

    let gezeigt = projekt
        .run(&["board", "question", "show", "ARC-F0001"])
        .stdout;
    assert!(gezeigt.contains("O2 (empfohlen): DB"), "{gezeigt}");
    assert!(
        gezeigt.contains("Herkunft: claude (Session), Session s-1"),
        "{gezeigt}"
    );

    let in_session = projekt.run_in_session(
        "s-1",
        &[
            "board",
            "question",
            "answer",
            "ARC-F0001",
            "--option",
            "O2",
            "--reason",
            "geteilt",
        ],
    );
    assert_eq!(in_session.code, 1, "{}", in_session.stdout);

    let beantwortet = projekt.run(&[
        "board",
        "question",
        "answer",
        "ARC-F0001",
        "--option",
        "O2",
        "--reason",
        "geteilt",
    ]);
    assert_eq!(beantwortet.code, 0, "{}", beantwortet.stderr);
    assert_eq!(
        projekt.json(&["board", "show", "ARC-0001"])["waitingOnQuestions"]
            .as_array()
            .expect("Liste")
            .len(),
        0
    );
    assert_eq!(
        projekt
            .run(&["board", "claim", "ARC-0001", "--agent", "claude"])
            .code,
        0
    );

    let beantwortete = projekt.json(&["board", "question", "list", "--state", "beantwortet"]);
    let ids: Vec<&str> = beantwortete["questions"]
        .as_array()
        .expect("Fragen")
        .iter()
        .map(|q| q["id"].as_str().expect("ID"))
        .collect();
    assert_eq!(ids, ["ARC-F0001"]);
}

#[test]
fn verknupft_tickets_und_abhangigkeiten_kommentiert_verwirft_und_offnet_wieder() {
    let projekt = projekt_mit_ticket();
    assert_eq!(projekt.run(&["board", "create", "Export"]).code, 0);
    assert_eq!(projekt.run(&["board", "question", "add", "Erste"]).code, 0);
    assert_eq!(
        projekt
            .run(&[
                "board",
                "question",
                "add",
                "Zweite",
                "--depends",
                "ARC-F0001"
            ])
            .code,
        0
    );

    let verknupft = projekt.run(&[
        "board",
        "question",
        "link",
        "ARC-F0002",
        "--add",
        "ARC-0001",
        "--add",
        "ARC-0002",
    ]);
    assert!(
        verknupft.stdout.contains("ARC-0001, ARC-0002"),
        "{}",
        verknupft.stdout
    );
    let entfernt = projekt.run(&[
        "board",
        "question",
        "link",
        "ARC-F0002",
        "--remove",
        "ARC-0001",
    ]);
    assert!(
        entfernt
            .stdout
            .contains("Betroffene Tickets von ARC-F0002: ARC-0002"),
        "{}",
        entfernt.stdout
    );

    let kreis = projekt.run(&[
        "board",
        "question",
        "depends",
        "ARC-F0001",
        "--add",
        "ARC-F0002",
    ]);
    assert_eq!(kreis.code, 1, "{}", kreis.stdout);
    assert!(kreis.stderr.contains("Kreis"), "{}", kreis.stderr);

    assert_eq!(
        projekt
            .run(&[
                "board",
                "question",
                "comment",
                "ARC-F0002",
                "Bitte bis Freitag",
                "--author",
                "claude"
            ])
            .code,
        0
    );
    assert!(projekt
        .run(&["board", "question", "show", "ARC-F0002"])
        .stdout
        .contains("Bitte bis Freitag"));

    assert_eq!(
        projekt
            .run(&[
                "board",
                "question",
                "discard",
                "ARC-F0002",
                "--reason",
                "doppelt"
            ])
            .code,
        0
    );
    assert_eq!(
        projekt
            .run(&[
                "board",
                "question",
                "reopen",
                "ARC-F0002",
                "--reason",
                "doch nötig"
            ])
            .code,
        0
    );

    let gezeigt = projekt.json(&["board", "question", "show", "ARC-F0002"]);
    assert_eq!(gezeigt["state"], "open");
    assert_eq!(gezeigt["ticketIds"][0], "ARC-0002");
    assert_eq!(gezeigt["comments"][0]["body"], "Bitte bis Freitag");
}

#[test]
fn ohne_add_und_remove_melden_link_und_depends_usage() {
    let projekt = projekt_mit_ticket();
    assert_eq!(projekt.run(&["board", "question", "add", "Erste"]).code, 0);

    for befehl in ["link", "depends"] {
        let lauf = projekt.run(&["board", "question", befehl, "ARC-F0001"]);

        assert_eq!(lauf.code, 1, "{befehl}: {}", lauf.stdout);
        assert!(
            lauf.stderr.contains("Bitte --add oder --remove angeben."),
            "{befehl}: {}",
            lauf.stderr
        );
    }
}

#[test]
fn ein_unbekannter_zustand_und_eine_unbekannte_frage_werden_abgewiesen() {
    let projekt = projekt_mit_ticket();

    let zustand = projekt.run(&["board", "question", "list", "--state", "quatsch"]);
    assert_eq!(zustand.code, 1, "{}", zustand.stdout);
    assert!(
        zustand.stderr.contains(
            "Zustand „quatsch“ ist ungültig. Erlaubt: offen, beantwortet, verworfen, alle."
        ),
        "{}",
        zustand.stderr
    );

    let frage = projekt.run(&["board", "question", "show", "ARC-F0099"]);
    assert_eq!(frage.code, 1, "{}", frage.stdout);
    assert!(
        frage.stderr.contains("Frage ARC-F0099 gibt es nicht."),
        "{}",
        frage.stderr
    );
}

#[test]
fn ohne_fragen_meldet_die_liste_dass_nichts_gefunden_wurde() {
    let projekt = projekt_mit_ticket();

    assert!(projekt
        .run(&["board", "question", "list"])
        .stdout
        .contains("(keine Fragen gefunden)"));
    assert!(projekt
        .run(&["board", "question", "list", "--md"])
        .stdout
        .contains("- (keine Fragen gefunden)"));
}
