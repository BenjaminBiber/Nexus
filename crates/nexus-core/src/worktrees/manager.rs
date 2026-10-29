//! Branch und Worktree je Ticket des DB-Boards.
//! Portiert aus `packages/core/src/worktrees/WorktreeManager.ts`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;

use crate::board::basics::{BoardActor, TicketStatus};
use crate::board::database::{BoardDatabase, TicketFilter};
use crate::board::services::BoardServices;
use crate::board::ticket_service::{MoveOptions, TicketPatch, WriteOptions};
use crate::board::types::Ticket;
use crate::dispatch::config::{JoinWorktrees, MERGE_REASON_PREFIX};
use crate::dispatch::git::{BranchMerger, TargetBranches};
use crate::error::{Error, HarnessError, Result};
use crate::git::{GitCommands, GitRunner, LongPaths, PathKey, LONG_PATHS_ARGS, LONG_PATHS_NOTICE};
use crate::install::{ClaudeImportApproval, ProjectPluginSwitch, ProjectRules, PROJECT_RULE_PATHS};
use crate::project::HarnessLayout;
use crate::registry::HarnessHome;
use crate::text::SlugBuilder;
use crate::tx;
use crate::worktrees::cleaner::WorktreeCleaner;
use crate::worktrees::links::WorktreeLinks;
use crate::worktrees::registry::{SetupState, WorktreeRegistry};
use crate::worktrees::touches::TouchesChecker;

/// Ergebnis von [`WorktreeManager::start`]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartResult {
    pub id: String,
    pub branch: String,
    pub path: String,
    /// `true`, wenn das Ticket einem bestehenden Worktree beigetreten ist
    pub joined: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    /// Pfade fertiger Worktrees, die dabei aufgeräumt wurden
    pub removed: Vec<String>,
    /// Hinweis, wenn der Zielbranch des Tickets dabei neu angelegt wurde (nennt den
    /// Startpunkt)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_created: Option<String>,
}

/// Ergebnis von [`WorktreeManager::finish`] und [`WorktreeManager::accept`]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishResult {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub ready_to_merge: bool,
    pub warnings: Vec<String>,
    /// Pfade fertiger Worktrees, die dabei aufgeräumt wurden
    pub removed: Vec<String>,
}

/// Ergebnis von [`WorktreeManager::remove_merged`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveMergedResult {
    pub removed: Option<String>,
    pub tickets: Vec<String>,
    pub warnings: Vec<String>,
}

/// Fortsetzung eines gestarteten Tickets: sein Worktree – oder der Hinweis, dass Branch und
/// Worktree nicht mehr zu ihm gehören und am Ticket geleert wurden (dann startet es neu)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeResult {
    /// Der Worktree des Tickets
    Path(String),
    /// Branch und Worktree wurden geleert; der Text nennt den Grund
    Reset(String),
}

/// Angaben für [`WorktreeManager::start`]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartOptions {
    /// Beitritt zum Worktree dieses Tickets (`--in`)
    pub in_ticket: Option<String>,
    /// Beitritt zu einem passenden offenen Worktree (`--join`)
    pub join: bool,
    pub force: bool,
    pub session_id: Option<String>,
    pub repo_board: Option<RepoBoard>,
    /// Tickets ohne Spec treten nie bei (Dispatcher)
    pub join_only_same_spec: bool,
}

/// Angaben für [`WorktreeManager::finish`]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FinishOptions {
    pub acceptance: Option<String>,
    pub force: bool,
    pub repo_board: Option<RepoBoard>,
}

/// Einstellungen des Verwalters
#[derive(Debug, Clone, Default)]
pub struct WorktreeOptions {
    /// `None` = jedes Ticket im eigenen Worktree („bereit“, sobald seine Tickets fertig
    /// sind); `SameSpec` = Worktree bleibt offen, solange ein ungestartetes Ticket derselben
    /// Spec wartet (Dispatcher-Einstellung `joinWorktrees`)
    pub join_worktrees: JoinWorktrees,
    /// Benutzerordner für die Rules im neuen Worktree (ohne Angabe aus der Umgebung)
    pub home: Option<HarnessHome>,
    /// Freigabe der Importe in Claude Code (ohne Angabe aus der Umgebung)
    pub approval: Option<ClaudeImportApproval>,
}

/// Board, das dem Repo zugeordnet ist; Tickets anderer Boards werden abgelehnt
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoBoard {
    pub id: i32,
    pub name: String,
}

/// Lokale Plugin-Einstellung des Harness im Worktree; zählt nie als Änderung
const HARNESS_FILE: &str = ".claude/settings.local.json";

/// Alle lokalen Dateien des Harness im Worktree (Plugin-Einstellung und Rules); zählen nie
/// als Änderung
fn harness_files() -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    files.insert(HARNESS_FILE.to_string());
    for path in PROJECT_RULE_PATHS {
        files.insert((*path).to_string());
    }
    files
}

/// Fertige Tickets halten keinen Worktree mehr offen
fn finished(status: TicketStatus) -> bool {
    matches!(status, TicketStatus::Done | TicketStatus::NotNeeded)
}

