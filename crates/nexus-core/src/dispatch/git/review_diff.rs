//! Portiert `packages/core/src/dispatch/git/ReviewDiff.ts`.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{HarnessError, Result};
use crate::git::{GitCommands, GitRunner};

/// Größter gezeigter Patch je Datei in Zeichen
const MAX_PATCH: usize = 200_000;

/// Eine geänderte Datei des Ticket-Branches
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ReviewFile {
    pub path: String,
    /// Bisheriger Pfad bei Umbenennung oder Kopie
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    /// A, M, D, R, C, T … (erster Buchstabe von --name-status)
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub binary: bool,
}

/// Dateiliste des Ticket-Branches gegen den Zielbranch des Tickets, sonst gegen den
/// ausgecheckten Branch des Hauptordners
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ReviewFileList {
    /// Zielbranch des Tickets, sonst ausgecheckter Branch des Hauptordners; „HEAD“ bei
    /// Detached HEAD
    pub target: String,
    /// Merge-Basis (Commit)
    pub base: String,
    pub files: Vec<ReviewFile>,
}

/// Patch einer Datei, notfalls gekürzt
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ReviewPatch {
    pub patch: String,
    pub truncated: bool,
}

/// Diff eines Ticket-Branches gegen das Ziel des Merges (Three-Dot: ab der Merge-Basis,
/// fremde Änderungen am Ziel fehlen). Ziel ist der Zielbranch des Tickets, wenn er lokal
/// existiert, sonst der ausgecheckte Branch des Hauptordners (HEAD).
#[derive(Debug, Clone, Copy, Default)]
pub struct ReviewDiff<G: GitCommands = GitRunner> {
    git: G,
}

impl ReviewDiff<GitRunner> {
    /// Neuer Diff (hält keinen Zustand)
    pub fn new() -> Self {
        Self {
            git: GitRunner::new(),
        }
    }
}

impl<G: GitCommands> ReviewDiff<G> {
    /// Diff mit eigenem Git-Aufrufer
    pub fn with_git(git: G) -> Self {
        Self { git }
    }

    /// Ziel, Merge-Basis und geänderte Dateien mit Zeilenzahlen
    pub fn files(&self, root: &Path, branch: &str, target: Option<&str>) -> Result<ReviewFileList> {
        check_branch(branch)?;
        let reference = self.base_ref(root, target)?;
        let symbolic = self
            .git
            .try_run(root, &["symbolic-ref", "--quiet", "--short", "HEAD"]);
        let base = self
            .git
            .run(root, &["merge-base", &reference.reference, branch])?;
        let range = format!("{}...{branch}", reference.reference);
        let stats = numstat(
            &self
                .git
                .raw(root, &["diff", "-M", "--numstat", "-z", &range])?,
        );
        let files = name_status(
            &self
                .git
                .raw(root, &["diff", "-M", "--name-status", "-z", &range])?,
        )
        .into_iter()
        .map(|entry| {
            let stat = stats.get(&entry.path);
            ReviewFile {
                path: entry.path,
                old_path: entry.old_path,
                status: entry.status,
                additions: stat.map_or(0, |s| s.additions),
                deletions: stat.map_or(0, |s| s.deletions),
                binary: stat.is_some_and(|s| s.binary),
            }
        })
        .collect();
        let name = reference.name.unwrap_or_else(|| {
            if symbolic.ok {
                symbolic.out
            } else {
                "HEAD".to_string()
            }
        });
        Ok(ReviewFileList {
            target: name,
            base,
            files,
        })
    }

    /// Patch einer Datei aus der Dateiliste; andere Pfade → `review-file-unknown`
    pub fn file(
        &self,
        root: &Path,
        branch: &str,
        path: &str,
        target: Option<&str>,
    ) -> Result<ReviewPatch> {
        let list = self.files(root, branch, target)?;
        let entry = list
            .files
            .into_iter()
            .find(|file| file.path == path)
            .ok_or_else(|| {
                HarnessError::new(
                    "review-file-unknown",
                    format!(
                        "Die Datei „{path}“ gehört nicht zu den Änderungen des Branches {branch}."
                    ),
                )
            })?;
        let range = format!("{}...{branch}", self.base_ref(root, target)?.reference);
        let mut args = vec!["diff", "-M", &range, "--"];
        if let Some(old) = &entry.old_path {
            args.push(old);
        }
        args.push(&entry.path);
        let patch = self.git.raw(root, &args)?;
        Ok(if patch.len() > MAX_PATCH {
            ReviewPatch {
                patch: cut(&patch, MAX_PATCH),
                truncated: true,
            }
        } else {
            ReviewPatch {
                patch,
                truncated: false,
            }
        })
    }

