//! Losgelöster Worktree eines Recherche- oder Planungslaufs; portiert aus
//! `packages/core/src/dispatch/git/ResearchWorktree.ts`.

use std::path::{Path, PathBuf};

use crate::error::{HarnessError, Result};
use crate::git::{
    GitCommands, GitPaths, GitRunner, LongPaths, PathKey, LONG_PATHS_ARGS, LONG_PATHS_NOTICE,
};
use crate::install::{ClaudeImportApproval, ProjectPluginSwitch, ProjectRules};
use crate::registry::HarnessHome;
use crate::worktrees::links::WorktreeLinks;

/// Angelegter Worktree; `warning` meldet, dass Plugin oder Rules nicht eingerichtet werden
/// konnten (nur Hinweis)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchWorktreeResult {
    pub path: String,
    pub warning: Option<String>,
}

/// Wofür der losgelöste Worktree angelegt wird; der Präfix steht im Ordnernamen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorktreePurpose {
    /// Recherche zu einer Frage (`<repo>-worktrees/research-<ID>`)
    #[default]
    Research,
    /// Planung eines Tickets (`<repo>-worktrees/plan-<ID>`)
    Plan,
}

impl WorktreePurpose {
    /// Präfix des Ordnernamens
    fn prefix(&self) -> &'static str {
        match self {
            WorktreePurpose::Research => "research",
            WorktreePurpose::Plan => "plan",
        }
    }

    /// Bezeichnung in Meldungen
    fn label(&self) -> &'static str {
        match self {
            WorktreePurpose::Research => "Recherche-Worktree",
            WorktreePurpose::Plan => "Planungs-Worktree",
        }
    }
}

/// Benutzerordner und Freigabe der Importe; ohne Angabe aus der Umgebung (wie
/// [`crate::worktrees::manager::WorktreeOptions`])
#[derive(Debug, Clone, Default)]
pub struct ResearchWorktreeOptions {
    /// Benutzerordner für die Rules im neuen Worktree
    pub home: Option<HarnessHome>,
    /// Freigabe der Importe in Claude Code
    pub approval: Option<ClaudeImportApproval>,
}

/// Losgelöster Worktree (`git worktree add --detach … HEAD`) für einen Recherche-Lauf zu einer
/// Frage: der Hauptordner des Nutzers bleibt unberührt, kein Branch entsteht. Liegt neben den
/// Ticket-Worktrees unter `<repo>-worktrees/research-<ID>`. Mit [`WorktreePurpose::Plan`]
/// derselbe Worktree für einen Planungslauf an einem Ticket (`<repo>-worktrees/plan-<ID>`).
#[derive(Debug)]
pub struct ResearchWorktree {
    purpose: WorktreePurpose,
    git: GitRunner,
    long_paths: LongPaths,
    plugin_switch: ProjectPluginSwitch,
    rules: ProjectRules,
    import_approval: ClaudeImportApproval,
    links: WorktreeLinks,
    keys: PathKey,
}

impl ResearchWorktree {
    /// Worktrees dieses Zwecks mit Benutzerordner und Freigabe aus der Umgebung
    pub fn new(purpose: WorktreePurpose) -> Self {
        Self::with_options(purpose, ResearchWorktreeOptions::default())
    }

    /// Worktrees dieses Zwecks mit eigenem Benutzerordner und eigener Freigabe
    pub fn with_options(purpose: WorktreePurpose, options: ResearchWorktreeOptions) -> Self {
        let home = options.home.unwrap_or_else(HarnessHome::from_process_env);
        Self {
            purpose,
            git: GitRunner::new(),
            long_paths: LongPaths::new(),
            plugin_switch: ProjectPluginSwitch::new(),
            rules: ProjectRules::new(home),
            import_approval: options
                .approval
                .unwrap_or_else(ClaudeImportApproval::from_process_env),
            links: WorktreeLinks::new(),
            keys: PathKey::new(),
        }
    }

    /// Pfad des Worktrees einer Frage (Recherche) bzw. eines Tickets (Planung)
    pub fn path(&self, root: &Path, subject_id: &str) -> String {
        let text = root.to_string_lossy().replace('\\', "/");
        let trimmed = text.trim_end_matches('/');
        let (parent, name) = match trimmed.rsplit_once('/') {
            Some(("", name)) => ("", name),
            Some((parent, name)) => (parent, name),
            None => (".", trimmed),
        };
        format!(
            "{parent}/{name}-worktrees/{}-{subject_id}",
            self.purpose.prefix()
        )
    }

