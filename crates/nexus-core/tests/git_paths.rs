//! Portiert `packages/core/tests/unit/git/{GitPaths,GitRunner,GitExclude}.test.ts`,
//! soweit dafür ein echtes Git-Projekt nötig ist.

mod common;

use std::fs;

use nexus_core::git::{GitCommands, GitPaths, GitRunner};

use common::{canonical, make_temp_dir, slashes, TempRepo};

/// Git-Projekt mit Leerzeichen und Umlaut im Ordnernamen
fn repo() -> TempRepo {
    TempRepo::new()
}

#[test]
fn liefert_das_gemeinsame_git_verzeichnis_im_hauptordner() {
    let repo = repo();

    let common_dir = GitPaths::new().common_dir(repo.path()).unwrap();

    assert_eq!(common_dir, slashes(&repo.path().join(".git")));
}

#[test]
fn liefert_aus_einem_worktree_dasselbe_gemeinsame_verzeichnis() {
    let repo = repo();
    let worktree = repo.add_worktree("wt");

    assert_eq!(
        GitPaths::new().common_dir(&worktree).unwrap(),
        GitPaths::new().common_dir(repo.path()).unwrap()
    );
}

#[test]
fn liefert_die_wurzel_des_hauptordners_auch_aus_einem_worktree() {
    let repo = repo();
    let worktree = repo.add_worktree("wt");

    assert_eq!(
        GitPaths::new().main_root(&worktree).unwrap(),
        slashes(repo.path())
    );
}

#[test]
fn wirft_einen_verstandlichen_fehler_ausserhalb_eines_git_projekts() {
    let base = make_temp_dir("harness-git-");
    let plain = canonical(base.path()).join("kein-git");
    fs::create_dir(&plain).unwrap();

    let error = GitPaths::new().common_dir(&plain).unwrap_err();

    assert_eq!(error.code(), Some("not-a-git-project"));
    assert!(error.to_string().contains("kein Git-Projekt"), "{error}");
}

#[test]
fn liefert_die_wurzel_des_arbeitsordners() {
    let repo = repo();
    let worktree = repo.add_worktree("wt");

    assert_eq!(
        GitPaths::new().top_level(repo.path()).unwrap(),
        slashes(repo.path())
    );
    assert_eq!(
        GitPaths::new().top_level(&worktree).unwrap(),
        slashes(&worktree)
    );
}

#[test]
fn try_run_fullt_die_fehlermeldung_auch_wenn_git_nichts_auf_stderr_schreibt() {
    let repo = repo();
    repo.git(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "--allow-empty",
        "-q",
        "-m",
        "zwei",
    ]);

    // HEAD ist kein Vorfahr von HEAD~1: Rückgabecode 1 ohne Fehlerausgabe
    let attempt = GitRunner::new().try_run(
        repo.path(),
        &["merge-base", "--is-ancestor", "HEAD", "HEAD~1"],
    );

    assert!(!attempt.ok);
    assert_ne!(attempt.err, "");
}

#[test]
fn run_liefert_die_getrimmte_ausgabe_und_raw_die_rohe() {
    let repo = repo();
    let git = GitRunner::new();

    assert_eq!(
        git.run(repo.path(), &["status", "--porcelain"]).unwrap(),
        ""
    );
    assert!(!git
        .run(repo.path(), &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap()
        .is_empty());
    assert!(git
        .raw(repo.path(), &["log", "--oneline"])
        .unwrap()
        .ends_with('\n'));
}

#[test]
fn meldet_einen_gescheiterten_git_befehl_mit_code_und_meldung() {
    let repo = repo();

    let error = GitRunner::new()
        .run(repo.path(), &["rev-parse", "--verify", "kein-zweig"])
        .unwrap_err();

    assert_eq!(error.code(), Some("git-failed"));
    assert!(
        error.to_string().contains("Git-Befehl fehlgeschlagen"),
        "{error}"
    );
}