/// Verwaltet Branch und Worktree je Ticket des DB-Boards: ein Branch pro Worktree, Tickets
/// ohne Überschneidung dürfen teilen. Gemergt wird nur vom Dispatcher; `remove_merged` räumt
/// danach auf. Das Verzeichnis der Worktrees bleibt eine lokale Datei im Git-Verzeichnis.
#[derive(Debug)]
pub struct WorktreeManager<D: BoardDatabase> {
    pub registry: WorktreeRegistry,
    services: BoardServices<D>,
    root: PathBuf,
    git: GitRunner,
    long_paths: LongPaths,
    merger: BranchMerger,
    targets: TargetBranches,
    touches: TouchesChecker,
    links: WorktreeLinks,
    keys: PathKey,
    plugin_switch: ProjectPluginSwitch,
    rules: ProjectRules,
    import_approval: ClaudeImportApproval,
    join_worktrees: JoinWorktrees,
}

impl<D: BoardDatabase> WorktreeManager<D> {
    /// Verwalter für dieses Projekt (Harness-Ordner, Dienste des Boards, Hauptordner)
    pub fn new(
        layout: HarnessLayout,
        services: BoardServices<D>,
        root: impl Into<PathBuf>,
        options: WorktreeOptions,
    ) -> Self {
        let home = options.home.unwrap_or_else(HarnessHome::from_process_env);
        Self {
            registry: WorktreeRegistry::new(layout),
            services,
            root: root.into(),
            git: GitRunner::new(),
            long_paths: LongPaths::new(),
            merger: BranchMerger::new(),
            targets: TargetBranches::new(),
            touches: TouchesChecker::new(),
            links: WorktreeLinks::new(),
            keys: PathKey::new(),
            plugin_switch: ProjectPluginSwitch::new(),
            rules: ProjectRules::new(home),
            import_approval: options
                .approval
                .unwrap_or_else(ClaudeImportApproval::from_process_env),
            join_worktrees: options.join_worktrees,
        }
    }

    /// Startet ein Ticket: Sperren prüfen, fertige Worktrees aufräumen, dann Beitritt zu
    /// `in_ticket`, mit `join` sonst zu einem passenden offenen Worktree derselben Spec,
    /// sonst neuer Worktree. Verknüpft auf Wunsch die Session.
    pub async fn start(
        &self,
        id: &str,
        options: &StartOptions,
        cwd: &Path,
        actor: &BoardActor,
    ) -> Result<StartResult> {
        let ticket = self.services.tickets.get(id).await?.view.ticket.clone();
        self.assert_repo_board(&ticket, options.repo_board.as_ref())
            .await?;
        if let Some(branch) = &ticket.branch {
            return Err(HarnessError::new(
                "already-started",
                format!("{} läuft bereits auf Branch {branch}.", ticket.id),
            )
            .into());
        }
        self.services
            .tickets
            .check_start(&ticket.id, options.force)
            .await?;
        let removed = WorktreeCleaner::new(&self.registry, &self.git).cleanup(cwd)?;
        let mut result: Option<StartResult> = None;
        let mut warning: Option<String> = None;
        if let Some(host_id) = &options.in_ticket {
            match self
                .try_join(&ticket, host_id, actor, options.force)
                .await?
            {
                Ok(joined) => result = Some(joined),
                Err(text) => warning = Some(text),
            }
        } else if options.join {
            result = self
                .auto_join(&ticket, actor, options.force, options.join_only_same_spec)
                .await?;
        }
        let mut result = match result {
            Some(result) => result,
            None => {
                let started = self.start_new(&ticket, cwd, actor, options.force).await?;
                let warnings: Vec<String> = [warning, started.warning.clone()]
                    .into_iter()
                    .flatten()
                    .collect();
                StartResult {
                    warning: join_warnings(&warnings),
                    ..started
                }
            }
        };
        if let Some(session_id) = &options.session_id {
            self.services
                .tickets
                .link_session(&ticket.id, session_id)
                .await?;
        }
        result.removed = removed;
        Ok(result)
    }

    /// Schließt ein Ticket ab: Erledigt (Abnahme-Pflicht), Worktree „bereit“ sobald alle
    /// Tickets fertig sind. Ein fertiger, sauberer Worktree wird aufgeräumt (der Branch
    /// bleibt), sofern `cwd` nicht darin liegt.
    pub async fn finish(
        &self,
        id: &str,
        options: &FinishOptions,
        cwd: &Path,
        actor: &BoardActor,
    ) -> Result<FinishResult> {
        let ticket = self.services.tickets.get(id).await?.view.ticket.clone();
        self.assert_repo_board(&ticket, options.repo_board.as_ref())
            .await?;
        self.services
            .tickets
            .move_to(
                &ticket.id,
                TicketStatus::Done,
                actor,
                &MoveOptions {
                    acceptance: options.acceptance.clone(),
                    force: options.force,
                    ..MoveOptions::default()
                },
            )
            .await?;
        let ready_to_merge = match &ticket.branch {
            Some(branch) => self.update_ready(branch).await?,
            None => false,
        };
        let removed = WorktreeCleaner::new(&self.registry, &self.git).cleanup(cwd)?;
        Ok(FinishResult {
            id: ticket.id,
            branch: ticket.branch,
            ready_to_merge,
            warnings: Vec::new(),
            removed,
        })
    }