    /// Legt den Worktree an (ein Rest eines früheren Laufs wird vorher entfernt) und schaltet
    /// Plugin und Rules ein
    pub fn create(&self, root: &Path, subject_id: &str) -> Result<ResearchWorktreeResult> {
        let path = self.path(root, subject_id);
        if Path::new(&path).exists() {
            self.remove(root, &path);
        }
        if Path::new(&path).exists() {
            return Err(HarnessError::new(
                format!("{}-worktree-busy", self.purpose.prefix()),
                format!(
                    "Der Ordner {path} existiert schon und ist kein entfernbarer Worktree – \
bitte von Hand prüfen."
                ),
            )
            .into());
        }
        let mut warnings: Vec<String> = Vec::new();
        if self.long_paths.ensure(root) {
            warnings.push(LONG_PATHS_NOTICE.to_string());
        }
        let mut args: Vec<&str> = LONG_PATHS_ARGS.to_vec();
        args.extend(["worktree", "add", "--detach", &path, "HEAD"]);
        self.git.run(root, &args)?;
        if let Err(error) = self.prepare(Path::new(&path)) {
            warnings.push(format!(
                "Plugin oder Rules im {} nicht eingerichtet: {error}",
                self.purpose.label()
            ));
        }
        Ok(ResearchWorktreeResult {
            path,
            warning: Some(warnings.join(" ")).filter(|text| !text.is_empty()),
        })
    }

    /// Entfernt genau diesen Worktree (mit `--force`: er enthält nur Recherche-Ausgaben) und
    /// räumt die Git-Verwaltung auf; liefert eine Fehlermeldung oder `None`. Entfernt wird nur
    /// ein Ordner `research-<ID>`, der genau am erwarteten Platz unter `<repo>-worktrees` liegt –
    /// andere Ordner nie
    pub fn remove(&self, root: &Path, path: &str) -> Option<String> {
        let label = self.purpose.label();
        if !self.is_own_path(root, path) {
            return Some(format!("{path} ist kein {label} – nicht entfernt."));
        }
        // Links (Symlinks/Junctions) zuerst entfernen, nie ihr Ziel; bleibt einer stehen, wird
        // nichts gelöscht
        let folder = PathBuf::from(path);
        if folder.exists() {
            let links = self.links.remove_all(&folder);
            if !links.failed.is_empty() {
                let failed: Vec<String> = links
                    .failed
                    .iter()
                    .map(|link| link.to_string_lossy().into_owned())
                    .collect();
                return Some(format!(
                    "{label} {path} nicht entfernt: Links ließen sich nicht entfernen ({}).",
                    failed.join(", ")
                ));
            }
        }
        let attempt = self
            .git
            .try_run(root, &["worktree", "remove", "--force", path]);
        self.git.try_run(root, &["worktree", "prune"]);
        if attempt.ok || !folder.exists() {
            None
        } else {
            Some(format!("{label} {path} nicht entfernt: {}", attempt.err))
        }
    }

    /// Entfernt verwaiste Recherche-Worktrees eines Repos (`repo_path` darf auch ein Unterordner
    /// sein): eingetragene Worktrees `research-<ID>` am erwarteten Platz, zu denen kein Lauf
    /// gehört (`active`: Ordner laufender Recherchen). Liefert je Worktree eine Meldung (entfernt
    /// oder Fehler); ist der Ordner kein Git-Repo, nichts
    pub fn remove_orphans(&self, repo_path: &Path, active: &[String]) -> Vec<String> {
        let Ok(root) = GitPaths::new().main_root(repo_path) else {
            return Vec::new();
        };
        let root = PathBuf::from(root);
        let list = self
            .git
            .try_run(&root, &["worktree", "list", "--porcelain"]);
        if !list.ok {
            return Vec::new();
        }
        let busy: Vec<String> = active.iter().map(|path| self.keys.of(path)).collect();
        let mut notes: Vec<String> = Vec::new();
        for line in list.out.lines() {
            let Some(path) = line.strip_prefix("worktree ") else {
                continue;
            };
            let path = path.trim();
            if !self.is_own_path(&root, path) || busy.contains(&self.keys.of(path)) {
                continue;
            }
            notes.push(
                self.remove(&root, path).unwrap_or_else(|| {
                    format!("verwaister {} {path} entfernt", self.purpose.label())
                }),
            );
        }
        notes
    }

