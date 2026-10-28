//! Portiert `packages/core/src/git/BranchNames.ts`.

use std::collections::BTreeSet;
use std::path::Path;

use crate::git::target_branch_rule::TargetBranchRule;
use crate::git::{GitCommands, GitRunner};

/// Höchstzahl der Namen, wenn keine andere genannt wird
const DEFAULT_LIMIT: usize = 500;

/// Liest die Branch-Namen eines Repos für die Vorschlagsliste des Zielbranches: lokale
/// Branches und die von origin (ohne Präfix zusammengeführt), gefiltert auf das, was als
/// Zielbranch gespeichert werden darf. Liefert nie einen Fehler.
#[derive(Debug, Clone, Copy)]
pub struct BranchNames {
    limit: usize,
    git: GitRunner,
    rule: TargetBranchRule,
}

impl Default for BranchNames {
    fn default() -> Self {
        Self::new()
    }
}

impl BranchNames {
    /// Liste mit der Standardgrenze
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_LIMIT)
    }

    /// Liste mit eigener Höchstzahl
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit,
            git: GitRunner::new(),
            rule: TargetBranchRule::new(),
        }
    }

    /// Branch-Namen alphabetisch, höchstens so viele wie erlaubt
    pub fn list(&self, root: &Path) -> Vec<String> {
        let result = self.git.try_run(
            root,
            &[
                "for-each-ref",
                "--format=%(refname:short)",
                "refs/heads",
                "refs/remotes/origin",
            ],
        );
        if !result.ok || result.out.is_empty() {
            return Vec::new();
        }
        // BTreeSet: ohne Doppelte und gleich alphabetisch sortiert
        let mut names: BTreeSet<String> = BTreeSet::new();
        for line in result.out.split('\n') {
            // Git kürzt refs/remotes/origin/HEAD zu „origin“ – der Zeiger auf den
            // Standardbranch ist kein eigener Branch
            let short = line.trim();
            if short == "origin" || short == "origin/HEAD" {
                continue;
            }
            let name = short.strip_prefix("origin/").unwrap_or(short);
            if name.is_empty() || name == "HEAD" {
                continue;
            }
            if self.allowed(name) {
                names.insert(name.to_string());
            }
        }
        names.into_iter().take(self.limit).collect()
    }

    /// Darf der Name als Zielbranch gespeichert werden? `normalize` meldet ungültige
    /// Namen und den Namensraum der Tickets als Fehler
    fn allowed(&self, name: &str) -> bool {
        matches!(self.rule.normalize(Some(name)), Ok(Some(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_support::TempRepo;

    /// Projekt mit main, release/1 und einem Ticket-Branch
    fn repo() -> TempRepo {
        let repo = TempRepo::new();
        repo.git(&["branch", "-M", "main"]);
        repo.git(&["branch", "release/1"]);
        repo.git(&["branch", "ticket/ARC-0001-x"]);
        repo
    }

    #[test]
    fn liefert_die_lokalen_branches_alphabetisch_ohne_den_namensraum_der_tickets() {
        let repo = repo();

        assert_eq!(BranchNames::new().list(repo.path()), ["main", "release/1"]);
    }

    #[test]
    fn fuhrt_die_branches_von_origin_ohne_prafix_mit_den_lokalen_zusammen() {
        let repo = repo();
        // Remote-Refs direkt setzen: ein echtes origin braucht es dafür nicht
        repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
        repo.git(&["update-ref", "refs/remotes/origin/HEAD", "HEAD"]);
        repo.git(&["update-ref", "refs/remotes/origin/feature/neu", "HEAD"]);

        assert_eq!(
            BranchNames::new().list(repo.path()),
            ["feature/neu", "main", "release/1"]
        );
    }

    #[test]
    fn liefert_ohne_repo_eine_leere_liste() {
        let repo = repo();

        assert!(BranchNames::new()
            .list(&repo.base().join("fehlt"))
            .is_empty());
    }

    #[test]
    fn begrenzt_die_zahl_der_namen() {
        let repo = repo();

        assert_eq!(BranchNames::with_limit(1).list(repo.path()), ["main"]);
    }
}