    /// Worktree eines gestarteten Tickets; fehlt der Ordner (aufgeräumt), wird er aus dem
    /// Branch neu angelegt. Ein vorhandener Ordner muss als Worktree genau dieses Branches
    /// eingetragen sein.
    pub async fn ensure_worktree(&self, id: &str) -> Result<String> {
        let ticket = self.services.tickets.get(id).await?.view.ticket.clone();
        let (branch, worktree) = match (&ticket.branch, &ticket.worktree) {
            (Some(branch), Some(worktree)) => (branch.clone(), worktree.clone()),
            _ => {
                return Err(HarnessError::new(
                    "ticket-not-started",
                    format!("{} läuft in keinem Worktree.", ticket.id),
                )
                .into())
            }
        };
        if Path::new(&worktree).exists() {
            if !self.is_worktree_of(Path::new(&worktree), &branch)? {
                return Err(HarnessError::new(
                    "worktree-mismatch",
                    format!(
                        "Der Ordner {worktree} ist kein Worktree von Branch {branch}. \
Bitte den Ordner prüfen und von Hand aufräumen."
                    ),
                )
                .into());
            }
            return Ok(worktree);
        }
        self.git.run(&self.root, &["worktree", "prune"])?;
        self.long_paths.ensure(&self.root);
        let mut args: Vec<&str> = LONG_PATHS_ARGS.to_vec();
        args.extend(["worktree", "add", &worktree, &branch]);
        self.git.run(&self.root, &args)?;
        self.registry.mark_restored(&branch)?;
        self.enable_plugin(Path::new(&worktree));
        Ok(worktree)
    }

    /// Ist der Worktree des Branches „bereit zum Merge“? Ein Worktree je Ticket: jetzt
    /// geprüft (alle seine Tickets fertig, auch wenn eines von Hand auf Erledigt kam);
    /// gebündelt: die Markierung aus `finish` (alle fertig, keines derselben Spec wartet)
    pub async fn is_ready(&self, branch: &str) -> Result<bool> {
        if self.join_worktrees == JoinWorktrees::None {
            return self.update_ready(branch).await;
        }
        Ok(self
            .registry
            .read()?
            .get(branch)
            .is_some_and(|entry| entry.ready_to_merge))
    }

    /// Abnahme durch einen Menschen (Blockiert mit Grund „Human Interaction: …“): Erledigt,
    /// Grund leeren, Verlauf, danach wie `finish` den Worktree als bereit markieren
    pub async fn accept(
        &self,
        id: &str,
        repo_board: Option<&RepoBoard>,
        cwd: &Path,
        actor: &BoardActor,
    ) -> Result<FinishResult> {
        let ticket = self.services.tickets.get(id).await?.view.ticket.clone();
        self.assert_repo_board(&ticket, repo_board).await?;
        self.services
            .tickets
            .accept(&ticket.id, actor, &WriteOptions::default())
            .await?;
        let ready_to_merge = match &ticket.branch {
            Some(branch) => self.update_ready(branch).await?,
            None => false,
        };
        let removed = WorktreeCleaner::new(&self.registry, &self.git).cleanup(cwd)?;
        Ok(FinishResult {
            id: ticket.id,
            branch: ticket.branch,
            ready_to_merge,
            warnings: Vec::new(),
            removed,
        })
    }

    /// Fortsetzung eines gestarteten Tickets (Dispatcher): Gehört der eingetragene Worktree
    /// laut Verzeichnis einem anderen Ticket oder wurde er aufgeräumt, wird er nicht
    /// wiederhergestellt – Branch und Worktree am Ticket werden geleert (der alte Branch
    /// bleibt im Repo) und das Ticket startet neu. Sonst der Worktree wie bei
    /// `ensure_worktree` (fehlender Ordner wird neu angelegt).
    pub async fn resume(&self, id: &str, actor: &BoardActor) -> Result<ResumeResult> {
        let ticket = self.services.tickets.get(id).await?.view.ticket.clone();
        let (branch, worktree) = match (&ticket.branch, &ticket.worktree) {
            (Some(branch), Some(worktree)) => (branch.clone(), worktree.clone()),
            _ => {
                return Err(HarnessError::new(
                    "ticket-not-started",
                    format!("{} läuft in keinem Worktree.", ticket.id),
                )
                .into())
            }
        };
        let Some(why) = self.stale_reason(&ticket.id, Some(&branch))? else {
            return Ok(ResumeResult::Path(self.ensure_worktree(&ticket.id).await?));
        };
        self.clear_branch(&ticket.id, actor).await?;
        Ok(ResumeResult::Reset(format!(
            "Der eingetragene Worktree {worktree} (Branch {branch}) {why} und wird nicht \
wiederhergestellt; Branch und Worktree am Ticket geleert, es startet auf einem eigenen neuen \
Branch (der alte Branch bleibt im Repo)."
        )))
    }

    /// Warum der eingetragene Worktree des Tickets nicht (mehr) zu ihm gehört – laut
    /// Verzeichnis einem anderen Ticket oder aufgeräumt –, sonst `None` (auch ohne Branch
    /// oder ohne Eintrag im Verzeichnis); ändert nichts
    pub fn stale_reason(&self, id: &str, branch: Option<&str>) -> Result<Option<String>> {
        let Some(branch) = branch else {
            return Ok(None);
        };
        let map = self.registry.read()?;
        let Some(entry) = map.get(branch) else {
            return Ok(None);
        };
        if !entry.tickets.iter().any(|ticket| ticket == id) {
            let names = entry.tickets.join(", ");
            let names = if names.is_empty() {
                "einem anderen Ticket".to_string()
            } else {
                names
            };
            return Ok(Some(format!("gehört {names}")));
        }
        Ok(if entry.is_removed() {
            Some("wurde aufgeräumt".to_string())
        } else {
            None
        })
    }

