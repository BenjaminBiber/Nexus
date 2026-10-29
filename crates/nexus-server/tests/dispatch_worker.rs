//! Prozess-Tests des Workers: portiert aus den Worker-Fällen in
//! `packages/server/tests/unit/dispatch/DispatchWorker.test.ts`. Statt des Skripts aus
//! `FakeClaude.ts` kommt das Binary `nexus-fake-claude` zum Einsatz.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use nexus_core::dispatch::{
    RunEnd, RunKind, RunOutputReader, RunRecord, RunSettings, RunStore, WorktreeSetupStep,
};
use nexus_core::{Clock, HarnessHome};
use nexus_server::dispatch::DispatchWorker;

/// Der Ersatz für Claude Code
fn fake_claude() -> String {
    env!("CARGO_BIN_EXE_nexus-fake-claude").to_string()
}

/// Einstellungen eines Laufs wie im TypeScript-Test (`RUN`)
fn einstellungen(timeout_minutes: i64) -> RunSettings {
    RunSettings {
        model: Some("opus".to_string()),
        effort: Some("high".to_string()),
        max_turns: 80,
        timeout_minutes,
        profile: "standard".to_string(),
        why: "Standardprofil standard".to_string(),
        reason: None,
        config_hash: None,
        rule_index: None,
        escalated_from: None,
    }
}

/// Ein Lauf mit Vorgaben, die jeder Test überschreiben kann
fn lauf(id: &str, cwd: &Path, log_file: &Path) -> RunRecord {
    RunRecord {
        id: id.to_string(),
        kind: RunKind::Work,
        ticket_id: "ARC-0001".to_string(),
        board_id: 1,
        title: "T".to_string(),
        agent: format!("junior-{id}"),
        cwd: cwd.to_string_lossy().into_owned(),
        branch: "b".to_string(),
        target: None,
        run: einstellungen(1),
        attempt: None,
        command: fake_claude(),
        args: vec!["-p".to_string()],
        prompt: "Arbeitsanweisung".to_string(),
        log_file: log_file.to_string_lossy().into_owned(),
        started_at: "2026-10-29T09:00:00.000Z".to_string(),
        pid: None,
        setup: None,
        setup_timeout_minutes: None,
        question_id: None,
        research_before: None,
        planned_before: None,
        repo_root: None,
    }
}

/// Umgebung des Workers (sie wird vollständig vorgegeben)
fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// Ein Benutzerordner samt Worker darin
struct Umgebung {
    dir: tempfile::TempDir,
    home: HarnessHome,
}

impl Umgebung {
    fn neu() -> Self {
        let dir = tempfile::tempdir().expect("Ordner");
        let home = HarnessHome::at(dir.path());
        Self { dir, home }
    }

