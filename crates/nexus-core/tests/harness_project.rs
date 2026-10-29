//! Portiert `packages/core/tests/unit/project/HarnessProject.test.ts` und prüft die
//! Session-Nummern, die in TypeScript bisher keinen eigenen Test haben.

mod common;

use std::fs;

use nexus_core::counter::CounterKind;
use nexus_core::project::HarnessProject;

use common::TempRepo;

#[test]
fn legt_zahler_und_token_an_aber_keinen_ticket_ordner() {
    let repo = TempRepo::new();

    let project = HarnessProject::init(repo.path()).unwrap();

    assert!(!project.layout.dir().join("tickets").exists());
    assert_eq!(
        fs::read_to_string(project.layout.counters_file()).unwrap(),
        r#"{"session":0}"#
    );
    let token = fs::read_to_string(project.layout.token_file()).unwrap();
    assert_eq!(token.len(), 64);
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()), "{token}");
}

#[test]
fn liegt_im_gemeinsamen_git_verzeichnis() {
    let repo = TempRepo::new();

    let project = HarnessProject::init(repo.path()).unwrap();

    let dir = project.layout.dir().to_string_lossy().replace('\\', "/");
    assert!(dir.contains("/.git/harness"), "{dir}");
}

#[test]
fn andert_bei_zweitem_init_weder_token_noch_zahler() {
    let repo = TempRepo::new();
    let first = HarnessProject::init(repo.path()).unwrap();
    first.counter().next(CounterKind::Session).unwrap();
    let token = fs::read_to_string(first.layout.token_file()).unwrap();

    let second = HarnessProject::init(repo.path()).unwrap();

    assert_eq!(
        fs::read_to_string(second.layout.token_file()).unwrap(),
        token
    );
    assert_eq!(
        second.counter().read().unwrap().get("session"),
        Some(&1),
        "{:?}",
        second.counter().read().unwrap()
    );
}

#[test]
fn lasst_alte_zahlerstande_des_datei_boards_und_des_spec_kits_in_der_datei_stehen() {
    let repo = TempRepo::new();
    let project = HarnessProject::init(repo.path()).unwrap();
    fs::write(
        project.layout.counters_file(),
        r#"{"ticket":42,"spec":3,"question":7}"#,
    )
    .unwrap();

    assert_eq!(project.counter().next(CounterKind::Session).unwrap(), 1);

    let state: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(project.layout.counters_file()).unwrap()).unwrap();
    assert_eq!(
        state,
        serde_json::json!({ "ticket": 42, "spec": 3, "question": 7, "session": 1 })
    );
}

#[test]
fn open_ohne_init_wirft_einen_hinweis_auf_harness_init() {
    let repo = TempRepo::new();

    let error = HarnessProject::open(repo.path()).unwrap_err();

    assert_eq!(error.code(), Some("not-initialized"));
    assert!(error.to_string().contains("nexus init"), "{error}");
}

#[test]
fn nutzt_aus_einem_worktree_dasselbe_verzeichnis() {
    let repo = TempRepo::new();
    let main = HarnessProject::init(repo.path()).unwrap();

    let from_worktree = HarnessProject::open(&repo.add_worktree("wt")).unwrap();

    assert_eq!(from_worktree.layout.dir(), main.layout.dir());
}

#[test]
fn meldet_einen_beschadigten_zahler_mit_pfad_statt_neu_zu_beginnen() {
    let repo = TempRepo::new();
    let project = HarnessProject::init(repo.path()).unwrap();
    fs::write(project.layout.counters_file(), "x").unwrap();

    let reopened = HarnessProject::open(repo.path()).unwrap();
    let error = reopened.counter().next(CounterKind::Session).unwrap_err();

    assert_eq!(error.code(), Some("counter-corrupt"));
    assert!(error.to_string().contains("beschädigt"), "{error}");
    assert!(error.to_string().contains("counters.json"), "{error}");
}

#[test]
fn vergibt_jeder_session_eine_nummer_und_behalt_sie() {
    let repo = TempRepo::new();
    let project = HarnessProject::init(repo.path()).unwrap();
    let numbers = project.session_numbers();

    assert_eq!(numbers.assign("se-1").unwrap(), 1);
    assert_eq!(numbers.assign("se-2").unwrap(), 2);
    assert_eq!(numbers.assign("se-1").unwrap(), 1);

    // Dieselbe Nummer auch nach einem Neustart: die Zuordnung steht in der Datei
    let wieder = HarnessProject::open(repo.path()).unwrap();
    assert_eq!(wieder.session_numbers().assign("se-2").unwrap(), 2);
    assert_eq!(wieder.counter().read().unwrap().get("session"), Some(&2));
}

#[test]
fn vergibt_bei_gleichzeitigen_sessions_keine_nummer_doppelt() {
    let repo = TempRepo::new();
    let project = HarnessProject::init(repo.path()).unwrap();

    let pfad = repo.path().to_path_buf();
    let mut numbers: Vec<i64> = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..4)
            .map(|index| {
                let pfad = pfad.clone();
                scope.spawn(move || {
                    let project = HarnessProject::open(&pfad).unwrap();
                    project
                        .session_numbers()
                        .assign(&format!("se-{index}"))
                        .unwrap()
                })
            })
            .collect();
        threads.into_iter().map(|t| t.join().unwrap()).collect()
    });

    numbers.sort_unstable();
    assert_eq!(numbers, [1, 2, 3, 4]);
    assert_eq!(project.counter().read().unwrap().get("session"), Some(&4));
}