    /// Basis des Vergleichs: lokaler Zielbranch, wenn auflösbar, sonst HEAD (z. B.
    /// Zielbranch noch nicht angelegt)
    fn base_ref(&self, root: &Path, target: Option<&str>) -> Result<BaseRef> {
        let Some(target) = target.filter(|name| !name.is_empty()) else {
            return Ok(BaseRef::head());
        };
        check_branch(target)?;
        let reference = format!("refs/heads/{target}");
        let found = self
            .git
            .try_run(root, &["rev-parse", "--verify", "--quiet", &reference])
            .ok;
        Ok(if found {
            BaseRef {
                reference,
                name: Some(target.to_string()),
            }
        } else {
            BaseRef::head()
        })
    }
}

/// Ziel des Vergleichs: die Referenz für Git und ihr Name, falls es der Zielbranch ist
struct BaseRef {
    reference: String,
    name: Option<String>,
}

impl BaseRef {
    /// Der ausgecheckte Branch des Hauptordners
    fn head() -> Self {
        Self {
            reference: "HEAD".to_string(),
            name: None,
        }
    }
}

/// Branch-Namen, die Git als Option lesen würde, sind ausgeschlossen
fn check_branch(branch: &str) -> Result<()> {
    if branch.is_empty() || branch.starts_with('-') {
        return Err(HarnessError::new(
            "review-branch-invalid",
            format!("Ungültiger Branch „{branch}“."),
        )
        .into());
    }
    Ok(())
}

