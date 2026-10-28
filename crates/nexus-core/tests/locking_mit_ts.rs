//! Gegenprobe zur Sperre: ein Node-Prozess mit der TypeScript-`FileLock` und ein
//! Rust-Prozess mit `nexus_core::locking::FileLock` schließen sich an derselben
//! Sperrdatei gegenseitig aus.
//!
//! Der Test braucht `node` und das gebaute `packages/core/dist`
//! (`npx tsc -b packages/core`). Fehlt eines davon, wird er mit einem Hinweis
//! übersprungen statt zu scheitern.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use nexus_core::error::Result;
use nexus_core::locking::{FileLock, FileLockOptions};

use common::make_temp_dir;

/// Wartezeit, in der die Gegenseite ihren Schritt getan haben muss
const WARTEZEIT: Duration = Duration::from_secs(30);

/// Die Sperre der Gegenseite darf nie als verwaist gelten, solange sie gehalten wird
const OPTIONS: FileLockOptions = FileLockOptions {
    timeout_ms: 300,
    stale_ms: 60_000,
};

#[test]
fn node_und_rust_schliessen_sich_an_derselben_sperrdatei_aus() {
    let Some(ts_file_lock) = gebaute_file_lock() else {
        println!(
            "übersprungen: node oder packages/core/dist fehlt – für die Gegenprobe zuerst `npx tsc -b packages/core` ausführen"
        );
        return;
    };
    let dir = make_temp_dir("harness-lock-ts-");
    let lock_path = dir.path().join("gemeinsam.lock");
    let ready = dir.path().join("bereit");
    let release = dir.path().join("freigabe");

    // 1. Node hält die Sperre – Rust darf sie nicht bekommen
    let mut node = node_starten(&[
        "halten",
        &ts_file_lock,
        &text(&lock_path),
        &text(&ready),
        &text(&release),
    ]);
    warten_auf(&ready, &mut node);
    let rust_lock = FileLock::new(&lock_path, OPTIONS);
    let error = rust_lock.with_lock(|| Ok(())).unwrap_err();
    assert_eq!(error.code(), Some("lock-timeout"), "{error}");
    assert!(lock_path.exists(), "Node hält die Sperrdatei");

    // 2. Nach der Freigabe bekommt Rust sie
    std::fs::write(&release, "los").unwrap();
    let ende = node.wait().unwrap();
    assert!(ende.success(), "Node endete mit {ende}");
    let bekommen = FileLock::new(&lock_path, FileLockOptions::default())
        .with_lock(|| Ok(42))
        .unwrap();
    assert_eq!(bekommen, 42);
    assert!(!lock_path.exists(), "Rust gibt die Sperrdatei wieder frei");

    // 3. Umgekehrt: Rust hält die Sperre – Node darf sie nicht bekommen
    let gehalten: Result<()> =
        FileLock::new(&lock_path, FileLockOptions::default()).with_lock(|| {
            let versuch = node_starten(&["versuchen", &ts_file_lock, &text(&lock_path)])
                .wait_with_output()
                .unwrap();
            assert_eq!(
                String::from_utf8_lossy(&versuch.stdout).trim(),
                "belegt",
                "Node hätte die Sperre nicht bekommen dürfen"
            );
            assert_eq!(versuch.status.code(), Some(3));
            Ok(())
        });
    gehalten.unwrap();

    // 4. Und danach bekommt Node sie wieder
    let frei = node_starten(&["versuchen", &ts_file_lock, &text(&lock_path)])
        .wait_with_output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&frei.stdout).trim(), "bekommen");
    assert!(frei.status.success());
}

/// Pfad der gebauten TypeScript-`FileLock`, falls es sie und `node` gibt
fn gebaute_file_lock() -> Option<String> {
    let datei = repo_root().join("packages/core/dist/locking/FileLock.js");
    let node_da = Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    (datei.is_file() && node_da).then(|| text(&datei))
}

/// Wurzel des Arbeitsbereichs (zwei Ebenen über `crates/nexus-core`)
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/nexus-core liegt im Arbeitsbereich")
        .to_path_buf()
}

/// Startet das Gegenstück in Node
fn node_starten(args: &[&str]) -> Child {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/file_lock_ts.mjs");
    Command::new("node")
        .arg(script)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("node startet")
}

/// Wartet, bis die Datei auftaucht; endet der Node-Prozess vorher, scheitert der Test
fn warten_auf(datei: &Path, node: &mut Child) {
    let frist = Instant::now() + WARTEZEIT;
    while !datei.exists() {
        if let Some(ende) = node.try_wait().unwrap() {
            panic!("Node endete vorzeitig mit {ende}");
        }
        assert!(Instant::now() < frist, "Node meldet sich nicht: {datei:?}");
        sleep(Duration::from_millis(20));
    }
}

/// Pfad als Text für die Übergabe an Node
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