    fn pfad(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn runs(&self) -> RunStore {
        RunStore::new(self.home.clone())
    }

    /// Worker mit kurzer Pause zwischen den Leseversuchen
    fn worker(&self) -> DispatchWorker {
        DispatchWorker::new(self.home.clone()).with_read_pause(Duration::from_millis(100))
    }

    fn ergebnis(&self, id: &str) -> RunEnd {
        self.runs()
            .result(id)
            .expect("Ergebnis lesbar")
            .expect("Ergebnis vorhanden")
    }

    fn protokoll(&self, datei: &Path) -> String {
        std::fs::read_to_string(datei).expect("Protokoll lesbar")
    }
}

#[tokio::test]
async fn schreibt_protokoll_und_ergebnis() {
    let u = Umgebung::neu();
    let log_file = u.pfad("lauf.log");
    let record = lauf("ARC-0001-x", u.dir.path(), &log_file);
    u.runs().create(&record).expect("Lauf angelegt");

    u.worker()
        .run("ARC-0001-x", env(&[("FAKE_CLAUDE_MODE", "fail")]))
        .await
        .expect("Lauf ausgeführt");

    let end = u.ergebnis("ARC-0001-x");
    assert_eq!(end.result.code, Some(3));
    assert!(!end.result.timed_out);
    assert!(end.result.output.contains("kaputt"));
    assert!(!end.ended_at.is_empty());
    let log = u.protokoll(&log_file);
    assert!(log.contains("junior-ARC-0001-x startet"), "{log}");
    assert!(log.contains("[dispatch] Profil standard: Modell opus, Effort high, 80 Schritte, 1 min (Standardprofil standard)"), "{log}");
    assert!(log.contains("Arbeitsanweisung"), "{log}");
    assert!(log.contains("Ende: Exit-Code 3"), "{log}");
}

#[tokio::test]
async fn fuehrt_die_einrichtung_vor_claude_nacheinander_im_jeweiligen_ordner_aus() {
    let u = Umgebung::neu();
    let log_file = u.pfad("einrichtung.log");
    std::fs::create_dir(u.pfad("sub")).expect("Unterordner");
    let mut record = lauf("ARC-0005-x", u.dir.path(), &log_file);
    record.prompt = "p".to_string();
    record.setup = Some(vec![
        WorktreeSetupStep {
            cwd: ".".to_string(),
            run: "echo eins>>order.txt".to_string(),
        },
        WorktreeSetupStep {
            cwd: "sub".to_string(),
            run: "echo zwei>>../order.txt".to_string(),
        },
    ]);
    record.setup_timeout_minutes = Some(1);
    u.runs().create(&record).expect("Lauf angelegt");

    u.worker()
        .run("ARC-0005-x", env(&[("FAKE_CLAUDE_MODE", "ok")]))
        .await
        .expect("Lauf ausgeführt");

    let reihenfolge: Vec<String> = u
        .protokoll(&u.pfad("order.txt"))
        .lines()
        .map(|zeile| zeile.trim().to_string())
        .filter(|zeile| !zeile.is_empty())
        .collect();
    assert_eq!(reihenfolge, vec!["eins".to_string(), "zwei".to_string()]);
    let end = u.ergebnis("ARC-0005-x");
    assert_eq!(end.result.code, Some(0));
    assert_eq!(end.result.setup_ok, Some(true));
    assert_eq!(end.result.setup_failed, None);
    assert!(
        end.result.output.contains("prompt=p"),
        "{}",
        end.result.output
    );
    let log = u.protokoll(&log_file);
    assert!(
        log.contains("[dispatch] Einrichtung: .$ echo eins>>order.txt"),
        "{log}"
    );
    assert!(
        log.contains("[dispatch] Einrichtung: sub$ echo zwei>>../order.txt"),
        "{log}"
    );
    assert!(
        log.find("Einrichtung: sub$") < log.find("Ende: Exit-Code 0"),
        "{log}"
    );
}

#[tokio::test]
async fn startet_claude_nicht_wenn_die_einrichtung_scheitert() {
    let u = Umgebung::neu();
    let log_file = u.pfad("kaputt.log");
    let mut record = lauf("ARC-0006-x", u.dir.path(), &log_file);
    record.prompt = "p".to_string();
    record.setup = Some(vec![
        WorktreeSetupStep {
            cwd: ".".to_string(),
            run: "echo kaputt; exit 3".to_string(),
        },
        WorktreeSetupStep {
            cwd: ".".to_string(),
            run: "echo nie>>nie.txt".to_string(),
        },
    ]);
    record.setup_timeout_minutes = Some(1);
    u.runs().create(&record).expect("Lauf angelegt");

    u.worker()
        .run("ARC-0006-x", env(&[("FAKE_CLAUDE_MODE", "ok")]))
        .await
        .expect("Lauf ausgeführt");

    let end = u.ergebnis("ARC-0006-x");
    assert_eq!(end.result.code, Some(3));
    assert_eq!(end.result.setup_ok, None);
    let gescheitert = end.result.setup_failed.expect("gescheiterter Befehl");
    assert_eq!(gescheitert.cwd, ".");
    assert_eq!(gescheitert.run, "echo kaputt; exit 3");
    assert_eq!(gescheitert.code, Some(3));
    assert!(!gescheitert.timed_out);
    assert!(gescheitert.output.contains("kaputt"));
    assert!(
        !end.result.output.contains("prompt="),
        "{}",
        end.result.output
    );
    assert!(!u.pfad("nie.txt").exists(), "der zweite Befehl lief nicht");
    assert!(u
        .protokoll(&log_file)
        .contains("Einrichtung fehlgeschlagen – Claude wird nicht gestartet"));
}

#[tokio::test]
async fn schreibt_stream_json_lesbar_mit_uhrzeit_ins_protokoll() {
    let u = Umgebung::neu();
    let log_file = u.pfad("strom.log");
    let mut record = lauf("ARC-0004-x", u.dir.path(), &log_file);
    record.prompt = "Zeile 1\n\nZeile 3".to_string();
    u.runs().create(&record).expect("Lauf angelegt");

    u.worker()
        .run(
            "ARC-0004-x",
            env(&[
                ("FAKE_CLAUDE_MODE", "stream"),
                ("HARNESS_TICKET", "ARC-0004"),
            ]),
        )
        .await
        .expect("Lauf ausgeführt");

    let log = u.protokoll(&log_file);
    assert!(
        log.contains("[dispatch] Arbeitsanweisung:\nZeile 1\n\nZeile 3\n\n[dispatch] Ausgabe:\n"),
        "{log}"
    );
    assert!(
        log.lines().any(|zeile| beginnt_mit_uhrzeit(zeile)
            && zeile.contains("[start] Claude startet (fake-opus)")),
        "{log}"
    );
    assert!(
        log.lines().any(|zeile| zeile
            .contains("liest Ticket ARC-0004\t\"$ harness board show ARC-0004\"")),
        "{log}"
    );
    assert!(log.contains("] abgelehnt\t\"abgelehnt\""), "{log}");
    assert!(log.contains("[done] fertig, 3 Schritte, 1 min"), "{log}");
    assert!(!log.contains("\"type\""), "{log}");

    let end = u.ergebnis("ARC-0004-x");
    assert_eq!(end.result.code, Some(0));
    assert!(!end.result.timed_out);
    let summary = RunOutputReader::new().summarize(&end.result.output);
    assert_eq!(summary.subtype.as_deref(), Some("success"));
    assert!(!summary.is_error);
    assert_eq!(summary.text, "erledigt");
    assert_eq!(summary.denied, vec!["Bash: git push".to_string()]);
}

#[tokio::test]
async fn beendet_claude_nach_dem_zeitlimit_und_vermerkt_es() {
    let u = Umgebung::neu();
    let log_file = u.pfad("zeitlimit.log");
    let mut record = lauf("ARC-0007-x", u.dir.path(), &log_file);
    // 0 Minuten: das Zeitlimit greift sofort, der Ersatz läuft sonst zehn Minuten
    record.run = einstellungen(0);
    u.runs().create(&record).expect("Lauf angelegt");

    u.worker()
        .run("ARC-0007-x", env(&[("FAKE_CLAUDE_MODE", "sleep")]))
        .await
        .expect("Lauf ausgeführt");

    let end = u.ergebnis("ARC-0007-x");
    assert!(end.result.timed_out);
    let log = u.protokoll(&log_file);
    assert!(
        log.contains("[dispatch] Zeitlimit erreicht – Claude wird beendet"),
        "{log}"
    );
    assert!(log.contains("Ende: Exit-Code"), "{log}");
    assert!(log.contains(", Zeitlimit"), "{log}");
}

#[tokio::test]
async fn liest_einen_kurz_danach_geschriebenen_lauf() {
    let u = Umgebung::neu();
    let log_file = u.pfad("spaet.log");
    let record = lauf("ARC-0002-x", u.dir.path(), &log_file);
    let runs = u.runs();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        runs.create(&record).expect("Lauf angelegt");
    });

    u.worker()
        .run("ARC-0002-x", env(&[("FAKE_CLAUDE_MODE", "ok")]))
        .await
        .expect("Lauf ausgeführt");

    let end = u.ergebnis("ARC-0002-x");
    assert_eq!(end.result.code, Some(0));
    assert!(!end.result.timed_out);
}

