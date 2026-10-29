//! Die Planungs- und Verwaltungsbefehle des Boards von außen: `board step`, `board note`,
//! `board inbox`, `board plan-json`, `board next-planning`, `board boards` und `board project`
//! gegen eine SQLite-Board-Datenbank in einem eigenen `HARNESS_HOME`.
//! Portiert aus `packages/cli/tests/unit/BoardStepCommands.test.ts`,
//! `BoardInboxCommands.test.ts`, `BoardBoardsCommands.test.ts` und den passenden Fällen von
//! `packages/cli/tests/integration/board-db.test.ts`.

mod common;

use common::temp_project::TempProject;

/// Die Texte der Schritte eines Tickets
fn schritte(projekt: &TempProject, id: &str) -> Vec<String> {
    projekt.json(&["board", "show", id])["steps"]
        .as_array()
        .expect("Schritte")
        .iter()
        .map(|step| step["text"].as_str().expect("Text").to_string())
        .collect()
}

/// Die Haken der Schritte eines Tickets
fn haken(projekt: &TempProject, id: &str) -> Vec<bool> {
    projekt.json(&["board", "show", id])["steps"]
        .as_array()
        .expect("Schritte")
        .iter()
        .map(|step| step["done"].as_bool().expect("Haken"))
        .collect()
}

#[test]
fn legt_mit_create_step_schritte_an_und_bearbeitet_sie_uber_die_unterbefehle() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&[
        "board",
        "create",
        "Export",
        "--step",
        "Test schreiben, rot",
        "--step",
        "Umsetzen",
    ]);
    assert_eq!(
        schritte(&projekt, "ARC-0001"),
        ["Test schreiben, rot", "Umsetzen"]
    );

    let angelegt = projekt.run(&[
        "board",
        "step",
        "add",
        "ARC-0001",
        "Committen",
        "--detail",
        "git commit",
        "--by",
        "claude",
    ]);
    assert_eq!(angelegt.code, 0, "{}", angelegt.stderr);
    assert!(
        angelegt.stdout.contains("  3. [ ] Committen"),
        "{}",
        angelegt.stdout
    );
    let detail = projekt.json(&["board", "show", "ARC-0001"]);
    assert_eq!(detail["steps"][2]["text"], "Committen");
    assert_eq!(detail["steps"][2]["detail"], "git commit");
    assert_eq!(detail["steps"][2]["done"], false);
    let verlauf = detail["history"].as_array().expect("Verlauf");
    let letzter = verlauf.last().expect("Eintrag");
    assert_eq!(letzter["byName"], "claude");
    assert_eq!(letzter["note"], "Schritt hinzugefügt");

    projekt.run(&["board", "step", "add", "ARC-0001", "Lesen", "--at", "1"]);
    projekt.run(&[
        "board",
        "step",
        "edit",
        "ARC-0001",
        "2",
        "--text",
        "Test zuerst",
    ]);
    projekt.run(&["board", "step", "move", "ARC-0001", "4", "1"]);
    projekt.run(&["board", "step", "rm", "ARC-0001", "3"]);
    assert_eq!(
        schritte(&projekt, "ARC-0001"),
        ["Committen", "Lesen", "Umsetzen"]
    );
}

#[test]
fn hakt_mit_der_alten_form_und_mit_step_done_ab_und_zuruck() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&["board", "create", "Export", "--step", "A", "--step", "B"]);

    projekt.run(&["board", "step", "ARC-0001", "2", "--by", "claude"]);
    assert_eq!(haken(&projekt, "ARC-0001"), [false, true]);
    let detail = projekt.json(&["board", "show", "ARC-0001"]);
    let letzter = detail["history"]
        .as_array()
        .expect("Verlauf")
        .last()
        .expect("Eintrag");
    assert_eq!(letzter["byName"], "claude");
    assert_eq!(letzter["field"], "step");

    projekt.run(&["board", "step", "ARC-0001", "2", "--undo"]);
    projekt.run(&["board", "step", "done", "ARC-0001", "1"]);
    assert_eq!(haken(&projekt, "ARC-0001"), [true, false]);

    projekt.run(&[
        "board", "step", "done", "ARC-0001", "1", "--undo", "--by", "claude",
    ]);
    assert_eq!(haken(&projekt, "ARC-0001"), [false, false]);
    let detail = projekt.json(&["board", "show", "ARC-0001"]);
    let letzter = detail["history"]
        .as_array()
        .expect("Verlauf")
        .last()
        .expect("Eintrag");
    assert_eq!(letzter["byName"], "claude");
    assert_eq!(letzter["from"], "1: erledigt");
    assert_eq!(letzter["to"], "1: offen");
}

