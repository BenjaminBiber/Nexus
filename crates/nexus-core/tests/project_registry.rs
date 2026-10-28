//! Portiert `packages/core/tests/unit/registry/ProjectRegistry.test.ts`.

mod common;

use std::fs;

use nexus_core::project::HarnessProject;
use nexus_core::registry::{HarnessHome, ProjectRegistry};

use common::{canonical, make_temp_dir, TempRepo};

/// Benutzerordner und ein eingerichtetes Projekt
fn setup() -> (tempfile::TempDir, TempRepo, ProjectRegistry) {
    let home = make_temp_dir("harness-home-");
    let repo = TempRepo::new();
    HarnessProject::init(repo.path()).unwrap();
    let registry = ProjectRegistry::new(HarnessHome::at(canonical(home.path())));
    (home, repo, registry)
}

#[test]
fn meldet_ein_projekt_an_mit_stabiler_id_und_ordnername() {
    let (_home, repo, registry) = setup();

    let added = registry.add(repo.path()).unwrap();

    assert!(added.created);
    assert_eq!(added.project.id.len(), 8);
    assert!(added.project.id.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(added.project.name, "Mein Projekt ä");
    assert_eq!(
        registry.add(repo.path()).unwrap().project.id,
        added.project.id
    );
}

#[test]
fn meldet_dasselbe_projekt_aus_einem_worktree_unter_derselben_id_an() {
    let (_home, repo, registry) = setup();
    let first = registry.add(repo.path()).unwrap();

    let second = registry.add(&repo.add_worktree("wt")).unwrap();

    assert!(!second.created);
    assert_eq!(second.project.id, first.project.id);
    assert_eq!(registry.list().unwrap().len(), 1);
}

#[test]
fn findet_ein_angemeldetes_projekt_uber_seine_id() {
    let (_home, repo, registry) = setup();
    let added = registry.add(repo.path()).unwrap();

    assert_eq!(
        registry.find(&added.project.id).unwrap(),
        Some(added.project)
    );
    assert_eq!(registry.find("00000000").unwrap(), None);
}

#[test]
fn lehnt_einen_ordner_ohne_git_oder_ohne_harness_init_ab() {
    let (_home, _repo, registry) = setup();

    let plain = make_temp_dir("harness-plain-");
    let error = registry.add(&canonical(plain.path())).unwrap_err();
    assert_eq!(error.code(), Some("not-a-git-project"));

    let bare = TempRepo::new();
    let error = registry.add(bare.path()).unwrap_err();
    assert_eq!(error.code(), Some("not-initialized"));
    assert!(error.to_string().contains("harness init"), "{error}");
}

#[test]
fn verliert_bei_gleichzeitigen_anmeldungen_nichts() {
    let (_home, _repo, registry) = setup();
    let others: Vec<TempRepo> = (0..4)
        .map(|_| {
            let repo = TempRepo::new();
            HarnessProject::init(repo.path()).unwrap();
            repo
        })
        .collect();

    std::thread::scope(|scope| {
        for repo in &others {
            scope.spawn(|| {
                registry.add(repo.path()).unwrap();
            });
        }
    });

    assert_eq!(registry.list().unwrap().len(), 4);
}

#[test]
fn markiert_projekte_mit_fehlendem_ordner_als_nicht_erreichbar_ohne_sie_zu_loschen() {
    let (_home, repo, registry) = setup();
    registry.add(repo.path()).unwrap();

    fs::remove_dir_all(repo.base()).unwrap();

    let state = registry.list_with_state().unwrap();
    assert_eq!(state.len(), 1);
    assert!(!state[0].reachable);
    assert_eq!(registry.list().unwrap().len(), 1);
}

#[test]
fn schreibt_projects_json_mit_zwei_leerzeichen_wie_typescript() {
    let (home, repo, registry) = setup();
    registry.add(repo.path()).unwrap();

    let text = fs::read_to_string(home.path().join("projects.json")).unwrap();

    assert!(text.starts_with("[\n  {\n    \"id\": "), "{text}");
    assert!(text.ends_with(']'), "{text}");
}
