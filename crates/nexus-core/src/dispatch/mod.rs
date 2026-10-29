//! Der Dispatcher: Einstellung samt Prüfung und Fingerabdruck, die Prompt-Vorlagen, die
//! Bereitschaftsprüfung, die Profilwahl sowie Argumente und Umgebung für `claude -p`.
//! Dazu die Vorsilben seiner Kommentare und die Git-Bausteine für den Merge.

pub mod abort_reason;
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
pub mod profile_chooser;
pub mod prompt;
pub mod prompt_check;
pub mod prompt_defaults;
pub mod prompt_template;
pub mod readiness;
pub mod review_feedback;
pub mod run_backfill;
pub mod time_window;

pub use abort_reason::{AbortCategory, AbortReasonClassifier, ABORT_CATEGORIES};
pub use attempts::AttemptCounter;
pub use comments::CommentLike;
pub use config::{
    limit_pause, ClaudeSettings, DispatchConfig, JoinWorktrees, PlanningSettings, Profile,
    PromptProblem, PromptTemplates, ResearchSettings, Rule, RuleCondition, RunSettings, TimeWindow,
    WorktreeSetupStep, JOIN_WORKTREES, MERGE_REASON_PREFIX, SETUP_REASON_PREFIX,
};
pub use config_normalizer::DispatchConfigNormalizer;
pub use config_store::{DispatchConfigRead, DispatchConfigStore};
pub use fingerprint::DispatchConfigFingerprint;
pub use git_bash::{EnvLookup, GitBashLocator, ProcessEnv};
pub use invocation::{ClaudeInvocation, RunKind};
pub use merge_strategy::MergeStrategy;
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
pub use time_window::TimeWindowCheck;
