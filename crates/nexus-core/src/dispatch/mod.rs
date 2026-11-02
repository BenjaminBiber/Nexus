//! Der Dispatcher: Einstellung samt Prüfung und Fingerabdruck, die Prompt-Vorlagen, die
//! Bereitschaftsprüfung, die Profilwahl sowie Argumente und Umgebung für `claude -p`.
//! Dazu die Vorsilben seiner Kommentare, die Git-Bausteine für den Merge und die Auswertung
//! der Ausgabe eines Laufs (`output`) und der Vergleich des Parallelbetriebs (`shadow`).

pub mod abort_reason;
pub mod account_chooser;
pub mod account_references;
pub mod attempts;
pub mod comments;
pub mod config;
pub mod config_normalizer;
pub mod config_store;
pub mod fingerprint;
pub mod git;
pub mod git_bash;
pub mod invocation;
mod js_value;
pub mod markers;
pub mod merge_strategy;
pub mod optimization;
pub mod output;
pub mod profile_chooser;
pub mod prompt;
pub mod prompt_check;
pub mod prompt_defaults;
pub mod prompt_template;
pub mod readiness;
pub mod review_feedback;
pub mod run_backfill;
pub mod state;
pub mod time_window;
pub mod types;

pub use abort_reason::{AbortCategory, AbortReasonClassifier, ABORT_CATEGORIES};
pub use account_chooser::{hhmm, AccountChoice, AccountChoiceInput, AccountChooser, AccountState};
pub use account_references::AccountReferences;
pub use attempts::{limit_account, AttemptCounter};
pub use comments::CommentLike;
pub use config::{
    limit_pause, AccountStrategy, ClaudeSettings, DispatchConfig, JoinWorktrees, PlanningSettings,
    Profile, PromptProblem, PromptTemplates, ResearchSettings, Rule, RuleCondition, RunSettings,
    TimeWindow, WorktreeSetupStep, ACCOUNT_STRATEGIES, JOIN_WORKTREES, MERGE_REASON_PREFIX,
    SETUP_REASON_PREFIX,
};
pub use config_normalizer::DispatchConfigNormalizer;
pub use config_store::{DispatchConfigRead, DispatchConfigStore};
pub use fingerprint::DispatchConfigFingerprint;
pub use git_bash::{EnvLookup, GitBashLocator, ProcessEnv};
pub use invocation::{ClaudeInvocation, RunKind};
pub use merge_strategy::MergeStrategy;
pub use optimization::{
    sorted_json, suggestion_id, AdvisorResult, AnalysisWorkspace, ConfigPatchOp, ConfigPatcher,
    EscalationPath, FactsInput, InsufficientData, OptimizationAdvisor, OptimizationBasis,
    OptimizationFacts, OptimizationFactsCollector, ProfileFacts, RuleFacts, StartSource,
    StoredAnalysis, Suggestion, SuggestionEvidence, SuggestionKind, SuggestionSource,
    SuggestionState, SuggestionStateStore, SuggestionTarget, MIN_RUNS,
};
pub use output::{
    ModelUsage, OutputSummary, RunLogLine, RunLogLineKind, RunOutputReader, RunUsage,
    ShellCommandSummary, ShellKind, ShellSummary, StreamEventFormatter, TestRunExtractor,
};
pub use profile_chooser::{DispatchLabels, ProfileChooser, ProfileTicket};
pub use prompt::{
    AnsweredQuestion, DispatchPrompt, MergePromptInput, PlanPromptInput, ResearchPromptInput,
    WorkPromptInput,
};
pub use prompt_check::{PromptKind, PromptTemplateCheck, RequiredPart};
pub use prompt_defaults::PromptPlaceholder;
pub use prompt_template::{PromptTemplate, PromptValue, PromptValues};
pub use readiness::{DispatchReadiness, ReadinessBoard, ReadinessTicket};
pub use review_feedback::ReviewFeedback;
pub use run_backfill::{BackfillInput, DispatchRunBackfill};
pub use state::{
    DispatchLog, DispatchState, DispatchStateStore, MergeEntry, MergePhase, PlanningAutoState,
    PlanningEntry, ResearchAutoState, ResearchEntry, ReviewEntry, ReviewStore, RunEnd, RunLogEntry,
    RunLogOutcome, RunLogParts, RunLogSplitter, RunOutcome, RunRecord, RunState, RunStore,
    RUN_LOG_OUTPUT_MARKER, RUN_LOG_PROMPT_MARKER,
};
pub use time_window::TimeWindowCheck;
pub use types::{RunResult, SetupFailure, TestRun};