#[test]
fn meldet_ungultige_nummern() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&["board", "create", "Export", "--step", "A"]);

    let ungultig = projekt.run(&["--json", "board", "step", "rm", "ARC-0001", "x"]);
    assert_eq!(ungultig.code, 1, "{}", ungultig.stdout);
    assert_eq!(ungultig.json()["error"]["code"], "step-invalid");

    let fehlt = projekt.run(&["--json", "board", "step", "move", "ARC-0001", "1", "3"]);
    assert_eq!(fehlt.code, 1, "{}", fehlt.stdout);
    assert_eq!(fehlt.json()["error"]["code"], "step-not-found");
}

#[test]
fn schutzt_mit_expect_und_rev_vor_gleichzeitigen_anderungen() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&["board", "create", "Export", "--step", "A", "--step", "B"]);
    projekt.run(&["board", "step", "move", "ARC-0001", "2", "1"]);

    // Schritt 1 heißt inzwischen „B“: jede Form mit --expect A meldet denselben Konflikt
    for args in [
        vec!["board", "step", "ARC-0001", "1", "--expect", "A"],
        vec!["board", "step", "done", "ARC-0001", "1", "--expect", "A"],
        vec![
            "board", "step", "edit", "ARC-0001", "1", "--text", "X", "--expect", "A",
        ],
        vec!["board", "step", "rm", "ARC-0001", "1", "--expect", "A"],
        vec![
            "board", "step", "move", "ARC-0001", "1", "2", "--expect", "A",
        ],
    ] {
        let mit_json: Vec<&str> = std::iter::once("--json")
            .chain(args.iter().copied())
            .collect();
        let lauf = projekt.run(&mit_json);
        assert_eq!(lauf.code, 1, "{args:?}: {}", lauf.stdout);
        let fehler = &lauf.json()["error"];
        assert_eq!(fehler["code"], "conflict", "{args:?}");
        assert_eq!(
            fehler["message"],
            "ARC-0001: Schritt 1 ist inzwischen „B“ – die Schritte wurden geändert.",
            "{args:?}"
        );
    }

    let alte_version = projekt.run(&[
        "--json", "board", "step", "rm", "ARC-0001", "1", "--rev", "1",
    ]);
    assert_eq!(alte_version.code, 1, "{}", alte_version.stdout);
    assert_eq!(alte_version.json()["error"]["code"], "conflict");

    let kaputt = projekt.run(&[
        "--json", "board", "step", "rm", "ARC-0001", "1", "--rev", "x",
    ]);
    assert_eq!(kaputt.code, 1, "{}", kaputt.stdout);
    assert_eq!(kaputt.json()["error"]["code"], "rev-invalid");

    // Mit passendem Schutz greifen beide Änderungen
    let rev = projekt.json(&["board", "show", "ARC-0001"])["rev"]
        .as_i64()
        .expect("rev");
    assert_eq!(
        projekt
            .run(&[
                "board",
                "step",
                "ARC-0001",
                "1",
                "--expect",
                "B",
                "--rev",
                &rev.to_string()
            ])
            .code,
        0
    );
    assert_eq!(
        projekt
            .run(&[
                "board",
                "step",
                "rm",
                "ARC-0001",
                "2",
                "--expect",
                "A",
                "--rev",
                &(rev + 1).to_string()
            ])
            .code,
        0
    );
    assert_eq!(schritte(&projekt, "ARC-0001"), ["B"]);
    assert_eq!(haken(&projekt, "ARC-0001"), [true]);
}

