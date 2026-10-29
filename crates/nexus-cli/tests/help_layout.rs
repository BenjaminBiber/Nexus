//! Vergleicht die gerenderte Hilfe Zeichen für Zeichen mit der aufgezeichneten commander-Ausgabe
//! aus `contract/cli/help/help.golden.json`. Die Befehle sind hier als clap-Modell nachgebaut,
//! genau wie sie heute in `packages/cli/src/commands` angemeldet sind; verglichen wird nur das
//! Layout, das der Renderer daraus macht.

use std::path::PathBuf;

use clap::{Arg, ArgAction, Command};
use nexus_cli::output::help;

/// Hilfe aus dem Modell, ohne die abschließenden Zeilenumbrüche (die Golden-Datei ist getrimmt)
fn gerendert(path: &str, mut cmd: Command) -> String {
    cmd.build();
    help::render(path, &cmd).trim_end_matches('\n').to_string()
}

/// Die aufgezeichnete Ausgabe von `node packages/cli/dist/main.js <befehl> --help`
fn golden(name: &str) -> String {
    let file =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contract/cli/help/help.golden.json");
    let text = std::fs::read_to_string(&file)
        .unwrap_or_else(|error| panic!("{} nicht lesbar: {error}", file.display()));
    let steps: serde_json::Value = serde_json::from_str(&text).expect("Golden-Datei ist kein JSON");
    steps
        .as_array()
        .expect("Golden-Datei enthält keine Liste")
        .iter()
        .find(|step| step["name"] == name)
        .unwrap_or_else(|| panic!("Kein Schritt „{name}“ in der Golden-Datei"))["response"]
        ["stdout"]
        .as_str()
        .expect("stdout fehlt")
        .to_string()
}

/// Ein Befehl ohne die clap-eigene Hilfe – wie im Befehlsbaum von `nexus`
fn befehl(name: &'static str) -> Command {
    Command::new(name)
        .disable_help_flag(true)
        .disable_help_subcommand(true)
}

/// Eine Option mit Wert, z. B. `--desc <text>`
fn option(long: &'static str, value: &'static str, help: &'static str) -> Arg {
    Arg::new(long).long(long).value_name(value).help(help)
}

/// Eine Option ohne Wert, z. B. `--planned`
fn flag(long: &'static str, help: &'static str) -> Arg {
    Arg::new(long)
        .long(long)
        .help(help)
        .action(ArgAction::SetTrue)
}

/// Ein Pflichtargument, z. B. `<titel>`
fn pflicht(name: &'static str) -> Arg {
    Arg::new(name).required(true)
}

/// Ein freiwilliges Argument, z. B. `[dokument]`
fn frei(name: &'static str) -> Arg {
    Arg::new(name).required(false)
}

const REV_HELP: &str = "nur ändern, wenn das Ticket noch diese Version hat (sonst Konflikt)";
const EXPECT_HELP: &str =
    "nur ändern, wenn der Schritt an dieser Nummer noch diesen Text hat (sonst Konflikt)";
const TARGET_BRANCH_HELP: &str = "Zielbranch, in den der Dispatcher mergt (fehlt er, wird er angelegt; ohne: ausgecheckter Branch des Hauptordners)";

/// Der Befehl aus dem echten Befehlsbaum von `nexus` (nicht nachgebaut)
fn aus_dem_baum(name: &str) -> Command {
    let mut root = <nexus_cli::cli::Cli as clap::CommandFactory>::command();
    root.build();
    root.find_subcommand(name)
        .unwrap_or_else(|| panic!("Befehl „{name}“ fehlt im Befehlsbaum"))
        .clone()
}

/// Ein Unterbefehl aus dem echten Befehlsbaum, über seinen Pfad (z. B. `board list`)
fn aus_dem_baum_tief(pfad: &[&str]) -> Command {
    let mut cmd = aus_dem_baum(pfad[0]);
    for name in &pfad[1..] {
        cmd.build();
        cmd = cmd
            .find_subcommand(name)
            .unwrap_or_else(|| panic!("Befehl „{}“ fehlt im Befehlsbaum", pfad.join(" ")))
            .clone();
    }
    cmd
}

/// Vergleicht die Hilfe eines Unterbefehls aus dem Baum mit der Golden-Datei. Ein Datum im
/// Beispielpfad ersetzt der Vergleich beidseitig: der Vertragslauf schreibt es als `<DATE-n>`
/// auf, wobei `n` die Tage bis zum Tag der Aufzeichnung zählt.
fn wie_aufgezeichnet(pfad: &[&str]) {
    let name = format!("harness {}", pfad.join(" "));
    assert_eq!(
        ohne_datum(&gerendert(&name, aus_dem_baum_tief(pfad))),
        ohne_datum(&golden(&name)),
        "{name}"
    );
}

