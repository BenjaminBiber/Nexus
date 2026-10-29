//! Die Rules des Projekts in einem echten Git-Repo: anlegen, wiederholen, aus Git herausnehmen,
//! im Worktree und die Ablösung der alten Rules des Harness. Portiert aus
//! `packages/core/tests/unit/install/ProjectDocsRule.test.ts`, `ProjectBoardRule.test.ts` und
//! `ProjectRules.test.ts`.

mod common;

use std::fs;
use std::path::Path;

use nexus_core::install::{
    ProjectRule, ProjectRules, LEGACY_BOARD_RULE_PATH, LEGACY_DOCS_RULE_PATH,
};
use nexus_core::HarnessHome;

use common::TempRepo;

/// Benutzerordner für die Tests (eigener Pfad, damit das Importziel absolut ist)
fn home() -> HarnessHome {
    HarnessHome::at(Path::new("/x").join(".nexus"))
}

/// Inhalt von `info/exclude` des Repos
fn exclude(repo: &TempRepo) -> String {
    let common_dir = repo.git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]);
    fs::read_to_string(Path::new(&common_dir).join("info").join("exclude")).unwrap()
}

/// Schreibt eine alte Rule des Harness ins Projekt
fn alte_rule(repo: &TempRepo, path: &str, text: &str) -> std::path::PathBuf {
    let mut file = repo.path().to_path_buf();
    for part in path.split('/') {
        file.push(part);
    }
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, text).unwrap();
    file
}

#[test]
fn legt_die_doku_rule_mit_eingrenzung_auf_docs_an() {
    let repo = TempRepo::new();
    let rule = ProjectRule::docs(home());

    assert!(rule.ensure(repo.path()).unwrap());

    assert_eq!(
        rule.file(repo.path()),
        repo.path()
            .join(".claude")
            .join("rules")
            .join("nexus-docs.md")
    );
    let text = fs::read_to_string(rule.file(repo.path())).unwrap();
    assert!(text.starts_with("---\n"), "{text}");
    assert!(text.contains("paths:\n  - \"**/docs/**\"\n"), "{text}");
    assert!(
        text.ends_with("\n---\n@/x/.nexus/rules/docs.md\n"),
        "{text}"
    );
}

#[test]
fn ist_wiederholbar_und_meldet_false_wenn_die_rule_schon_aktuell_ist() {
    let repo = TempRepo::new();
    let rule = ProjectRule::docs(home());

    rule.ensure(repo.path()).unwrap();

    assert!(!rule.ensure(repo.path()).unwrap());
}

#[test]
fn ersetzt_eine_veraltete_rule() {
    let repo = TempRepo::new();
    let rule = ProjectRule::docs(home());
    rule.ensure(repo.path()).unwrap();
    fs::write(rule.file(repo.path()), "alt").unwrap();

    assert!(rule.ensure(repo.path()).unwrap());
    assert!(fs::read_to_string(rule.file(repo.path()))
        .unwrap()
        .contains("@/x/.nexus/rules/docs.md"));
}

#[test]
fn nimmt_die_rule_aus_git_heraus_ohne_eine_versionierte_datei_zu_andern() {
    let repo = TempRepo::new();
    let rule = ProjectRule::docs(home());

    rule.ensure(repo.path()).unwrap();

    assert_eq!(
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        ""
    );
    assert!(exclude(&repo).contains("/.claude/rules/nexus-docs.md"));
}

#[test]
fn gilt_auch_in_einem_worktree_der_ausschluss_liegt_im_gemeinsamen_git_verzeichnis() {
    let repo = TempRepo::new();
    let worktree = repo.add_worktree("wt");
    let rule = ProjectRule::docs(home());

    assert!(rule.ensure(&worktree).unwrap());

    assert!(rule.file(&worktree).exists());
    assert_eq!(
        repo.git(&[
            "-C",
            &worktree.to_string_lossy(),
            "status",
            "--porcelain",
            "--untracked-files=all"
        ]),
        ""
    );
}

#[test]
fn legt_die_board_rule_ohne_eingrenzung_an_und_nimmt_sie_aus_git_heraus() {
    let repo = TempRepo::new();
    let rule = ProjectRule::board(home());

    assert!(rule.ensure(repo.path()).unwrap());

    assert_eq!(
        rule.file(repo.path()),
        repo.path()
            .join(".claude")
            .join("rules")
            .join("nexus-board.md")
    );
    let text = fs::read_to_string(rule.file(repo.path())).unwrap();
    assert!(!text.contains("paths:"), "{text}");
    assert!(text.ends_with("---\n@/x/.nexus/rules/board.md\n"), "{text}");
    assert!(!rule.ensure(repo.path()).unwrap());
    assert_eq!(
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        ""
    );
    assert!(exclude(&repo).contains("/.claude/rules/nexus-board.md"));
}

#[test]
fn schreibt_beide_rules_und_meldet_bei_wiederholung_nichts_neues() {
    let repo = TempRepo::new();
    let rules = ProjectRules::new(home());

    assert_eq!(
        rules.ensure(repo.path()).unwrap().written,
        [
            ".claude/rules/nexus-docs.md".to_string(),
            ".claude/rules/nexus-board.md".to_string()
        ]
    );
    assert_eq!(
        rules.ensure(repo.path()).unwrap().written,
        Vec::<String>::new()
    );
    assert_eq!(
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        ""
    );
}

#[test]
fn erganzt_in_einem_schon_eingerichteten_projekt_nur_die_fehlende_board_rule() {
    let repo = TempRepo::new();
    let rules = ProjectRules::new(home());
    rules.ensure(repo.path()).unwrap();
    fs::remove_file(rules.board.file(repo.path())).unwrap();

    assert_eq!(
        rules.ensure(repo.path()).unwrap().written,
        [".claude/rules/nexus-board.md".to_string()]
    );
    assert!(rules.board.file(repo.path()).exists());
}

#[test]
fn lost_die_alten_rules_des_harness_ab_und_lasst_das_repo_sauber() {
    let repo = TempRepo::new();
    let rules = ProjectRules::new(home());
    let docs = alte_rule(
        &repo,
        LEGACY_DOCS_RULE_PATH,
        "---\n# Angelegt von `harness init`, nicht versioniert.\npaths:\n  - \"**/docs/**\"\n---\n@~/.harness/rules/docs.md\n",
    );
    let board = alte_rule(
        &repo,
        LEGACY_BOARD_RULE_PATH,
        "---\n# Angelegt von `harness init`, nicht versioniert.\n---\n@~/.harness/rules/board.md\n",
    );

    let result = rules.ensure(repo.path()).unwrap();

    assert!(!docs.exists());
    assert!(!board.exists());
    assert_eq!(
        result.removed,
        [
            LEGACY_DOCS_RULE_PATH.to_string(),
            LEGACY_BOARD_RULE_PATH.to_string()
        ]
    );
    assert!(result.kept.is_empty());
    assert_eq!(
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        ""
    );
}

#[test]
fn lasst_eine_von_hand_geschriebene_alte_rule_liegen_und_meldet_sie() {
    let repo = TempRepo::new();
    let rules = ProjectRules::new(home());
    let eigen = alte_rule(&repo, LEGACY_BOARD_RULE_PATH, "# Meine eigene Regel\n");

    let result = rules.ensure(repo.path()).unwrap();

    assert!(eigen.exists());
    assert_eq!(result.kept, [LEGACY_BOARD_RULE_PATH.to_string()]);
    assert!(result.removed.is_empty());
}