    /// Steht die Einrichtung (`worktreeSetup`) des Worktrees noch aus? Nur für Worktrees,
    /// die der Dispatcher mit Einrichtung anlegte
    pub fn setup_pending(&self, branch: &str) -> Result<bool> {
        Ok(self
            .registry
            .read()?
            .get(branch)
            .is_some_and(|entry| entry.setup == Some(SetupState::Pending)))
    }

    /// Merkt, dass der Worktree eingerichtet werden muss (pending) bzw. eingerichtet ist (done)
    pub fn mark_setup(&self, branch: &str, state: SetupState) -> Result<()> {
        self.registry.set_setup(branch, state)
    }

    /// Tickets, die im Worktree dieses Branches arbeiten (leer, wenn es keinen Eintrag gibt)
    pub fn tickets_of(&self, branch: &str) -> Result<Vec<String>> {
        Ok(self
            .registry
            .read()?
            .get(branch)
            .map(|entry| entry.tickets.clone())
            .unwrap_or_default())
    }

    /// Branch, dem `start` das ungestartete Ticket automatisch beitreten würde (ohne Beitritt:
    /// `None`); ändert nichts. Der Dispatcher prüft damit vor der Übernahme, ob in diesem
    /// Worktree gerade ein anderer Worker läuft.
    pub async fn join_candidate(
        &self,
        id: &str,
        join_only_same_spec: bool,
    ) -> Result<Option<String>> {
        let ticket = self.services.tickets.get(id).await?.view.ticket.clone();
        if ticket.branch.is_some() {
            return Ok(None);
        }
        Ok(self
            .find_join(&ticket, join_only_same_spec)
            .await?
            .map(|(branch, _)| branch))
    }

    /// Nach dem Merge: nur wenn der Branch im Zielbranch steckt (ohne Zielbranch: im
    /// ausgecheckten HEAD des Hauptordners) oder mit seiner aktuellen Spitze per Squash
    /// übernommen ist (siehe [`BranchMerger::integrated`]), den sauberen Worktree entfernen
    /// (ohne Gewalt), Branch löschen und erst dann Branch/Worktree an allen Tickets des
    /// Eintrags leeren. Sonst Warnungen. Mit Zielbranch wird immer auf der geprüften Spitze
    /// gelöscht, weil `git branch -d` gegen HEAD statt gegen das Ziel prüft.
    pub async fn remove_merged(
        &self,
        branch: &str,
        actor: &BoardActor,
        target: Option<&str>,
    ) -> Result<RemoveMergedResult> {
        let entry = self.registry.read()?.get(branch).cloned();
        let tickets = entry
            .as_ref()
            .map(|entry| entry.tickets.clone())
            .unwrap_or_default();
        let mut warnings: Vec<String> = Vec::new();
        let reference = match target {
            Some(target) => format!("refs/heads/{target}"),
            None => "HEAD".to_string(),
        };
        // Schon gemergt und gelöscht (Abbruch beim letzten Aufräumen): nur noch Verzeichnis
        // und Tickets nachziehen
        if self
            .merger
            .merged_earlier(&self.root, branch, &reference)
            .is_some()
        {
            if let Some(entry) = &entry {
                if !entry.is_removed() && !Path::new(&entry.path).exists() {
                    self.registry.mark_removed(branch)?;
                }
            }
            self.git.try_run(&self.root, &["worktree", "prune"]);
            self.clear_branch_of(&tickets, branch, actor).await?;
            return Ok(RemoveMergedResult {
                removed: None,
                tickets,
                warnings,
            });
        }
        if !self.merger.integrated(&self.root, branch, &reference) {
            return Ok(RemoveMergedResult {
                removed: None,
                tickets,
                warnings: vec![format!(
                    "Branch {branch} ist nicht gemergt; Worktree und Branch bleiben."
                )],
            });
        }
        // Per Squash übernommen oder in einen anderen Zielbranch als HEAD gemergt: Git hält
        // den Branch für ungemergt, gelöscht wird er nur auf der geprüften Spitze
        let full = format!("refs/heads/{branch}");
        let squashed_tip = if target.is_none() && self.merger.contains(&self.root, &full, "HEAD") {
            None
        } else {
            Some(self.merger.head(&self.root, &full)?)
        };
        let mut removed: Option<String> = None;
        if let Some(entry) =
            entry.filter(|entry| !entry.is_removed() && Path::new(&entry.path).exists())
        {
            let path = PathBuf::from(&entry.path);
            if is_empty_dir(&path) {
                // Rest eines halb entfernten Worktrees (Windows ließ den gesperrten leeren
                // Ordner stehen): ohne .git kein Worktree; `remove_dir` löscht nur leere Ordner
                fs::remove_dir(&path).map_err(Error::unexpected)?;
                self.registry.mark_removed(branch)?;
                removed = Some(entry.path.clone());
            } else {
                if self.keys.same(&path, &self.root) {
                    warnings.push(format!(
                        "Der Worktree-Eintrag von Branch {branch} zeigt auf den Hauptordner {}; \
er wird nicht entfernt.",
                        entry.path
                    ));
                    return Ok(RemoveMergedResult {
                        removed: None,
                        tickets,
                        warnings,
                    });
                }
                if !self.is_worktree_of(&path, branch)? {
                    warnings.push(format!(
                        "Der Ordner {} ist kein Worktree von Branch {branch}; er wird nicht \
angefasst. Bitte von Hand prüfen.",
                        entry.path
                    ));
                    return Ok(RemoveMergedResult {
                        removed: None,
                        tickets,
                        warnings,
                    });
                }
                if !self.is_clean(&path) {
                    warnings.push(format!(
                        "Worktree {} hat ungesicherte Änderungen und bleibt stehen.",
                        entry.path
                    ));
                    return Ok(RemoveMergedResult {
                        removed: None,
                        tickets,
                        warnings,
                    });
                }
                // Symlinks/Junctions (z. B. node_modules auf den Hauptordner) zuerst
                // entfernen – nur die Links, nie ihr Ziel; sonst könnte Git beim Entfernen
                // durch den Link hindurch löschen
                let links = self.links.remove_all(&path);
                if !links.failed.is_empty() {
                    let failed: Vec<String> = links
                        .failed
                        .iter()
                        .map(|link| link.to_string_lossy().into_owned())
                        .collect();
                    warnings.push(format!(
                        "Worktree {} nicht entfernt: Links ließen sich nicht entfernen ({}). \
Bitte von Hand prüfen.",
                        entry.path,
                        failed.join(", ")
                    ));
                    return Ok(RemoveMergedResult {
                        removed: None,
                        tickets,
                        warnings,
                    });
                }
                // Plugin-Einstellung und Rules liegen nicht im Git und würden
                // `worktree remove` ohne --force verhindern
                for file in harness_files() {
                    let mut local = path.clone();
                    for part in file.split('/') {
                        local.push(part);
                    }
                    let _ = fs::remove_file(local);
                }
                let attempt = self
                    .git
                    .try_run(&self.root, &["worktree", "remove", &entry.path]);
                if !attempt.ok {
                    let plugin = self.enable_plugin(&path);
                    warnings.push(format!(
                        "Worktree {} nicht entfernt: {}",
                        entry.path, attempt.err
                    ));
                    warnings.extend(plugin);
                    return Ok(RemoveMergedResult {
                        removed: None,
                        tickets,
                        warnings,
                    });
                }
                self.registry.mark_removed(branch)?;
                removed = Some(entry.path.clone());
            }
        }
        self.git.try_run(&self.root, &["worktree", "prune"]);
        if let Some(failed) = self
            .merger
            .delete_branch(&self.root, branch, squashed_tip.as_deref())
        {
            warnings.push(format!(
                "{failed} Branch und Worktree bleiben an den Tickets eingetragen."
            ));
            return Ok(RemoveMergedResult {
                removed,
                tickets,
                warnings,
            });
        }
        self.clear_branch_of(&tickets, branch, actor).await?;
        Ok(RemoveMergedResult {
            removed,
            tickets,
            warnings,
        })
    }

