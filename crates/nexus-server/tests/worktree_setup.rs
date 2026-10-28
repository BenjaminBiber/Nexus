//! Tests der Einrichtung eines Worktrees: portiert aus den Einrichtungs-Fällen in
//! `packages/server/tests/unit/dispatch/DispatchWorker.test.ts`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nexus_core::dispatch::GitBashLocator;
use nexus_server::dispatch::{SetupInput, SetupOutcome, WorktreeSetupRunner, WorktreeSetupStep};

/// Umgebung aus Paaren
fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// Ein Schritt
fn step(cwd: &str, run: &str) -> WorktreeSetupStep {
    WorktreeSetupStep {
        cwd: cwd.to_string(),
        run: run.to_string(),
    }
}

/// Führt die Einrichtung aus und liefert Ergebnis und Protokoll
async fn fuehre_aus(input: SetupInput) -> (SetupOutcome, String) {
    let protokoll = Arc::new(Mutex::new(String::new()));
    let mitschrift = Arc::clone(&protokoll);
    let outcome = WorktreeSetupRunner::new()
        .run(input, move |text| {
            mitschrift.lock().expect("Protokoll").push_str(text);
        })
        .await;
    let protokoll = protokoll.lock().expect("Protokoll").clone();
    (outcome, protokoll)
}

#[tokio::test]
async fn fuehrt_die_befehle_nacheinander_im_jeweiligen_ordner_aus_und_protokolliert_sie() {
    let dir = tempfile::tempdir().expect("Ordner");
    std::fs::create_dir(dir.path().join("sub")).expect("Unterordner");
    let input = SetupInput {
        steps: vec![
            step(".", "echo eins>>order.txt"),
            step("sub", "echo zwei>>../order.txt"),
        ],
        root: dir.path().to_path_buf(),
        env: env(&[]),
        timeout: Duration::from_secs(60),
    };
    let (outcome, protokoll) = fuehre_aus(input).await;
    assert_eq!(outcome, SetupOutcome::Ok);
    let order = std::fs::read_to_string(dir.path().join("order.txt")).expect("order.txt");
    let zeilen: Vec<&str> = order
        .lines()
        .map(str::trim)
        .filter(|zeile| !zeile.is_empty())
        .collect();
    assert_eq!(zeilen, ["eins", "zwei"]);
    assert!(protokoll.contains("[dispatch] Einrichtung: .$ echo eins>>order.txt"));
    assert!(protokoll.contains("[dispatch] Einrichtung: sub$ echo zwei>>../order.txt"));
    assert!(protokoll.contains("[dispatch] Einrichtung beendet: Exit-Code 0"));
}

#[tokio::test]
async fn haelt_beim_ersten_fehlschlag_an_und_nennt_den_befehl() {
    let dir = tempfile::tempdir().expect("Ordner");
    let input = SetupInput {
        steps: vec![
            step(".", "echo kaputt&& exit 3"),
            step(".", "echo nie>>nie.txt"),
        ],
        root: dir.path().to_path_buf(),
        env: env(&[]),
        timeout: Duration::from_secs(60),
    };
    let (outcome, _) = fuehre_aus(input).await;
    let SetupOutcome::Failed(failure) = outcome else {
        panic!("die Einrichtung muss scheitern");
    };
    assert_eq!(failure.cwd, ".");
    assert_eq!(failure.run, "echo kaputt&& exit 3");
    assert_eq!(failure.code, Some(3));
    assert!(!failure.timed_out);
    assert!(failure.output.contains("kaputt"));
    assert!(
        !dir.path().join("nie.txt").exists(),
        "der zweite Befehl läuft nicht mehr"
    );
}

