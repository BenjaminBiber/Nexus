//! Der Dispatcher im Server: startet Claude und die Einrichtungsbefehle eines Worktrees,
//! liest ihre Ausgabe mit und wertet die Antwort von Claude aus.

pub mod claude_process;
pub mod result_envelope;
pub mod worktree_setup;

pub use claude_process::{ClaudeProcess, ClaudeRunInput, OutputSource, RunResult};
pub use result_envelope::{ClaudeResultEnvelope, ClaudeResultText};
pub use worktree_setup::{
    SetupFailure, SetupInput, SetupOutcome, WorktreeSetupRunner, WorktreeSetupStep,
};