    /// Leert Branch und Worktree an allen Tickets, die noch auf diesem Branch stehen
    async fn clear_branch_of(
        &self,
        tickets: &[String],
        branch: &str,
        actor: &BoardActor,
    ) -> Result<()> {
        for ticket in self.load(tickets).await? {
            if ticket.branch.as_deref() == Some(branch) {
                self.clear_branch(&ticket.id, actor).await?;
            }
        }
        Ok(())
    }

    /// Leert Branch und Worktree an einem Ticket (immer erlaubt)
    async fn clear_branch(&self, id: &str, actor: &BoardActor) -> Result<()> {
        self.services
            .tickets
            .update(
                id,
                &TicketPatch {
                    branch: Some(None),
                    worktree: Some(None),
                    ..TicketPatch::default()
                },
                actor,
                &WriteOptions {
                    force: true,
                    ..WriteOptions::default()
                },
            )
            .await?;
        Ok(())
    }

    /// Sauber: keine geänderten oder neuen Dateien außer den lokalen Dateien des Harness
    /// (Umbenennungen zählen mit beiden Pfaden)
    fn is_clean(&self, path: &Path) -> bool {
        let Ok(output) = self.git.raw(
            path,
            &[
                "-c",
                "core.quotePath=false",
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
            ],
        ) else {
            return false;
        };
        let harness = harness_files();
        let entries: Vec<&str> = output.split('\0').collect();
        let mut index = 0;
        while index < entries.len() {
            let entry = entries[index];
            if entry.len() < 4 {
                index += 1;
                continue;
            }
            if !harness.contains(&entry[3..]) {
                return false;
            }
            // Umbenennung/Kopie: der nächste Eintrag ist der alte Pfad
            let marks = &entry.as_bytes()[..2];
            if marks.iter().any(|mark| *mark == b'R' || *mark == b'C') {
                index += 1;
                if let Some(old) = entries.get(index) {
                    if !harness.contains(*old) {
                        return false;
                    }
                }
            }
            index += 1;
        }
        true
    }