/// Kürzt auf höchstens `max` Bytes, ohne ein Zeichen zu zerschneiden
fn cut(text: &str, max: usize) -> String {
    let mut end = max.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// Eine Datei aus `--name-status`
struct StatusEntry {
    path: String,
    old_path: Option<String>,
    status: String,
}

/// Zeilenzahlen einer Datei aus `--numstat`
struct Stat {
    additions: u64,
    deletions: u64,
    binary: bool,
}

/// Ausgabe von `--name-status -z`: Status, Pfad; bei R/C Status, alter Pfad, neuer Pfad
fn name_status(out: &str) -> Vec<StatusEntry> {
    let parts = fields(out);
    let mut list = Vec::new();
    let mut index = 0;
    while index < parts.len() {
        let Some(status) = parts[index].chars().next() else {
            index += 1;
            continue;
        };
        if status == 'R' || status == 'C' {
            list.push(StatusEntry {
                status: status.to_string(),
                old_path: Some(parts.get(index + 1).cloned().unwrap_or_default()),
                path: parts.get(index + 2).cloned().unwrap_or_default(),
            });
            index += 3;
        } else {
            list.push(StatusEntry {
                status: status.to_string(),
                old_path: None,
                path: parts.get(index + 1).cloned().unwrap_or_default(),
            });
            index += 2;
        }
    }
    list
}

/// Ausgabe von `--numstat -z`: „plus\tminus\tpfad“ oder bei Umbenennung
/// „plus\tminus\t“, alter, neuer Pfad; „-“ = binär
fn numstat(out: &str) -> HashMap<String, Stat> {
    let parts = fields(out);
    let mut map = HashMap::new();
    let mut index = 0;
    while index < parts.len() {
        let record: Vec<&str> = parts[index].splitn(3, '\t').collect();
        if record.len() < 3 || !is_count(record[0]) || !is_count(record[1]) {
            index += 1;
            continue;
        }
        let binary = record[0] == "-" && record[1] == "-";
        let stat = Stat {
            additions: if binary {
                0
            } else {
                record[0].parse().unwrap_or(0)
            },
            deletions: if binary {
                0
            } else {
                record[1].parse().unwrap_or(0)
            },
            binary,
        };
        // Umbenennung: der Pfad des Datensatzes ist leer, es folgen alter und neuer Pfad
        let path = if record[2].is_empty() {
            let path = parts.get(index + 2).cloned().unwrap_or_default();
            index += 2;
            path
        } else {
            record[2].to_string()
        };
        map.insert(path, stat);
        index += 1;
    }
    map
}

/// Zerlegt eine mit `-z` erzeugte Ausgabe in ihre Felder (das leere Feld am Ende entfällt)
fn fields(out: &str) -> Vec<String> {
    let mut parts: Vec<String> = out.split('\0').map(str::to_string).collect();
    if parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    parts
}

/// Eine Zahl oder „-“ für binär
fn is_count(field: &str) -> bool {
    field == "-" || (!field.is_empty() && field.bytes().all(|byte| byte.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_support::TempRepo;

    /// Schreibt die Dateien und committet sie
    fn commit(repo: &TempRepo, files: &[(&str, &[u8])], message: &str) {
        for (name, content) in files {
            std::fs::write(repo.path().join(name), content).unwrap();
        }
        repo.git(&["add", "-A"]);
        repo.git(&["commit", "-q", "-m", message]);
    }

    /// Projekt mit Basis-Commit, einem Ticket-Branch (Änderung, neue und binäre Datei,
    /// Löschung, Umbenennung) und einem eigenen Commit auf dem Hauptbranch
    fn repo() -> TempRepo {
        let repo = TempRepo::without_commit();
        commit(
            &repo,
            &[
                ("alt.txt", b"a\nb\nc\n"),
                ("weg.txt", b"weg\n"),
                (
                    "umbenennen.txt",
                    "eins\nzwei\ndrei\nvier\nfünf\n".as_bytes(),
                ),
            ],
            "basis",
        );
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        commit(
            &repo,
            &[
                ("alt.txt", b"a\nneu\nc\n"),
                ("neu.txt", b"neu\n"),
                ("bild.bin", &[0, 1, 2, 0, 255]),
            ],
            "ticket",
        );
        repo.git(&["rm", "-q", "weg.txt"]);
        repo.git(&["mv", "umbenennen.txt", "umbenannt.txt"]);
        repo.git(&["commit", "-q", "-m", "ticket 2"]);
        repo.git(&["checkout", "-q", "-"]);
        commit(&repo, &[("nur-ziel.txt", b"ziel\n")], "ziel");
        repo
    }

    /// Projekt mit main (m.txt), release/1 (r.txt) und ticket/x (t.txt); ausgecheckt
    /// bleibt main
    fn repo_mit_ziel() -> TempRepo {
        let repo = TempRepo::without_commit();
        commit(&repo, &[("m.txt", b"main\n")], "basis");
        repo.git(&["branch", "-M", "main"]);
        repo.git(&["checkout", "-q", "-b", "release/1"]);
        commit(&repo, &[("r.txt", b"release\n")], "release");
        repo.git(&["checkout", "-q", "-b", "ticket/x"]);
        commit(&repo, &[("t.txt", b"ticket\n")], "ticket");
        repo.git(&["checkout", "-q", "main"]);
        repo
    }

    /// Die Datei der Liste mit diesem Pfad
    fn by_path(list: &ReviewFileList, path: &str) -> ReviewFile {
        list.files
            .iter()
            .find(|file| file.path == path)
            .unwrap_or_else(|| panic!("{path} fehlt in der Liste"))
            .clone()
    }

    /// Die Pfade der Liste, sortiert
    fn paths(list: &ReviewFileList) -> Vec<String> {
        let mut paths: Vec<String> = list.files.iter().map(|file| file.path.clone()).collect();
        paths.sort();
        paths
    }

    #[test]
    fn liefert_ziel_basis_und_geanderte_dateien_ab_der_merge_basis() {
        let repo = repo();
        let list = ReviewDiff::new()
            .files(repo.path(), "ticket/x", None)
            .unwrap();
        assert_eq!(list.target, repo.git(&["symbolic-ref", "--short", "HEAD"]));
        assert_eq!(list.base, repo.git(&["merge-base", "HEAD", "ticket/x"]));
        assert_eq!(
            paths(&list),
            ["alt.txt", "bild.bin", "neu.txt", "umbenannt.txt", "weg.txt"]
        );
        let alt = by_path(&list, "alt.txt");
        assert_eq!(
            (alt.status.as_str(), alt.additions, alt.deletions),
            ("M", 1, 1)
        );
        assert!(!alt.binary);
        let neu = by_path(&list, "neu.txt");
        assert_eq!(
            (neu.status.as_str(), neu.additions, neu.deletions),
            ("A", 1, 0)
        );
        let weg = by_path(&list, "weg.txt");
        assert_eq!(
            (weg.status.as_str(), weg.additions, weg.deletions),
            ("D", 0, 1)
        );
        let umbenannt = by_path(&list, "umbenannt.txt");
        assert_eq!(umbenannt.status, "R");
        assert_eq!(umbenannt.old_path.as_deref(), Some("umbenennen.txt"));
        let bild = by_path(&list, "bild.bin");
        assert_eq!((bild.binary, bild.additions, bild.deletions), (true, 0, 0));
    }

    #[test]
    fn liefert_den_patch_einer_datei_der_liste() {
        let repo = repo();
        let diff = ReviewDiff::new();
        let patch = diff.file(repo.path(), "ticket/x", "alt.txt", None).unwrap();
        assert!(patch.patch.contains("+neu"));
        assert!(patch.patch.contains("-b"));
        assert!(!patch.truncated);
        assert!(diff
            .file(repo.path(), "ticket/x", "umbenannt.txt", None)
            .unwrap()
            .patch
            .contains("rename to umbenannt.txt"));
    }

    #[test]
    fn lehnt_pfade_ausserhalb_der_liste_und_optionen_ab() {
        let repo = repo();
        let diff = ReviewDiff::new();
        for path in ["nur-ziel.txt", "--output=x", "gibt-es-nicht.txt"] {
            let error = diff.file(repo.path(), "ticket/x", path, None).unwrap_err();
            assert_eq!(error.code(), Some("review-file-unknown"));
        }
        let error = diff.files(repo.path(), "--output=x", None).unwrap_err();
        assert!(error.to_string().contains("Ungültiger Branch"));
    }

    #[test]
    fn kurzt_sehr_grosse_patches() {
        let repo = repo();
        repo.git(&["checkout", "-q", "ticket/x"]);
        commit(
            &repo,
            &[(
                "gross.txt",
                "zeile mit etwas text\n".repeat(20_000).as_bytes(),
            )],
            "gross",
        );
        repo.git(&["checkout", "-q", "-"]);
        let patch = ReviewDiff::new()
            .file(repo.path(), "ticket/x", "gross.txt", None)
            .unwrap();
        assert!(patch.truncated);
        assert_eq!(patch.patch.len(), MAX_PATCH);
    }

    #[test]
    fn vergleicht_gegen_den_zielbranch_statt_gegen_den_ausgecheckten_branch() {
        let repo = repo_mit_ziel();
        let list = ReviewDiff::new()
            .files(repo.path(), "ticket/x", Some("release/1"))
            .unwrap();
        assert_eq!(list.target, "release/1");
        assert_eq!(list.base, repo.git(&["rev-parse", "release/1"]));
        assert_eq!(paths(&list), ["t.txt"]);
    }

    #[test]
    fn bleibt_ohne_zielbranch_beim_ausgecheckten_branch() {
        let repo = repo_mit_ziel();
        for target in [None, Some("")] {
            let list = ReviewDiff::new()
                .files(repo.path(), "ticket/x", target)
                .unwrap();
            assert_eq!(list.target, "main");
            assert_eq!(list.base, repo.git(&["merge-base", "HEAD", "ticket/x"]));
            assert_eq!(paths(&list), ["r.txt", "t.txt"]);
        }
    }

    #[test]
    fn fallt_bei_nicht_vorhandenem_zielbranch_auf_head_zuruck() {
        let repo = repo_mit_ziel();
        let list = ReviewDiff::new()
            .files(repo.path(), "ticket/x", Some("release/2"))
            .unwrap();
        assert_eq!(list.target, "main");
        assert_eq!(paths(&list), ["r.txt", "t.txt"]);
    }

    #[test]
    fn liefert_den_patch_gegen_den_zielbranch_und_lehnt_dateien_nur_des_zwischenstands_ab() {
        let repo = repo_mit_ziel();
        let diff = ReviewDiff::new();
        assert!(diff
            .file(repo.path(), "ticket/x", "t.txt", Some("release/1"))
            .unwrap()
            .patch
            .contains("+ticket"));
        let error = diff
            .file(repo.path(), "ticket/x", "r.txt", Some("release/1"))
            .unwrap_err();
        assert_eq!(error.code(), Some("review-file-unknown"));
        assert!(diff
            .file(repo.path(), "ticket/x", "r.txt", None)
            .unwrap()
            .patch
            .contains("+release"));
    }

    #[test]
    fn zeigt_im_patch_einer_auch_auf_dem_zielbranch_vorhandenen_datei_nur_die_anderung_des_tickets()
    {
        let repo = repo_mit_ziel();
        // ticket/y zweigt von release/1 ab und hängt eine Zeile an r.txt an, die es auf
        // release/1 schon gibt
        repo.git(&["checkout", "-q", "-b", "ticket/y", "release/1"]);
        commit(&repo, &[("r.txt", b"release\nticket-zeile\n")], "ticket y");
        repo.git(&["checkout", "-q", "main"]);
        let diff = ReviewDiff::new();
        let against = diff
            .file(repo.path(), "ticket/y", "r.txt", Some("release/1"))
            .unwrap()
            .patch;
        assert!(against.contains("+ticket-zeile"));
        assert!(!against.contains("+release"));
        assert!(!against.contains("new file"));
        // Ohne Zielbranch (HEAD = main) ist r.txt für den Vergleich neu: auch die Zeile
        // von release/1 gehört zur Änderung
        let head = diff
            .file(repo.path(), "ticket/y", "r.txt", None)
            .unwrap()
            .patch;
        assert!(head.contains("new file"));
        assert!(head.contains("+release"));
        assert!(head.contains("+ticket-zeile"));
    }

    #[test]
    fn lehnt_einen_zielbranch_ab_den_git_als_option_lesen_wurde() {
        let repo = repo_mit_ziel();
        let error = ReviewDiff::new()
            .files(repo.path(), "ticket/x", Some("--output=x"))
            .unwrap_err();
        assert_eq!(error.code(), Some("review-branch-invalid"));
    }
}
