//! Anpassungen eines Clones: eigener Workflow, eigene Skills und Output-Styles unter
//! `custom/`. Prüfen, anwenden, kopieren und zurücksetzen. Portiert aus
//! `packages/core/src/customization`; die Dateiformate (`customizations/overview.json`,
//! `workflow.json`, `custom/.base/`) bleiben unverändert, weil Hook, Server und Oberfläche
//! sie lesen.

pub mod applier;
pub mod args;
pub mod base_snapshot;
pub mod clone_workflow_tools;
pub mod copier;
pub mod front_matter;
pub mod name_rules;
pub mod output_style;
pub mod overview;
pub mod overview_file;
pub mod overview_formatter;
pub mod paths;
pub mod skill;
pub mod tools;
pub mod workflow;
pub mod workflow_validator;

pub use applier::{CheckResult, CustomizationApplier, CustomizationInput};
pub use args::{CustomizeArgs, CustomizeCommand, NEXUS_USAGE, NPM_USAGE};
pub use base_snapshot::{BaseCheck, BaseSnapshot};
pub use clone_workflow_tools::CloneWorkflowTools;
pub use copier::{CopyTarget, CustomizationCopier};
pub use front_matter::{FrontMatter, FrontMatterResult};
pub use name_rules::NameRules;
pub use output_style::OutputStyleCustomization;
pub use overview::{
    BaseState, CustomizationEntry, CustomizationOverview, EntryBase, EntryKind, EntryState,
    ExtensionPoint, Problem, WorkflowState, CUSTOMIZE_COMMAND,
};
pub use overview_file::OverviewFile;
pub use overview_formatter::{label, OverviewFormatter, DEFAULT_MAX_DIFF_LINES};
pub use paths::CustomizationPaths;
pub use skill::SkillCustomization;
pub use tools::WorkflowTools;
pub use workflow::{WorkflowCustomization, WorkflowResult};
pub use workflow_validator::{WorkflowValidation, WorkflowValidator};
