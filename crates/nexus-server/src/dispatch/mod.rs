//! Der Dispatcher im Server: startet Claude und die Einrichtungsbefehle eines Worktrees,
//! liest ihre Ausgabe mit und wertet die Antwort von Claude aus. Dazu der losgelöste
//! Worker eines Laufs (`worker`) und sein Start als eigener Prozess (`worker_launcher`).

pub mod claude_process;
pub mod claude_runner;
pub mod result_envelope;
pub mod worker;
pub mod worker_launcher;
pub mod worktree_setup;

pub use claude_process::{ClaudeProcess, ClaudeRunInput, OutputSource, RunResult};
pub use claude_runner::{
    analysis_dir, ClaudeRunner, ClaudeRunners, FixedClaudeRunner, ProcessClaudeRunner,
};
pub use result_envelope::{ClaudeResultEnvelope, ClaudeResultText};
pub use worker::DispatchWorker;
pub use worker_launcher::{ProcessWorkerLauncher, WorkerLauncher};
pub use worktree_setup::{
    SetupFailure, SetupInput, SetupOutcome, WorktreeSetupRunner, WorktreeSetupStep,
};
