//! Der Dispatcher im Server: startet Claude und die Einrichtungsbefehle eines Worktrees,
//! liest ihre Ausgabe mit und wertet die Antwort von Claude aus. Dazu der losgelöste
//! Worker eines Laufs (`worker`) und sein Start als eigener Prozess (`worker_launcher`).
//! Darüber der Dispatch-Service selbst (`service`) mit Zustand (`status`) und Takt
//! (`scheduler`), der Merge erledigter Tickets (`merge_service`) und die Freigabe vor dem
//! Merge (`review_service`).

pub mod account_pool;
pub mod claude_process;
pub mod claude_runner;
pub mod context;
pub mod merge_git;
pub mod merge_result;
pub mod merge_service;
pub mod merge_step;
pub mod notifier;
pub mod plan_runner;
pub mod prompt_catalog;
pub mod prompt_values;
pub mod research_runner;
pub mod result_envelope;
pub mod result_handler;
pub mod review_service;
pub mod run_recorder;
pub mod scheduler;
pub mod service;
pub mod status;
pub mod suggestion_analyzer;
pub mod suggestion_service;
pub mod ticket_starter;
pub mod worker;
pub mod worker_launcher;
pub mod worktree_setup;

pub use account_pool::{
    side_run_account, side_run_account_with, AccountPool, RunAccounts, SideRunAccount,
    StoredAccountPool,
};
pub use claude_process::{ClaudeProcess, ClaudeRunInput, OutputSource, RunResult};
pub use claude_runner::{ClaudeRunner, ClaudeRunners, FixedClaudeRunner, ProcessClaudeRunner};
pub use context::{DispatchContext, DispatchStores, LaunchInput, RepoWork};
pub use merge_git::{MergeGit, RealMergeGit};
pub use merge_result::{MergeResultReader, MergeRetryOutcome, MergeRetryResult};
pub use merge_service::MergeService;
pub use merge_step::{AcceptOutcome, MergeStep, MergeStepReport, NoMergeStep};
pub use notifier::{
    DispatchNotice, DispatchNoticeKind, DispatchNotify, NoDispatchNotify, ServerDispatchNotify,
};
pub use plan_runner::{PlanAutoReport, PlanOutcome, PlanRunner, PLANNING_RETRY_MINUTES};
pub use prompt_catalog::{DispatchPromptInfo, PromptCatalog};
pub use prompt_values::{PromptValues, SampleValues};
pub use research_runner::{
    ResearchAutoReport, ResearchOutcome, ResearchRunner, StartMode, RESEARCH_RETRY_MINUTES,
};
pub use result_envelope::{ClaudeResultEnvelope, ClaudeResultText};
pub use result_handler::{
    MergeSessionEnd, NoMergeSessionEnd, NoReviewGate, ResultHandler, ReviewGate,
};
pub use review_service::{
    ReturnedToAgent, ReviewFilePatch, ReviewService, ReviewView, MAX_RETURN_TEXT,
};
pub use run_recorder::{AbortInfo, HandleOutcome, HandleResult, RunRecorder, StartFailed};
pub use scheduler::{DispatchScheduler, DispatchTick, LogLine, STOP_WAIT};
pub use service::{
    AliveCheck, DispatchReport, DispatchService, DispatchServiceDeps, EvaluatedRun,
    MergeRetryReport, PromptPreview, PromptPreviewInput,
};
pub use status::{
    DispatchMergeInfo, DispatchPlanningInfo, DispatchResearchInfo, DispatchRunInfo, DispatchRunLog,
    DispatchRunLogText, DispatchStateKind, DispatchStatus, DispatchTicketRow,
    QuestionResearchStatus, TicketPlanningStatus,
};
pub use suggestion_analyzer::SuggestionAnalyzer;
pub use suggestion_service::{
    AnalysisDto, ApplyResult, DispatchSuggestionService, SuggestionContext, SuggestionListDto,
};
pub use ticket_starter::{StartOutcome, TicketStarter};
pub use worker::DispatchWorker;
pub use worker_launcher::{NoWorkerLauncher, ProcessWorkerLauncher, WorkerLauncher};
pub use worktree_setup::{
    SetupFailure, SetupInput, SetupOutcome, WorktreeSetupRunner, WorktreeSetupStep,
};