#[test]
fn listet_die_eigenen_aufgaben_nummeriert_mit_aktionshinweis_als_markdown() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&["board", "create", "Fertig"]);
    projekt.run(&[
        "board",
        "set",
        "ARC-0001",
        "--status",
        "blocked",
        "--blocked-reason",
        "Human Interaction: ansehen",
    ]);
    projekt.run(&["board", "create", "Konflikt"]);
    projekt.run(&[
        "board",
        "set",
        "ARC-0002",
        "--status",
        "blocked",
        "--blocked-reason",
        "Merge: a.ts",
    ]);
    let notiz = projekt.json(&["board", "note", "add", "Cache prüfen"])["id"]
        .as_i64()
        .expect("Notiz-Nummer");
    std::fs::write(
        projekt.home().join("inbox-order.json"),
        "{\n  \"version\": 1,\n  \"keys\": [\n    \"ticket:ARC-0001\",\n    \"ticket:ARC-0002\"\n  ]\n}\n",
    )
    .expect("Reihenfolge");

    let lauf = projekt.run(&["board", "inbox", "--md"]);
    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert_eq!(
        lauf.stdout.lines().collect::<Vec<_>>(),
        [
            format!("1. **Notiz** `Notiz {notiz}` Cache prüfen (ARC · Sprint) → `nexus board note take {notiz}`"),
            "2. **Abnahme** `ARC-0001` Fertig (ARC · Sprint) – Human Interaction: ansehen → `nexus board accept ARC-0001`".to_string(),
            "3. **Merge gescheitert** `ARC-0002` Konflikt (ARC · Sprint) – Merge: a.ts → `nexus board merge ARC-0002`".to_string(),
        ]
    );
}

#[test]
fn filtert_die_eigenen_aufgaben_nach_art_und_meldet_eine_leere_liste() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&["board", "create", "Konflikt"]);
    projekt.run(&[
        "board",
        "set",
        "ARC-0001",
        "--status",
        "blocked",
        "--blocked-reason",
        "Merge: a.ts",
    ]);

    let gefiltert = projekt.run(&["board", "inbox", "--kind", "accept,note"]);
    assert_eq!(gefiltert.code, 0, "{}", gefiltert.stderr);
    assert_eq!(gefiltert.stdout.trim(), "Nichts wartet auf dich.");

    let unbekannt = projekt.run(&["--json", "board", "inbox", "--kind", "quatsch"]);
    assert_eq!(unbekannt.code, 1, "{}", unbekannt.stdout);
    assert_eq!(unbekannt.json()["error"]["code"], "input-invalid");
}

#[test]
fn liefert_die_eigenen_aufgaben_mit_json_als_liste() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&["board", "create", "Zugang"]);
    projekt.run(&[
        "board",
        "set",
        "ARC-0001",
        "--status",
        "blocked",
        "--blocked-reason",
        "Zugang fehlt",
    ]);

    let eintraege = projekt.json(&["board", "inbox"]);
    let liste = eintraege.as_array().expect("Liste");
    assert_eq!(liste[0]["key"], "ticket:ARC-0001");
    assert_eq!(liste[0]["kind"], "blocked");
    assert_eq!(liste[0]["detail"], "Zugang fehlt");
}

#[test]
fn boards_repo_schreibt_die_lokale_zuordnung_nicht_die_db() {
    let projekt = TempProject::new();
    let board = projekt.seed();

    let lauf = projekt.run(&[
        "board",
        "boards",
        "repo",
        &board.to_string(),
        "C:/Projects/Arches",
    ]);
    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert_eq!(
        projekt.repo_file_path(board).as_deref(),
        Some("C:/Projects/Arches")
    );
    assert_eq!(projekt.db_repo_path(board), None);
    assert!(
        lauf.stdout.contains(&format!(
            "Mein Repo-Pfad von Board {board}: C:/Projects/Arches"
        )),
        "{}",
        lauf.stdout
    );
}