    /// Ist der Ordner in `git worktree list` als Worktree des Branches eingetragen?
    fn is_worktree_of(&self, path: &Path, branch: &str) -> Result<bool> {
        let wanted = self.keys.of(path);
        let listed = self
            .git
            .run(&self.root, &["worktree", "list", "--porcelain"])?;
        let mut current: Option<String> = None;
        for line in listed.split('\n') {
            let text = line.trim();
            if let Some(found) = text.strip_prefix("worktree ") {
                current = Some(self.keys.of(found));
            } else if text == format!("branch refs/heads/{branch}") {
                if current.as_deref() == Some(wanted.as_str()) {
                    return Ok(true);
                }
            } else if text.is_empty() {
                current = None;
            }
        }
        Ok(false)
    }

    /// Lehnt Tickets ab, die nicht auf dem Board des Repos liegen (Worktrees entstehen immer
    /// neben diesem Repo)
    async fn assert_repo_board(
        &self,
        ticket: &Ticket,
        repo_board: Option<&RepoBoard>,
    ) -> Result<()> {
        let Some(repo_board) = repo_board else {
            return Ok(());
        };
        if ticket.board_id == repo_board.id {
            return Ok(());
        }
        let board_id = ticket.board_id;
        let own = tx!(self.services.db, [board_id], |u| {
            u.boards().by_id(board_id).await
        })
        .await?;
        let ticket_board = match own {
            Some(board) => format!("Board {board_id} „{}“", board.name),
            None => format!("Board {board_id}"),
        };
        Err(HarnessError::new(
            "board-repo-mismatch",
            format!(
                "{} liegt auf {ticket_board}, dieses Repo gehört aber zu Board {} „{}“. \
Das Ticket im Repo seines Boards bearbeiten oder die Zuordnung mit \
„nexus board boards repo <id> <pfad>“ ändern.",
                ticket.id, repo_board.id, repo_board.name
            ),
        )
        .with_details(json!({ "ticketBoard": ticket.board_id, "repoBoard": repo_board.id }))
        .into())
    }

    /// Versucht den Beitritt; bei anderem Zielbranch oder Überschneidung kommt eine Warnung
    /// statt eines Ergebnisses zurück
    #[allow(clippy::result_large_err)]
    async fn try_join(
        &self,
        ticket: &Ticket,
        host_id: &str,
        actor: &BoardActor,
        force: bool,
    ) -> Result<std::result::Result<StartResult, String>> {
        let host = self
            .services
            .tickets
            .get(host_id)
            .await?
            .view
            .ticket
            .clone();
        let (host_branch, host_path) = match (&host.branch, &host.worktree) {
            (Some(branch), Some(worktree)) => (branch.clone(), worktree.clone()),
            _ => {
                return Err(HarnessError::new(
                    "host-not-started",
                    format!("{} läuft in keinem Worktree.", host.id),
                )
                .into())
            }
        };
        if !same_target(ticket, &host) {
            return Ok(Err(format!(
                "{} hat einen anderen Zielbranch als {}; es wird ein eigener Worktree angelegt.",
                ticket.id, host.id
            )));
        }
        let tickets = self
            .registry
            .read()?
            .get(&host_branch)
            .map(|entry| entry.tickets.clone())
            .unwrap_or_default();
        if let Some(other) = self.overlapping(ticket, &tickets).await? {
            return Ok(Err(format!(
                "Die Bereiche von {} überschneiden sich mit {}; es wird ein eigener Worktree \
angelegt.",
                ticket.id, other.id
            )));
        }
        Ok(Ok(self
            .join(ticket, &host_branch, &host_path, actor, force)
            .await?))
    }

    /// Tritt dem ersten offenen Worktree derselben Spec bei, dessen laufende Tickets sich
    /// nicht überschneiden. `same_spec_only`: Tickets ohne Spec treten nie bei (Dispatcher:
    /// jedes Ticket ohne Spec bekommt seinen eigenen Branch).
    async fn auto_join(
        &self,
        ticket: &Ticket,
        actor: &BoardActor,
        force: bool,
        same_spec_only: bool,
    ) -> Result<Option<StartResult>> {
        let Some((branch, path)) = self.find_join(ticket, same_spec_only).await? else {
            return Ok(None);
        };
        Ok(Some(self.join(ticket, &branch, &path, actor, force).await?))
    }

    /// Erster offener Worktree derselben Spec und desselben Zielbranches ohne Überschneidung,
    /// dem das Ticket beitreten kann
    async fn find_join(
        &self,
        ticket: &Ticket,
        same_spec_only: bool,
    ) -> Result<Option<(String, String)>> {
        if same_spec_only && ticket.spec.is_none() {
            return Ok(None);
        }
        let entries: Vec<(String, crate::worktrees::registry::WorktreeEntry)> = self
            .registry
            .read()?
            .iter()
            .map(|(branch, entry)| (branch.clone(), entry.clone()))
            .collect();
        for (branch, entry) in entries {
            if entry.is_removed() || entry.ready_to_merge || !Path::new(&entry.path).exists() {
                continue;
            }
            let others = self.load(&entry.tickets).await?;
            if !others.iter().all(|other| other.spec == ticket.spec) {
                continue;
            }
            if !others.iter().all(|other| same_target(ticket, other)) {
                continue;
            }
            if self.overlapping(ticket, &entry.tickets).await?.is_some() {
                continue;
            }
            return Ok(Some((branch, entry.path)));
        }
        Ok(None)
    }