    /// Liegt `path` genau dort, wo der Worktree der Frage bzw. des Tickets aus seinem Namen
    /// `research-<ID>` läge?
    fn is_own_path(&self, root: &Path, path: &str) -> bool {
        let name = Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(id) = name.strip_prefix(&format!("{}-", self.purpose.prefix())) else {
            return false;
        };
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
        self.keys.same(path, self.path(root, id))
    }

    /// Plugin, Rules und die Freigabe ihrer Importe im neuen Worktree
    fn prepare(&self, path: &Path) -> Result<()> {
        self.plugin_switch.enable(path)?;
        self.rules.ensure(path)?;
        self.import_approval.approve(&path.to_string_lossy())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_support::{canonical, make_temp_dir, TempRepo};

    /// Worktrees mit Benutzerordner und Claude-Einstellung in einem temporären Ordner: die Tests
    /// schreiben nie in den echten Benutzerordner
    fn worktrees(purpose: WorktreePurpose, home: &Path) -> ResearchWorktree {
        ResearchWorktree::with_options(
            purpose,
            ResearchWorktreeOptions {
                home: Some(HarnessHome::at(home)),
                approval: Some(ClaudeImportApproval::new(
                    Some(home.to_string_lossy().into_owned()),
                    home.to_path_buf(),
                    cfg!(windows),
                )),
            },
        )
    }

    #[test]
    fn legt_einen_losgelosten_worktree_neben_dem_repo_an_und_entfernt_nur_ihn_wieder() {
        let dir = make_temp_dir("nexus-research-home-");
        let repo = TempRepo::new();
        let worktrees = worktrees(WorktreePurpose::Research, dir.path());
        let created = worktrees
            .create(repo.path(), "ARC-F0007")
            .expect("Worktree anlegen");
        assert_eq!(
            created.path,
            format!(
                "{}/Mein Projekt ä-worktrees/research-ARC-F0007",
                repo.base().to_string_lossy().replace('\\', "/")
            )
        );
        assert!(Path::new(&created.path).exists());
        assert_eq!(
            repo.git(&["-C", &created.path, "rev-parse", "--abbrev-ref", "HEAD"]),
            "HEAD"
        );
        assert!(!repo.git(&["branch", "--list"]).contains("research"));
        std::fs::write(Path::new(&created.path).join("research.json"), "{}").expect("Datei");
        assert_eq!(worktrees.remove(repo.path(), &created.path), None);
        assert!(!Path::new(&created.path).exists());
        assert!(!repo
            .git(&["worktree", "list"])
            .contains("research-ARC-F0007"));
    }

    #[test]
    fn ersetzt_einen_rest_eines_fruheren_laufs_und_fasst_fremde_ordner_nicht_an() {
        let dir = make_temp_dir("nexus-research-home-");
        let repo = TempRepo::new();
        let worktrees = worktrees(WorktreePurpose::Research, dir.path());
        let first = worktrees
            .create(repo.path(), "ARC-F0001")
            .expect("erster Worktree");
        let second = worktrees
            .create(repo.path(), "ARC-F0001")
            .expect("zweiter Worktree");
        assert_eq!(second.path, first.path);
        assert!(Path::new(&second.path).exists());
        let other = repo.base().join("anderes");
        std::fs::create_dir(&other).expect("Ordner");
        let other = other.to_string_lossy().into_owned();
        assert!(worktrees
            .remove(repo.path(), &other)
            .expect("Meldung")
            .contains("kein Recherche-Worktree"));
        assert!(worktrees
            .remove(repo.path(), &repo.path().to_string_lossy())
            .expect("Meldung")
            .contains("kein Recherche-Worktree"));
        assert!(Path::new(&other).exists());
        // Ein Ordner research-<ID> an anderem Platz (nicht unter <repo>-worktrees) wird nie
        // angefasst
        let elsewhere = repo.base().join("research-ARC-F0002");
        std::fs::create_dir(&elsewhere).expect("Ordner");
        let elsewhere = elsewhere.to_string_lossy().into_owned();
        assert!(worktrees
            .remove(repo.path(), &elsewhere)
            .expect("Meldung")
            .contains("kein Recherche-Worktree"));
        assert!(Path::new(&elsewhere).exists());
        worktrees.remove(repo.path(), &second.path);
    }

    #[test]
    fn entfernt_verwaiste_recherche_worktrees_ohne_lauf_laufende_und_fremde_bleiben() {
        let dir = make_temp_dir("nexus-research-home-");
        let repo = TempRepo::new();
        let worktrees = worktrees(WorktreePurpose::Research, dir.path());
        let orphan = worktrees
            .create(repo.path(), "ARC-F0001")
            .expect("verwaister Worktree");
        let running = worktrees
            .create(repo.path(), "ARC-F0002")
            .expect("laufender Worktree");
        let foreign = repo
            .base()
            .join("research-ARC-F0003")
            .to_string_lossy()
            .replace('\\', "/");
        repo.git(&["worktree", "add", "--detach", &foreign, "HEAD"]);
        let notes = worktrees.remove_orphans(repo.path(), std::slice::from_ref(&running.path));
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(
            notes[0].starts_with("verwaister Recherche-Worktree")
                && notes[0].ends_with("-worktrees/research-ARC-F0001 entfernt"),
            "{}",
            notes[0]
        );
        assert!(!Path::new(&orphan.path).exists());
        assert!(Path::new(&running.path).exists());
        assert!(Path::new(&foreign).exists());
        assert!(worktrees
            .remove_orphans(repo.path(), std::slice::from_ref(&running.path))
            .is_empty());
        assert!(worktrees
            .remove_orphans(&repo.base().join("kein-repo"), &[])
            .is_empty());
        worktrees.remove(repo.path(), &running.path);
        repo.git(&["worktree", "remove", "--force", &foreign]);
    }

    #[test]
    fn erkennt_laufende_worktrees_auch_bei_abweichender_pfadschreibweise() {
        let dir = make_temp_dir("nexus-research-home-");
        let repo = TempRepo::new();
        let worktrees = worktrees(WorktreePurpose::Research, dir.path());
        let running = worktrees
            .create(repo.path(), "ARC-F0004")
            .expect("laufender Worktree");
        // Unter Windows andere Groß-/Kleinschreibung und Rückstriche, sonst ein Umweg über „/./“
        let spelled = if cfg!(windows) {
            running.path.to_uppercase().replace('/', "\\")
        } else {
            running.path.replace("/research-", "/./research-")
        };
        assert!(worktrees.remove_orphans(repo.path(), &[spelled]).is_empty());
        assert!(Path::new(&running.path).exists());
        worktrees.remove(repo.path(), &running.path);
    }

    #[test]
    fn prafix_plan_hat_einen_eigenen_ordner_und_raumt_getrennt_auf() {
        let dir = make_temp_dir("nexus-research-home-");
        let repo = TempRepo::new();
        let plans = worktrees(WorktreePurpose::Plan, dir.path());
        let research = worktrees(WorktreePurpose::Research, dir.path());
        let plan = plans.create(repo.path(), "ARC-0007").expect("Planung");
        assert_eq!(
            plan.path,
            format!(
                "{}/Mein Projekt ä-worktrees/plan-ARC-0007",
                repo.base().to_string_lossy().replace('\\', "/")
            )
        );
        let question = research
            .create(repo.path(), "ARC-F0007")
            .expect("Recherche");
        assert!(research
            .remove(repo.path(), &plan.path)
            .expect("Meldung")
            .contains("kein Recherche-Worktree"));
        assert!(plans
            .remove(repo.path(), &question.path)
            .expect("Meldung")
            .contains("kein Planungs-Worktree"));
        assert!(research
            .remove_orphans(repo.path(), std::slice::from_ref(&question.path))
            .is_empty());
        let notes = plans.remove_orphans(repo.path(), &[]);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(
            notes[0].starts_with("verwaister Planungs-Worktree")
                && notes[0].ends_with("-worktrees/plan-ARC-0007 entfernt"),
            "{}",
            notes[0]
        );
        assert!(!Path::new(&plan.path).exists());
        assert!(Path::new(&question.path).exists());
        research.remove(repo.path(), &question.path);
    }

    #[test]
    fn der_pfad_liegt_immer_neben_dem_hauptordner() {
        let worktrees = ResearchWorktree::new(WorktreePurpose::Research);
        let root = canonical(Path::new("."));
        let path = worktrees.path(&root, "ARC-F0001");
        assert!(path.ends_with("-worktrees/research-ARC-F0001"), "{path}");
        assert!(!path.contains('\\'), "{path}");
    }
}
