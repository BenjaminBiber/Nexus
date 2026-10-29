//! Tests des Worktree-Verwalters mit echten Git-Repos in temporären Ordnern.
//! Portiert aus `packages/core/tests/unit/worktrees/WorktreeManager.test.ts` und
//! `WorktreeManagerDispatch.test.ts`; je Modul eine der Testdateien aus TypeScript.

mod dispatch;
mod start_done;

use std::path::Path;

use tempfile::TempDir;

use crate::board::in_memory::InMemoryBoardDatabase;
use crate::board::services::BoardServices;
use crate::board::ticket_service::{NewTicketInput, TicketPatch, TicketView, WriteOptions};
use crate::dispatch::config::JoinWorktrees;
use crate::error::Result;
use crate::install::ClaudeImportApproval;
use crate::project::HarnessProject;
use crate::registry::HarnessHome;
use crate::test_support::{make_temp_dir, TempRepo};
use crate::testing::BoardFixture;
use crate::worktrees::manager::{
    FinishOptions, FinishResult, StartOptions, StartResult, WorktreeManager, WorktreeOptions,
};

/// Ein Repo mit eingerichtetem Harness-Ordner, den Diensten eines Boards im Speicher und
/// einem Verwalter darauf. Benutzerordner und Claude-Konfiguration liegen im Temp-Ordner,
/// damit die Tests den Rechner des Nutzers nicht anfassen.
pub(super) struct Env {
    pub repo: TempRepo,
    pub f: BoardFixture,
    pub manager: WorktreeManager<InMemoryBoardDatabase>,
    /// Benutzerordner (`HARNESS_HOME`) und Ordner der Claude-Konfiguration der Tests
    home: TempDir,
    claude: TempDir,
}

impl Env {
    /// Umgebung mit einem Worktree je Ticket
    pub async fn new() -> Self {
        Self::with_join(JoinWorktrees::None).await
    }

    /// Umgebung mit gebündelten Worktrees (`joinWorktrees: 'sameSpec'`)
    pub async fn bundled() -> Self {
        Self::with_join(JoinWorktrees::SameSpec).await
    }

    /// Umgebung mit dieser Worktree-Einstellung
    async fn with_join(join: JoinWorktrees) -> Self {
        let repo = TempRepo::new();
        let home = make_temp_dir("harness-home-");
        let claude = make_temp_dir("harness-claude-");
        let f = BoardFixture::create().await;
        let manager = manager_for(repo.path(), &f, home.path(), claude.path(), join);
        Self {
            repo,
            f,
            manager,
            home,
            claude,
        }
    }

    /// Ein Verwalter auf einem anderen Repo, aber denselben Diensten
    pub fn manager_on(&self, repo: &Path) -> WorktreeManager<InMemoryBoardDatabase> {
        manager_for(
            repo,
            &self.f,
            self.home.path(),
            self.claude.path(),
            JoinWorktrees::None,
        )
    }

    /// Die Konfigurationsdatei von Claude Code dieser Tests
    pub fn claude_config_file(&self) -> std::path::PathBuf {
        self.claude.path().join(".claude.json")
    }

    /// Legt ein Ticket auf dem Board der Testumgebung an
    pub async fn ticket(&self, input: NewTicketInput) -> TicketView {
        self.f
            .tickets
            .create(
                self.f.board_id,
                &input,
                &self.f.actor,
                &WriteOptions::default(),
            )
            .await
            .expect("Ticket anlegen geht immer")
    }

    /// Ändert ein Ticket mit dem Handelnden der Umgebung
    pub async fn set(&self, id: &str, patch: TicketPatch) -> Result<TicketView> {
        self.f
            .tickets
            .update(id, &patch, &self.f.actor, &WriteOptions::default())
            .await
    }

    /// Git im Worktree eines Pfades, mit Erfolg erwartet
    pub fn git_in(&self, cwd: &Path, args: &[&str]) -> String {
        let mut all: Vec<&str> = vec!["-C"];
        let cwd = cwd.to_string_lossy().into_owned();
        all.push(&cwd);
        all.extend(args);
        self.repo.git(&all)
    }
}

/// Verwalter für dieses Repo (Harness-Ordner wird dabei eingerichtet)
fn manager_for(
    repo: &Path,
    f: &BoardFixture,
    home: &Path,
    claude: &Path,
    join: JoinWorktrees,
) -> WorktreeManager<InMemoryBoardDatabase> {
    let project = HarnessProject::init(repo).expect("Projekt einrichten geht immer");
    let services = BoardServices::new(f.db.clone(), f.clock.clone(), f.repo_paths.clone());
    WorktreeManager::new(
        project.layout.clone(),
        services,
        project.root.clone(),
        WorktreeOptions {
            join_worktrees: join,
            home: Some(HarnessHome::at(home)),
            approval: Some(ClaudeImportApproval::new(
                Some(claude.to_string_lossy().into_owned()),
                home.to_path_buf(),
                cfg!(windows),
            )),
        },
    )
}

/// Ein Ticket mit Titel und betroffenen Bereichen
pub(super) fn touching(title: &str, touches: &[&str]) -> NewTicketInput {
    NewTicketInput {
        touches: Some(touches.iter().map(|path| path.to_string()).collect()),
        ..NewTicketInput::titled(title)
    }
}

/// Ein Ticket einer Spec mit betroffenen Bereichen
pub(super) fn in_spec(title: &str, touches: &[&str], spec: &str, task: &str) -> NewTicketInput {
    NewTicketInput {
        spec: Some(spec.to_string()),
        spec_task: Some(task.to_string()),
        ..touching(title, touches)
    }
}

/// Startet ein Ticket im Hauptordner ohne besondere Angaben
pub(super) async fn start(e: &Env, id: &str) -> Result<StartResult> {
    e.manager
        .start(id, &StartOptions::default(), e.repo.path(), &e.f.actor)
        .await
}

/// Startet ein Ticket im Hauptordner mit diesen Angaben
pub(super) async fn start_with(e: &Env, id: &str, options: StartOptions) -> Result<StartResult> {
    e.manager
        .start(id, &options, e.repo.path(), &e.f.actor)
        .await
}

/// Schließt ein Ticket ab; `cwd` ist der Ordner, aus dem der Abschluss kommt
pub(super) async fn finish_in(
    e: &Env,
    id: &str,
    acceptance: Option<&str>,
    cwd: &Path,
) -> Result<FinishResult> {
    e.manager
        .finish(
            id,
            &FinishOptions {
                acceptance: acceptance.map(|text| text.to_string()),
                ..FinishOptions::default()
            },
            cwd,
            &e.f.actor,
        )
        .await
}

/// Schließt ein Ticket aus dem Hauptordner ab
pub(super) async fn finish(e: &Env, id: &str, acceptance: Option<&str>) -> Result<FinishResult> {
    let cwd = e.repo.path().to_path_buf();
    finish_in(e, id, acceptance, &cwd).await
}

/// Fehlercode eines erwarteten Fehlers
pub(super) fn code<T>(result: Result<T>) -> String {
    result
        .err()
        .expect("der Aufruf muss scheitern")
        .code()
        .unwrap_or_default()
        .to_string()
}
