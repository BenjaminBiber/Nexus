//! Die Rules des Projekts in einem echten Git-Repo: anlegen, wiederholen, aus Git herausnehmen
//! und im Worktree. Portiert aus `packages/core/tests/unit/install/ProjectDocsRule.test.ts`,
//! `ProjectBoardRule.test.ts` und `ProjectRules.test.ts`.

mod common;

use std::fs;
use std::path::Path;

use nexus_core::install::{ProjectRule, ProjectRules};
use nexus_core::HarnessHome;

use common::TempRepo;

/// Benutzerordner für die Tests (eigener Pfad, damit das Importziel absolut ist)
fn home() -> HarnessHome {
    HarnessHome::at(Path::new("/x").join(".harness"))
}

/// Inhalt von `info/exclude` des Repos
fn exclude(repo: &TempRepo) -> String {
    let common_dir = repo.git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]);
    fs::read_to_string(Path::new(&common_dir).join("info").join("exclude")).unwrap()
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
            .join("ai-harness-docs.md")
    );
    let text = fs::read_to_string(rule.file(repo.path())).unwrap();
    assert!(text.starts_with("---\n"), "{text}");
    assert!(text.contains("paths:\n  - \"**/docs/**\"\n"), "{text}");
    assert!(text.ends_with("\n---\n@/x/.harness/rules/docs.md\n"), "{text}");
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
        .contains("@/x/.harness/rules/docs.md"));
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
    assert!(exclude(&repo).contains("/.claude/rules/ai-harness-docs.md"));
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
            .join("ai-harness-board.md")
    );
    let text = fs::read_to_string(rule.file(repo.path())).unwrap();
    assert!(!text.contains("paths:"), "{text}");
    assert!(text.ends_with("---\n@/x/.harness/rules/board.md\n"), "{text}");
    assert!(!rule.ensure(repo.path()).unwrap());
    assert_eq!(
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        ""
    );
    assert!(exclude(&repo).contains("/.claude/rules/ai-harness-board.md"));
}

#[test]
fn schreibt_beide_rules_und_meldet_bei_wiederholung_nichts_neues() {
    let repo = TempRepo::new();
    let rules = ProjectRules::new(home());

    assert_eq!(
        rules.ensure(repo.path()).unwrap(),
        [
            ".claude/rules/ai-harness-docs.md".to_string(),
            ".claude/rules/ai-harness-board.md".to_string()
        ]
    );
    assert_eq!(rules.ensure(repo.path()).unwrap(), Vec::<String>::new());
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
        rules.ensure(repo.path()).unwrap(),
        [".claude/rules/ai-harness-board.md".to_string()]
    );
    assert!(rules.board.file(repo.path()).exists());
}