#[test]
fn boards_repo_ohne_pfad_zeigt_den_eigenen_pfad_und_clear_entfernt_ihn() {
    let projekt = TempProject::new();
    let board = projekt.seed();
    let id = board.to_string();
    projekt.run(&["board", "boards", "repo", &id, "C:/Projects/Arches"]);

    // Ohne Pfad wird nur gezeigt, die Zuordnung bleibt unverändert
    let gezeigt = projekt.run(&["board", "boards", "repo", &id]);
    assert_eq!(gezeigt.code, 0, "{}", gezeigt.stderr);
    assert!(
        gezeigt.stdout.contains("C:/Projects/Arches"),
        "{}",
        gezeigt.stdout
    );
    assert_eq!(
        projekt.repo_file_path(board).as_deref(),
        Some("C:/Projects/Arches")
    );

    let geleert = projekt.run(&["board", "boards", "repo", &id, "--clear"]);
    assert_eq!(geleert.code, 0, "{}", geleert.stderr);
    assert!(geleert.stdout.contains("(keiner)"), "{}", geleert.stdout);
    assert_eq!(projekt.repo_file_path(board), None);

    // Pfad und --clear zusammen sind ein Bedienfehler
    let beides = projekt.run(&["--json", "board", "boards", "repo", &id, "C:/x", "--clear"]);
    assert_eq!(beides.code, 1, "{}", beides.stdout);
    assert_eq!(beides.json()["error"]["code"], "usage");
}

#[test]
fn boards_list_zeigt_den_eigenen_pfad_und_kennzeichnet_den_db_altbestand() {
    let projekt = TempProject::new();
    let board = projekt.seed();
    projekt.set_db_repo_path(board, "C:/alt");

    let altbestand = projekt.run(&["board", "boards", "list"]).stdout;
    assert!(
        altbestand.contains("C:/alt (aus der DB, Altbestand)"),
        "{altbestand}"
    );

    projekt.run(&["board", "boards", "repo", &board.to_string(), "D:/neu"]);
    let eigener = projekt.run(&["board", "boards", "list"]).stdout;
    assert!(eigener.contains("D:/neu"), "{eigener}");
    assert!(!eigener.contains("Altbestand"), "{eigener}");
}

#[test]
fn legt_projekt_und_board_an_schaltet_um_und_listet() {
    let projekt = TempProject::new();

    assert_eq!(
        projekt
            .run(&["board", "project", "create", "arc", "Arches"])
            .code,
        0
    );
    let projekte = projekt.run(&["board", "project", "list"]).stdout;
    assert!(projekte.contains("ARC  Arches"), "{projekte}");

    let board = projekt.json(&["board", "boards", "create", "ARC", "Sprint", "--use"]);
    assert_eq!(board["name"], "Sprint");
    assert!(board["projectId"].is_i64(), "{board}");
    let id = board["id"].as_i64().expect("Board-Nummer").to_string();

    let gelistet = projekt.run(&["board", "boards", "list"]).stdout;
    assert!(
        gelistet.contains(&format!("* {id}  ARC  Sprint  @Test")),
        "{gelistet}"
    );

    assert_eq!(
        projekt
            .run(&["board", "boards", "repo", &id, "C:\\Projects\\Arches"])
            .code,
        0
    );
    let mit_repo = projekt.run(&["board", "boards", "list"]).stdout;
    assert!(mit_repo.contains("C:\\Projects\\Arches"), "{mit_repo}");

    assert_eq!(
        projekt
            .run(&["board", "boards", "rename", &id, "Sprint 2"])
            .code,
        0
    );
    let eigene = projekt.run(&["board", "boards", "list", "--mine"]).stdout;
    assert!(eigene.contains("Sprint 2"), "{eigene}");

    // Umbenennen des Projekts: der Key bleibt
    let umbenannt = projekt.run(&["board", "project", "rename", "ARC", "Arches Shop"]);
    assert_eq!(umbenannt.code, 0, "{}", umbenannt.stderr);
    assert!(
        umbenannt
            .stdout
            .contains("Projekt ARC heißt jetzt Arches Shop"),
        "{}",
        umbenannt.stdout
    );
}

#[test]
fn boards_use_schaltet_das_aktive_board_um() {
    let projekt = TempProject::new();
    let aktiv = projekt.seed();
    let anderes = projekt.create_board("Zweites");

    let lauf = projekt.run(&["board", "boards", "use", &anderes.to_string()]);
    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert!(
        lauf.stdout
            .contains(&format!("Aktives Board: {anderes} Zweites")),
        "{}",
        lauf.stdout
    );
    let gelistet = projekt.run(&["board", "boards", "list"]).stdout;
    assert!(gelistet.contains(&format!("* {anderes}")), "{gelistet}");
    assert!(
        gelistet.contains(&format!("  {aktiv}  ARC  Sprint")),
        "{gelistet}"
    );
}