#[tokio::test]
async fn schreibt_auch_ohne_lesbaren_lauf_ein_ergebnis_und_prueft_die_lauf_id() {
    let u = Umgebung::neu();

    let fehler = u
        .worker()
        .run("fehlt", env(&[]))
        .await
        .expect_err("Lauf fehlt");
    assert_eq!(fehler.code(), Some("run-not-found"));
    let end = u.ergebnis("fehlt");
    assert_eq!(end.result.code, Some(1));
    assert!(!end.result.timed_out);
    assert!(end.result.output.contains("fehlt"), "{}", end.result.output);

    let fehler = u
        .worker()
        .run("../böse", env(&[]))
        .await
        .expect_err("ungültige ID");
    assert_eq!(fehler.code(), Some("run-id-invalid"));
    assert!(!u.pfad("dispatch/runs/böse.result.json").exists());
}

#[tokio::test]
async fn laeuft_weiter_wenn_das_protokoll_nicht_geschrieben_werden_kann() {
    let u = Umgebung::neu();
    // Ein Ordner statt einer Datei: das Protokoll lässt sich nicht öffnen
    let log_file = u.pfad("ist-ein-ordner");
    std::fs::create_dir(&log_file).expect("Ordner");
    let mut record = lauf("ARC-0003-x", u.dir.path(), &log_file);
    record.prompt = "p".to_string();
    u.runs().create(&record).expect("Lauf angelegt");

    u.worker()
        .run("ARC-0003-x", env(&[("FAKE_CLAUDE_MODE", "ok")]))
        .await
        .expect("Lauf ausgeführt");

    let end = u.ergebnis("ARC-0003-x");
    assert_eq!(end.result.code, Some(0));
    assert!(!end.result.timed_out);
}

