//! Der Dispatcher im Server: startet Claude und die Einrichtungsbefehle eines Worktrees,
//! liest ihre Ausgabe mit und wertet die Antwort von Claude aus. Dazu der losgelöste
//! Worker eines Laufs (`worker`) und sein Start als eigener Prozess (`worker_launcher`).

pub mod claude_process;
pub mod context;
pub mod notifier;
pub mod prompt_values;
pub mod result_envelope;
pub mod result_handler;
pub mod run_recorder;
pub mod ticket_starter;
pub mod worker;
pub mod worker_launcher;
pub mod worktree_setup;

pub use claude_process::{ClaudeProcess, ClaudeRunInput, OutputSource, RunResult};
pub use context::{DispatchContext, DispatchStores, LaunchInput, RepoWork};
pub use notifier::{DispatchNotice, DispatchNoticeKind, DispatchNotify, NoDispatchNotify};
pub use prompt_values::{PromptValues, SampleValues};
pub use result_envelope::{ClaudeResultEnvelope, ClaudeResultText};
pub use result_handler::{
    MergeSessionEnd, NoMergeSessionEnd, NoReviewGate, ResultHandler, ReviewGate,
};
pub use run_recorder::{AbortInfo, HandleOutcome, HandleResult, RunRecorder, StartFailed};
pub use ticket_starter::{StartOutcome, TicketStarter};
pub use worker::DispatchWorker;
pub use worker_launcher::{ProcessWorkerLauncher, WorkerLauncher};
pub use worktree_setup::{
    SetupFailure, SetupInput, SetupOutcome, WorktreeSetupRunner, WorktreeSetupStep,
};