#[test]
fn loscht_ein_board_nur_ohne_offene_tickets_oder_mit_delete_open() {
    let projekt = TempProject::new();
    let board = projekt.seed().to_string();
    projekt.run(&["board", "create", "Offen"]);

    let abgelehnt = projekt.run(&["board", "boards", "delete", &board]);
    assert_eq!(abgelehnt.code, 1, "{}", abgelehnt.stdout);
    assert!(
        abgelehnt.stderr.contains("ARC-0001"),
        "{}",
        abgelehnt.stderr
    );

    assert_eq!(
        projekt
            .run(&["board", "boards", "delete", &board, "--delete-open"])
            .code,
        0
    );
    let leer = projekt.run(&["board", "boards", "list"]).stdout;
    assert!(leer.contains("Keine Boards"), "{leer}");
}

#[test]
fn halt_notizen_fest_macht_daraus_ein_ticket_und_verwirft_nur_mit_grund() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&["board", "create", "Laufend"]);

    let notiz = projekt.json(&[
        "board",
        "note",
        "add",
        "Cache läuft nie ab",
        "--from",
        "ARC-0001",
        "--label",
        "cache",
        "--author",
        "claude",
    ])["id"]
        .as_i64()
        .expect("Notiz-Nummer")
        .to_string();
    let md = projekt.run(&["board", "note", "list", "--md"]).stdout;
    assert!(md.contains("Cache läuft nie ab"), "{md}");
    // Die Herkunft wird auf die kanonische Ticket-ID gebracht
    let aus_ticket = projekt
        .run(&["board", "note", "list", "--from", "arc-1"])
        .stdout;
    assert!(aus_ticket.contains("Cache läuft nie ab"), "{aus_ticket}");

    let uebernommen = projekt.json(&["board", "note", "take", &notiz, "--priority", "high"]);
    assert_eq!(uebernommen["ticket"]["id"], "ARC-0002");
    assert_eq!(uebernommen["ticket"]["priority"], "high");
    assert_eq!(
        uebernommen["ticket"]["labels"],
        serde_json::json!(["cache"])
    );

    let offen = projekt.run(&["board", "note", "list"]).stdout;
    assert!(offen.contains("(keine Notizen)"), "{offen}");
    let alle = projekt
        .run(&["board", "note", "list", "--state", "alle"])
        .stdout;
    assert!(alle.contains("übernommen"), "{alle}");

    let andere = projekt.json(&["board", "note", "add", "Egal"])["id"]
        .as_i64()
        .expect("Notiz-Nummer")
        .to_string();
    assert_ne!(projekt.run(&["board", "note", "drop", &andere]).code, 0);
    assert_eq!(
        projekt
            .run(&[
                "board",
                "note",
                "drop",
                &andere,
                "--reason",
                "erledigt sich"
            ])
            .code,
        0
    );
    let gezeigt = projekt.run(&["board", "note", "show", &andere]).stdout;
    assert!(gezeigt.contains("Grund: erledigt sich"), "{gezeigt}");
}

#[test]
fn zahlt_eine_erneut_aufgefallene_notiz_hoch() {
    let projekt = TempProject::new();
    projekt.seed();
    let notiz = projekt.json(&["board", "note", "add", "Cache läuft nie ab"])["id"]
        .as_i64()
        .expect("Notiz-Nummer")
        .to_string();

    let lauf = projekt.run(&["board", "note", "seen", &notiz]);
    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert!(
        lauf.stdout
            .contains(&format!("Notiz {notiz}: 2× aufgefallen.")),
        "{}",
        lauf.stdout
    );

    let unbekannt = projekt.run(&["--json", "board", "note", "seen", "x"]);
    assert_eq!(unbekannt.code, 1, "{}", unbekannt.stdout);
    assert_eq!(unbekannt.json()["error"]["code"], "note-id-invalid");
}