#[tokio::test]
async fn der_protokollkopf_nennt_die_art_des_laufs() {
    let u = Umgebung::neu();
    let log_file = u.pfad("merge.log");
    let mut record = lauf("ARC-0008-x", u.dir.path(), &log_file);
    record.kind = RunKind::Merge;
    record.target = Some("main".to_string());
    // Eine Merge-Session richtet nichts ein, selbst wenn Befehle dastehen
    record.setup = Some(vec![WorktreeSetupStep {
        cwd: ".".to_string(),
        run: "echo nie>>nie.txt".to_string(),
    }]);
    u.runs().create(&record).expect("Lauf angelegt");

    u.worker()
        .run("ARC-0008-x", env(&[("FAKE_CLAUDE_MODE", "ok")]))
        .await
        .expect("Lauf ausgeführt");

    assert!(u.protokoll(&log_file).contains("(Merge-Session)"));
    assert!(!u.pfad("nie.txt").exists(), "keine Einrichtung beim Merge");
    assert_eq!(u.ergebnis("ARC-0008-x").result.setup_ok, None);
}

#[tokio::test]
async fn ohne_modell_und_effort_steht_standard_im_kopf() {
    let u = Umgebung::neu();
    let log_file = u.pfad("standard.log");
    let mut record = lauf("ARC-0009-x", u.dir.path(), &log_file);
    record.run.model = None;
    record.run.effort = None;
    u.runs().create(&record).expect("Lauf angelegt");

    u.worker()
        .run("ARC-0009-x", env(&[("FAKE_CLAUDE_MODE", "ok")]))
        .await
        .expect("Lauf ausgeführt");

    assert!(u
        .protokoll(&log_file)
        .contains("Modell Standard, Effort Standard"));
}

#[tokio::test]
async fn die_uhr_bestimmt_die_zeitstempel_des_protokolls() {
    let u = Umgebung::neu();
    let log_file = u.pfad("uhr.log");
    let record = lauf("ARC-0010-x", u.dir.path(), &log_file);
    u.runs().create(&record).expect("Lauf angelegt");
    let fest = "2026-10-29T08:55:00.000Z"
        .parse::<chrono::DateTime<chrono::Utc>>()
        .expect("Zeitpunkt");

    DispatchWorker::with_clock(u.home.clone(), Clock::fixed(fest))
        .with_read_pause(Duration::from_millis(100))
        .run("ARC-0010-x", env(&[("FAKE_CLAUDE_MODE", "ok")]))
        .await
        .expect("Lauf ausgeführt");

    let log = u.protokoll(&log_file);
    assert!(log.contains("[dispatch] 2026-10-29T08:55:00.000Z"), "{log}");
    assert_eq!(
        u.ergebnis("ARC-0010-x").ended_at,
        "2026-10-29T08:55:00.000Z"
    );
}

#[tokio::test]
async fn das_protokoll_gleicht_dem_des_typescript_workers() {
    let u = Umgebung::neu();
    let log_file = u.pfad("strom.log");
    // Genau der Lauf, aus dem das eingecheckte Golden-Protokoll des TypeScript-Workers stammt
    let mut record = lauf("ARC-0004-x", u.dir.path(), &log_file);
    record.agent = "junior-ARC-0004-x".to_string();
    record.prompt = "Zeile 1\n\nZeile 3".to_string();
    u.runs().create(&record).expect("Lauf angelegt");

    u.worker()
        .run(
            "ARC-0004-x",
            env(&[
                ("FAKE_CLAUDE_MODE", "stream"),
                ("HARNESS_TICKET", "ARC-0004"),
            ]),
        )
        .await
        .expect("Lauf ausgeführt");

    let gesehen = normalisiert(
        &u.protokoll(&log_file),
        &u.dir.path().to_string_lossy(),
        &fake_claude(),
    );
    let golden = include_str!("fixtures/run_log_stream.golden.log");
    assert_eq!(gesehen, golden);
}