/// Ersetzt `JJJJ-MM-TT` und `<DATE-n>` durch `<DATUM>`
fn ohne_datum(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let zeichen: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < zeichen.len() {
        if let Some(laenge) = datum_laenge(&zeichen[index..]) {
            out.push_str("<DATUM>");
            index += laenge;
        } else {
            out.push(zeichen[index]);
            index += 1;
        }
    }
    out
}

/// Länge eines Datums am Anfang der Zeichen, sonst `None`
fn datum_laenge(zeichen: &[char]) -> Option<usize> {
    if zeichen.len() >= 10
        && [0, 1, 2, 3, 5, 6, 8, 9]
            .iter()
            .all(|i| zeichen[*i].is_ascii_digit())
        && [4, 7].iter().all(|i| zeichen[*i] == '-')
    {
        return Some(10);
    }
    let rest = zeichen.strip_prefix(&['<', 'D', 'A', 'T', 'E', '-'])?;
    let ziffern = rest.iter().take_while(|c| c.is_ascii_digit()).count();
    (ziffern > 0 && rest.get(ziffern) == Some(&'>')).then_some(6 + ziffern + 1)
}

#[test]
fn hilfe_der_ticketbefehle_wie_aufgezeichnet() {
    // Die Blätter aus `LEAVES` in `packages/cli/tests/integration/help.test.ts`, die dieses
    // Ticket portiert
    for pfad in [
        ["board", "list"],
        ["board", "show"],
        ["board", "create"],
        ["board", "set"],
        ["board", "move"],
        ["board", "delete"],
        ["board", "claim"],
        ["board", "release"],
        ["board", "comment"],
    ] {
        wie_aufgezeichnet(&pfad);
    }
}

#[test]
fn hilfe_der_dispatcher_befehle_wie_aufgezeichnet() {
    // Die Blätter aus `LEAVES`, die dieses Ticket portiert
    for pfad in [
        vec!["board", "dispatch"],
        vec!["board", "dispatch", "status"],
        vec!["board", "dispatch", "run"],
        vec!["board", "dispatch", "pause"],
        vec!["board", "dispatch", "resume"],
        vec!["board", "dispatch", "enable"],
        vec!["board", "dispatch", "disable"],
        vec!["board", "dispatch-runs"],
        vec!["board", "dispatch-runs", "backfill"],
        vec!["board", "plan-start"],
        vec!["board", "question", "research-start"],
    ] {
        wie_aufgezeichnet(&pfad);
    }
}

#[test]
fn hilfe_von_spec_und_stop_wie_aufgezeichnet() {
    for pfad in [
        vec!["spec"],
        vec!["spec", "comments"],
        vec!["spec", "resolve"],
        vec!["stop"],
    ] {
        wie_aufgezeichnet(&pfad);
    }
}

#[test]
fn hilfe_der_fragenbefehle_wie_aufgezeichnet() {
    // Die Blätter aus `LEAVES` zu `board question`; `research-start` kommt aus diesem Ticket
    for pfad in [
        ["board", "question", "add"],
        ["board", "question", "research"],
        ["board", "question", "comment"],
        ["board", "question", "list"],
        ["board", "question", "show"],
        ["board", "question", "answer"],
        ["board", "question", "discard"],
        ["board", "question", "reopen"],
        ["board", "question", "link"],
        ["board", "question", "depends"],
    ] {
        wie_aufgezeichnet(&pfad);
    }
}

#[test]
fn hilfe_von_init_wie_aufgezeichnet() {
    assert_eq!(
        gerendert("harness init", aus_dem_baum("init")),
        golden("harness init")
    );
}

#[test]
fn hilfe_von_install_wie_aufgezeichnet() {
    assert_eq!(
        gerendert("harness install", aus_dem_baum("install")),
        golden("harness install")
    );
}

#[test]
fn hilfe_eines_blattbefehls_ohne_optionen() {
    let hook = befehl("hook")
        .about("Wird von Claude Code aufgerufen (session-start, user-prompt, notification, stop, subagent-start, subagent-stop, session-end); liest die Hook-Meldung von der Standardeingabe.")
        .arg(pflicht("ereignis"))
        .after_help("\nBeispiel:\n  echo '{\"session_id\":\"abc\",\"cwd\":\".\"}' | harness hook session-start");
    assert_eq!(gerendert("harness hook", hook), golden("harness hook"));
}

#[test]
fn hilfe_einer_gruppe_mit_unterbefehlen_und_hilfe_befehl() {
    let spec = befehl("spec")
        .about("Kommentare zu Spec- und Plan-Dokumenten abarbeiten")
        .subcommand(
            befehl("comments")
                .about("Listet offene Kommentare zu Spec- und Plan-Dokumenten, Glossar und ADRs (optional eines Dokuments oder Themas).")
                .arg(frei("dokument")),
        )
        .subcommand(
            befehl("resolve")
                .about("Markiert einen Kommentar als erledigt und speichert eine kurze Antwort.")
                .arg(pflicht("kommentar"))
                .arg(pflicht("antwort")),
        )
        .subcommand(
            befehl("help")
                .about("Hilfe zu einem Befehl anzeigen")
                .arg(frei("befehl")),
        );
    assert_eq!(gerendert("harness spec", spec), golden("harness spec"));
}