#[test]
fn nimmt_mit_from_das_board_des_herkunftstickets() {
    let projekt = TempProject::new();
    let aktiv = projekt.seed();
    let repo_board = projekt.create_board("Repo");
    projekt.assign_repo(repo_board, &projekt.path);
    let vor_init = projekt.json(&["board", "create", "Vor init"]);
    assert_eq!(projekt.run(&["init"]).code, 0);

    // Ohne --from das Board des Repos …
    assert_eq!(
        projekt.json(&["board", "note", "add", "Repo-Notiz"])["boardId"],
        serde_json::json!(repo_board)
    );
    // … mit --from das Board des Herkunftstickets
    let herkunft = vor_init["id"].as_str().expect("Ticket-ID");
    assert_eq!(
        projekt.json(&[
            "board",
            "note",
            "add",
            "Herkunfts-Notiz",
            "--from",
            herkunft
        ])["boardId"],
        serde_json::json!(aktiv)
    );
}

#[test]
fn plant_uber_next_planning_und_plan_json() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&["board", "create", "Planen"]);

    let naechstes = projekt.json(&["board", "next-planning", "--claim", "claude"]);
    assert_eq!(naechstes["ticket"]["id"], "ARC-0001");
    assert_eq!(naechstes["ticket"]["planningClaimBy"], "claude");

    let eingabe = serde_json::json!({
        "plan": "## Ziel",
        "handbook": "# HB",
        "diagram": "flowchart TD\n A-->B",
    })
    .to_string();
    let geplant = projekt.run_with_input(
        &[
            "--json",
            "board",
            "plan-json",
            "ARC-0001",
            "--file",
            "-",
            "--by",
            "claude",
        ],
        &eingabe,
    );
    assert_eq!(geplant.code, 0, "{}", geplant.stderr);
    let wert = geplant.json();
    assert_eq!(wert["status"], "planned");
    assert_eq!(wert["needsPlanning"], false);
    assert_eq!(wert["plannedBy"], "claude");

    assert_eq!(
        projekt.json(&["board", "next-planning"]),
        serde_json::json!({ "ticket": null })
    );
}

#[test]
fn lehnt_kaputte_eingaben_von_plan_json_ab() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&["board", "create", "Planen"]);

    for (eingabe, meldung) in [
        (
            "{kaputt",
            "Die Eingabe für plan-json ist kein gültiges JSON.",
        ),
        ("[]", "Die Eingabe für plan-json muss ein JSON-Objekt sein."),
        (
            r#"{"plan":7}"#,
            "Das Feld „plan“ in der Eingabe für plan-json muss ein Text sein.",
        ),
    ] {
        let lauf = projekt.run_with_input(
            &["--json", "board", "plan-json", "ARC-0001", "--file", "-"],
            eingabe,
        );
        assert_eq!(lauf.code, 1, "{eingabe}: {}", lauf.stdout);
        let fehler = &lauf.json()["error"];
        assert_eq!(fehler["code"], "input-invalid", "{eingabe}");
        assert_eq!(fehler["message"], meldung, "{eingabe}");
    }
}

#[test]
fn schreibt_plan_handbuch_und_diagramm_mit_board_plan() {
    let projekt = TempProject::new();
    projekt.seed();
    projekt.run(&["board", "create", "Planen"]);

    let lauf = projekt.run(&[
        "board",
        "plan",
        "ARC-0001",
        "--plan",
        "## Ziel",
        "--handbook",
        "# HB",
        "--diagram",
        "flowchart TD\n A-->B",
        "--by",
        "claude",
    ]);
    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert!(
        lauf.stdout.contains("ARC-0001 geplant (planned)"),
        "{}",
        lauf.stdout
    );

    let ticket = projekt.json(&["board", "show", "ARC-0001"]);
    assert_eq!(ticket["plan"], "## Ziel");
    assert_eq!(ticket["handbook"], "# HB");
    assert_eq!(ticket["needsPlanning"], false);
}

#[test]
fn next_planning_ohne_ticket_meldet_es_im_klartext() {
    let projekt = TempProject::new();
    projekt.seed();

    let lauf = projekt.run(&["board", "next-planning"]);
    assert_eq!(lauf.code, 0, "{}", lauf.stderr);
    assert_eq!(lauf.stdout.trim(), "Kein Ticket mit Planungsbedarf.");
}