    /// Erstes noch nicht fertige Ticket der Liste, dessen Bereiche sich mit dem Ticket
    /// überschneiden
    async fn overlapping(&self, ticket: &Ticket, ids: &[String]) -> Result<Option<Ticket>> {
        Ok(self.load(ids).await?.into_iter().find(|other| {
            other.id != ticket.id
                && !finished(other.status)
                && self.touches.overlaps(&ticket.touches, &other.touches)
        }))
    }

    /// Nimmt das Ticket in den bestehenden Worktree auf und setzt es auf In Arbeit
    async fn join(
        &self,
        ticket: &Ticket,
        branch: &str,
        path: &str,
        actor: &BoardActor,
        force: bool,
    ) -> Result<StartResult> {
        self.registry.add_ticket(branch, &ticket.id)?;
        self.services
            .tickets
            .update(
                &ticket.id,
                &TicketPatch {
                    branch: Some(Some(branch.to_string())),
                    worktree: Some(Some(path.to_string())),
                    status: Some(TicketStatus::InProgress),
                    ..TicketPatch::default()
                },
                actor,
                &WriteOptions {
                    force,
                    ..WriteOptions::default()
                },
            )
            .await?;
        Ok(StartResult {
            id: ticket.id.clone(),
            branch: branch.to_string(),
            path: path.to_string(),
            joined: true,
            warning: None,
            removed: Vec::new(),
            target_created: None,
        })
    }

    /// Legt Branch und Worktree neu an, nachdem alle Voraussetzungen geprüft sind; hat das
    /// Ticket einen Zielbranch, wird er bei Bedarf lokal angelegt (immer relativ zum
    /// Hauptordner, die Refs sind gemeinsam) und der Ticket-Branch zweigt von ihm ab
    async fn start_new(
        &self,
        ticket: &Ticket,
        cwd: &Path,
        actor: &BoardActor,
        force: bool,
    ) -> Result<StartResult> {
        let branch = format!("ticket/{}-{}", ticket.id, SlugBuilder::build(&ticket.title));
        let path = self.worktree_path(&ticket.id);
        self.assert_ready(&branch, &path, cwd)?;
        let target = ticket.target_branch.clone();
        let ensured = match &target {
            Some(target) => Some(self.targets.ensure(&self.root, target)?),
            None => None,
        };
        let long_paths = if self.long_paths.ensure(cwd) {
            Some(LONG_PATHS_NOTICE.to_string())
        } else {
            None
        };
        let start_point = target.as_ref().map(|target| format!("refs/heads/{target}"));
        let mut args: Vec<&str> = LONG_PATHS_ARGS.to_vec();
        args.extend(["worktree", "add", "-b", &branch, &path]);
        if let Some(start_point) = &start_point {
            args.push(start_point);
        }
        self.git.run(cwd, &args)?;
        self.registry.add(&branch, &path, &ticket.id)?;
        self.services
            .tickets
            .update(
                &ticket.id,
                &TicketPatch {
                    branch: Some(Some(branch.clone())),
                    worktree: Some(Some(path.clone())),
                    status: Some(TicketStatus::InProgress),
                    ..TicketPatch::default()
                },
                actor,
                &WriteOptions {
                    force,
                    ..WriteOptions::default()
                },
            )
            .await?;
        let worktree = PathBuf::from(&path);
        let warnings: Vec<String> = [
            long_paths,
            self.enable_plugin(&worktree),
            self.add_rules(&worktree),
        ]
        .into_iter()
        .flatten()
        .collect();
        let target_created = ensured.filter(|ensured| ensured.created).map(|ensured| {
            format!(
                "Zielbranch {} neu angelegt von {}.",
                target.clone().unwrap_or_default(),
                ensured.from.clone().unwrap_or_default()
            )
        });
        Ok(StartResult {
            id: ticket.id.clone(),
            branch,
            path,
            joined: false,
            warning: join_warnings(&warnings),
            removed: Vec::new(),
            target_created,
        })
    }

    /// Ordner des Worktrees eines Tickets: `<hauptordner>-worktrees/<ID>`
    fn worktree_path(&self, id: &str) -> String {
        let parent = self.root.parent().unwrap_or_else(|| Path::new(""));
        let base = self
            .root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let path: PathBuf = parent.join(format!("{base}-worktrees")).join(id);
        path.to_string_lossy().replace('\\', "/")
    }

    /// Schaltet das Plugin im neuen Worktree ein (die lokale Einstellung liegt nicht im Git);
    /// Fehler sind nur eine Warnung
    fn enable_plugin(&self, path: &Path) -> Option<String> {
        match self.plugin_switch.enable(path) {
            Ok(_) => None,
            Err(error) => Some(format!(
                "Das Plugin konnte im Worktree nicht eingeschaltet werden: {error}"
            )),
        }
    }

    /// Legt die Rules für Doku- und Board-Regeln im neuen Worktree an (liegen wie die
    /// Einstellung nicht im Git) und gibt ihre Importe für den Worktree frei, der für Claude
    /// Code ein eigenes Projekt ist; Fehler sind nur eine Warnung
    fn add_rules(&self, path: &Path) -> Option<String> {
        let written = self
            .rules
            .ensure(path)
            .and_then(|_| self.import_approval.approve(&path.to_string_lossy()));
        match written {
            Ok(_) => None,
            Err(error) => Some(format!(
                "Die Rules für Doku- und Board-Regeln konnten im Worktree nicht angelegt \
werden:{error}"
            )),
        }
    }