/// Macht ein Lauf-Protokoll vergleichbar: Pfade und Zeiten raus (wie bei der Aufzeichnung des
/// Golden-Protokolls)
fn normalisiert(log: &str, cwd: &str, claude: &str) -> String {
    let ohne_pfade = log.replace(claude, "<claude>").replace(cwd, "<cwd>");
    let ohne_iso = regex::Regex::new(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z")
        .expect("festes Muster")
        .replace_all(&ohne_pfade, "<iso>")
        .into_owned();
    regex::Regex::new(r"(?m)^\d\d:\d\d:\d\d")
        .expect("festes Muster")
        .replace_all(&ohne_iso, "<zeit>")
        .into_owned()
}

/// Beginnt die Zeile mit einer Uhrzeit `HH:MM:SS`?
fn beginnt_mit_uhrzeit(zeile: &str) -> bool {
    let zeichen: Vec<char> = zeile.chars().take(8).collect();
    zeichen.len() == 8
        && zeichen.iter().enumerate().all(|(stelle, zeichen)| {
            if stelle == 2 || stelle == 5 {
                *zeichen == ':'
            } else {
                zeichen.is_ascii_digit()
            }
        })
}

/// „Claude“, das im Ordner `cwd` einen losgelösten Dauerläufer (eigene Sitzung) startet, dessen
/// PID in `pid_file` schreibt und endet
#[cfg(unix)]
fn hinterlaesst_waise(record: &mut RunRecord, cwd: &Path, pid_file: &Path) {
    record.command = "/usr/bin/perl".to_string();
    record.args = vec![
        "-e".to_string(),
        "use POSIX qw(setsid); chdir $ARGV[0] or die; defined(my $p = fork) or die; \
         if ($p == 0) { setsid(); open STDIN, '<', '/dev/null'; open STDOUT, '>', '/dev/null'; \
         open STDERR, '>', '/dev/null'; exec '/bin/sleep', '300' } \
         open my $f, '>', $ARGV[1] or die; print $f $p; close $f"
            .to_string(),
        cwd.to_string_lossy().into_owned(),
        pid_file.to_string_lossy().into_owned(),
    ];
}

/// Läuft der Prozess noch?
#[cfg(unix)]
fn laeuft(pid: u32) -> bool {
    nexus_core::process::is_alive(pid)
}

/// Wartet höchstens fünf Sekunden, bis der Prozess beendet ist
#[cfg(unix)]
async fn beendet(pid: u32) -> bool {
    for _ in 0..50 {
        if !laeuft(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[cfg(unix)]
#[tokio::test]
async fn beendet_nach_dem_lauf_was_im_worktree_verwaist_uebrig_bleibt() {
    let u = Umgebung::neu();
    let worktree = u.pfad("wt");
    std::fs::create_dir(&worktree).expect("Worktree");
    std::fs::write(worktree.join(".git"), "gitdir: /x/.git/worktrees/wt\n").expect(".git");
    let pid_file = u.pfad("waise.pid");
    let log_file = u.pfad("lauf.log");
    let mut record = lauf("ARC-0001-x", &worktree, &log_file);
    hinterlaesst_waise(&mut record, &worktree, &pid_file);
    u.runs().create(&record).expect("Lauf angelegt");

    u.worker()
        .run("ARC-0001-x", env(&[]))
        .await
        .expect("Lauf ausgeführt");

    let waise: u32 = std::fs::read_to_string(&pid_file)
        .expect("PID-Datei")
        .trim()
        .parse()
        .expect("PID");
    assert!(beendet(waise).await, "Waise {waise} läuft noch");
    let log = u.protokoll(&log_file);
    assert!(
        log.contains(&format!(
            "Aufgeräumt: 1 übrig gebliebene Prozesse beendet (PID {waise})"
        )),
        "{log}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn raeumt_im_hauptordner_nichts_ab() {
    let u = Umgebung::neu();
    let pid_file = u.pfad("waise.pid");
    let mut record = lauf("ARC-0001-x", u.dir.path(), &u.pfad("lauf.log"));
    record.kind = RunKind::Merge;
    hinterlaesst_waise(&mut record, u.dir.path(), &pid_file);
    u.runs().create(&record).expect("Lauf angelegt");

    u.worker()
        .run("ARC-0001-x", env(&[]))
        .await
        .expect("Lauf ausgeführt");

    let waise: u32 = std::fs::read_to_string(&pid_file)
        .expect("PID-Datei")
        .trim()
        .parse()
        .expect("PID");
    let lebt = laeuft(waise);
    nexus_core::process::terminate(waise);
    assert!(lebt, "die Waise im Hauptordner wurde beendet");
}