#[tokio::test]
async fn ein_ordner_ausserhalb_des_worktrees_scheitert_ohne_start() {
    let dir = tempfile::tempdir().expect("Ordner");
    let input = SetupInput {
        steps: vec![step("../daneben", "echo nie>>nie.txt")],
        root: dir.path().to_path_buf(),
        env: env(&[]),
        timeout: Duration::from_secs(60),
    };
    let (outcome, protokoll) = fuehre_aus(input).await;
    let SetupOutcome::Failed(failure) = outcome else {
        panic!("die Einrichtung muss scheitern");
    };
    assert_eq!(failure.code, None);
    assert_eq!(
        failure.output,
        "Ordner ../daneben gibt es im Worktree nicht."
    );
    assert!(protokoll.contains("[dispatch] Ordner ../daneben gibt es im Worktree nicht."));
}

#[tokio::test]
async fn ein_fehlender_ordner_scheitert_ohne_start() {
    let dir = tempfile::tempdir().expect("Ordner");
    let input = SetupInput {
        steps: vec![step("fehlt", "echo nie")],
        root: dir.path().to_path_buf(),
        env: env(&[]),
        timeout: Duration::from_secs(60),
    };
    let (outcome, _) = fuehre_aus(input).await;
    let SetupOutcome::Failed(failure) = outcome else {
        panic!("die Einrichtung muss scheitern");
    };
    assert_eq!(failure.output, "Ordner fehlt gibt es im Worktree nicht.");
}

#[tokio::test]
async fn beendet_einen_befehl_nach_dem_zeitlimit() {
    let dir = tempfile::tempdir().expect("Ordner");
    // Der Test-Ersatz läuft im Modus sleep, bis er beendet wird
    let fake = env!("CARGO_BIN_EXE_nexus-fake-claude");
    let befehl = if cfg!(windows) {
        format!("\"{fake}\"")
    } else {
        format!("'{fake}'")
    };
    let input = SetupInput {
        steps: vec![step(".", &befehl)],
        root: dir.path().to_path_buf(),
        env: env(&[("FAKE_CLAUDE_MODE", "sleep")]),
        timeout: Duration::from_secs(1),
    };
    let (outcome, protokoll) = fuehre_aus(input).await;
    let SetupOutcome::Failed(failure) = outcome else {
        panic!("die Einrichtung muss scheitern");
    };
    assert!(failure.timed_out);
    assert!(
        protokoll.contains("[dispatch] Zeitlimit der Einrichtung erreicht – Befehl wird beendet")
    );
    assert!(protokoll.contains("[dispatch] Einrichtung beendet: Exit-Code"));
}

/// Nur unter Windows und nur, wenn eine Git Bash installiert ist
#[tokio::test]
#[cfg(windows)]
async fn laeuft_unter_windows_in_der_git_bash_wenn_sie_gefunden_wurde() {
    use nexus_core::dispatch::ProcessEnv;

    let Some(bash) = GitBashLocator::new().locate(&ProcessEnv, "win32") else {
        eprintln!("keine Git Bash gefunden – Test übersprungen");
        return;
    };
    let dir = tempfile::tempdir().expect("Ordner");
    let input = SetupInput {
        steps: vec![step(".", "echo \"$BASH_VERSION\" > shell.txt")],
        root: dir.path().to_path_buf(),
        env: env(&[("CLAUDE_CODE_GIT_BASH_PATH", &bash)]),
        timeout: Duration::from_secs(20),
    };
    let (outcome, _) = fuehre_aus(input).await;
    assert_eq!(outcome, SetupOutcome::Ok);
    let shell = std::fs::read_to_string(dir.path().join("shell.txt")).expect("shell.txt");
    assert!(
        shell
            .trim()
            .starts_with(|zeichen: char| zeichen.is_ascii_digit()),
        "Version der Git Bash: {shell}"
    );
}

/// Ohne Windows gibt es keine Git Bash – der Sucher darf trotzdem gefragt werden
#[tokio::test]
#[cfg(not(windows))]
async fn ohne_windows_gibt_es_keine_git_bash() {
    use nexus_core::dispatch::ProcessEnv;

    assert_eq!(GitBashLocator::new().locate(&ProcessEnv, "darwin"), None);
    assert_eq!(GitBashLocator::new().locate(&ProcessEnv, "linux"), None);
}