#[test]
fn hilfe_mit_vielen_optionen_bricht_wie_commander_um() {
    let create = befehl("create")
        .about("Legt ein Ticket auf dem Board dieses Repos an, sonst auf dem aktiven Board (Standard: Backlog, normal, braucht Planung).")
        .arg(pflicht("titel"))
        .arg(option("desc", "text", "Beschreibung"))
        .arg(option("desc-file", "datei", "Beschreibung aus Datei („-“ = Standardeingabe)"))
        .arg(option("status", "status", "Startspalte"))
        .arg(option("priority", "prio", "low, normal, high oder urgent"))
        .arg(option("label", "label", "Label (wiederholbar oder kommagetrennt)"))
        .arg(flag("planned", "braucht keine Planung"))
        .arg(option("acceptance", "text", "Abnahme: woran man sieht, dass es fertig ist"))
        .arg(option("acceptance-file", "datei", "Abnahme aus Datei"))
        .arg(option("depends-on", "ids", "Voraussetzungen, kommagetrennt"))
        .arg(option("step", "text", "Arbeitsschritt (wiederholbar, in dieser Reihenfolge; Kommas bleiben erhalten)"))
        .arg(option("target-branch", "name", TARGET_BRANCH_HELP))
        .arg(option("board", "id", "Board (Standard: Board dieses Repos, sonst aktives Board)"))
        .arg(flag("force", "trotz offenem Ticket mit gleichem Titel anlegen; Sperren für Voraussetzungen und „nicht mehr nötig“ übergehen"))
        .arg(option("by", "name", "wer anlegt (z. B. claude)"))
        .after_help("\nBeispiele:\n  harness board create \"Login bauen\" --desc \"Kontext\" --priority high --label backend --by claude\n  harness board create \"Export bauen\" --step \"Test schreiben\" --step \"Umsetzen\"");
    assert_eq!(
        gerendert("harness board create", create),
        golden("harness board create")
    );
}

#[test]
fn hilfe_mit_optionen_und_unterbefehlen_teilt_sich_eine_spaltenbreite() {
    let step = befehl("step")
        .about("Hakt einen Schritt des Tickets ab (Nummer ab 1, siehe „harness board show“). Unterbefehle legen Schritte an, bearbeiten, löschen und verschieben sie.")
        .arg(pflicht("id"))
        .arg(pflicht("nummer"))
        .arg(flag("undo", "nimmt den Haken zurück"))
        .arg(option("by", "name", "wer abhakt"))
        .arg(option("rev", "n", REV_HELP))
        .arg(option("expect", "text", EXPECT_HELP))
        .subcommand(
            befehl("done")
                .about("Hakt einen Schritt ab (gleich wie „harness board step <id> <nummer>“).")
                .arg(pflicht("id"))
                .arg(pflicht("nummer"))
                .arg(flag("undo", "nimmt den Haken zurück"))
                .arg(option("by", "name", "wer abhakt")),
        )
        .subcommand(
            befehl("add")
                .about("Hängt einen Schritt an (oder fügt ihn mit --at an dieser Nummer ein); optional mit Anleitung.")
                .arg(pflicht("id"))
                .arg(pflicht("text"))
                .arg(option("by", "name", "wer anlegt (z. B. claude)")),
        )
        .subcommand(
            befehl("edit")
                .about("Ändert Text und/oder Anleitung eines Schritts; der Haken bleibt. Leere Anleitung („“) entfernt sie.")
                .arg(pflicht("id"))
                .arg(pflicht("nummer"))
                .arg(option("text", "text", "neuer Text")),
        )
        .subcommand(
            befehl("rm")
                .about("Löscht einen Schritt; die folgenden rücken eine Nummer nach vorn.")
                .arg(pflicht("id"))
                .arg(pflicht("nummer"))
                .arg(option("by", "name", "wer löscht")),
        )
        .subcommand(
            befehl("move")
                .about("Verschiebt einen Schritt auf eine andere Nummer (samt Haken und Anleitung).")
                .arg(pflicht("id"))
                .arg(pflicht("von"))
                .arg(pflicht("nach"))
                .arg(option("by", "name", "wer verschiebt")),
        )
        .after_help("\nBeispiele:\n  harness board step ARC-0007 2 --by claude\n  harness board step ARC-0007 2 --undo\n  harness board step add ARC-0007 \"Test schreiben\" --detail \"Erwartet: rot\"\n  harness board step done ARC-0007 2");
    assert_eq!(
        gerendert("harness board step", step),
        golden("harness board step")
    );
}