    /// Prüft erster Commit, freien Branch-Namen und freien Ordner, bevor etwas angelegt wird
    fn assert_ready(&self, branch: &str, path: &str, cwd: &Path) -> Result<()> {
        if !self.git.try_run(cwd, &["rev-parse", "--verify", "HEAD"]).ok {
            return Err(HarnessError::new(
                "no-commit",
                "Bitte zuerst einen Commit anlegen, sonst kann kein Worktree entstehen.",
            )
            .into());
        }
        if self.branch_exists(branch, cwd) {
            return Err(HarnessError::new(
                "branch-exists",
                format!("Branch {branch} existiert bereits."),
            )
            .into());
        }
        if Path::new(path).exists() {
            return Err(HarnessError::new(
                "worktree-exists",
                format!("Der Ordner {path} existiert bereits."),
            )
            .into());
        }
        Ok(())
    }

    /// Gibt es den Branch schon?
    fn branch_exists(&self, branch: &str, cwd: &Path) -> bool {
        self.git
            .try_run(
                cwd,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{branch}"),
                ],
            )
            .ok
    }

    /// „Bereit zum Merge“, wenn alle Tickets des Worktrees fertig sind; gebündelt
    /// (`SameSpec`) zusätzlich erst, wenn kein Ticket derselben Spec mehr auf den Start
    /// wartet. Schreibt das Verzeichnis nur bei einer Änderung
    async fn update_ready(&self, branch: &str) -> Result<bool> {
        let Some(entry) = self.registry.read()?.get(branch).cloned() else {
            return Ok(false);
        };
        // Nur Tickets, die noch auf diesem Branch arbeiten (nach --clear-branch oder Wechsel
        // zählen sie nicht mehr)
        let all: Vec<Ticket> = self
            .load(&entry.tickets)
            .await?
            .into_iter()
            .filter(|ticket| ticket.branch.as_deref() == Some(branch))
            .collect();
        // Ein vom Merge blockiertes Ticket („Merge: …“) ist fertig – sonst wartet
        // „Erneut mergen“ auf sich selbst
        let done = !all.is_empty()
            && all.iter().all(|ticket| {
                finished(ticket.status)
                    || (ticket.status == TicketStatus::Blocked
                        && ticket
                            .blocked_reason
                            .as_deref()
                            .unwrap_or("")
                            .starts_with(MERGE_REASON_PREFIX))
            });
        let ready = done
            && !(self.join_worktrees == JoinWorktrees::SameSpec
                && self.has_waiting_tickets(&all).await?);
        if entry.ready_to_merge != ready {
            self.registry.set_ready(branch, ready)?;
        }
        Ok(ready)
    }

    /// Wartet auf den Boards dieser Tickets in einer ihrer Specs noch ein ungestartetes,
    /// offenes Ticket?
    async fn has_waiting_tickets(&self, in_worktree: &[Ticket]) -> Result<bool> {
        let specs: BTreeSet<String> = in_worktree
            .iter()
            .filter_map(|ticket| ticket.spec.clone())
            .collect();
        if specs.is_empty() {
            return Ok(false);
        }
        let board_ids: Vec<i32> = in_worktree
            .iter()
            .map(|ticket| ticket.board_id)
            .collect::<BTreeSet<i32>>()
            .into_iter()
            .collect();
        let filter = TicketFilter {
            board_ids: Some(board_ids),
            ids: None,
            without_texts: false,
        };
        let tickets = tx!(self.services.db, [filter], |u| {
            u.tickets().list(&filter).await
        })
        .await?;
        Ok(tickets.iter().any(|ticket| {
            ticket
                .spec
                .as_ref()
                .is_some_and(|spec| specs.contains(spec))
                && ticket.branch.is_none()
                && !finished(ticket.status)
        }))
    }

    /// Lädt Tickets zu IDs in deren Reihenfolge; gelöschte fehlen
    async fn load(&self, ids: &[String]) -> Result<Vec<Ticket>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let filter = TicketFilter {
            board_ids: None,
            ids: Some(ids.to_vec()),
            without_texts: false,
        };
        let found = tx!(self.services.db, [filter], |u| {
            u.tickets().list(&filter).await
        })
        .await?;
        Ok(ids
            .iter()
            .filter_map(|id| found.iter().find(|ticket| &ticket.id == id).cloned())
            .collect())
    }
}

/// Haben beide Tickets denselben Zielbranch (kein Zielbranch zählt als gleich)?
fn same_target(a: &Ticket, b: &Ticket) -> bool {
    a.target_branch == b.target_branch
}

/// Hinweise zu einem Text zusammengefasst; ohne Hinweis bleibt es bei `None`
fn join_warnings(warnings: &[String]) -> Option<String> {
    let text = warnings.join(" ");
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests;

/// Ordner ohne jeden Eintrag (auch keine versteckten Dateien)
fn is_empty_dir(path: &Path) -> bool {
    match fs::read_dir(path) {
        Ok(mut entries) => entries.next().is_none(),
        Err(_) => false,
    }
}
