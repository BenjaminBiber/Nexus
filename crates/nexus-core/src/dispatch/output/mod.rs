//! Auswertung der Ausgabe eines Dispatcher-Laufs: lesbare Protokollzeilen, Zusammenfassung,
//! Verbrauch, Abbruchgründe, Testläufe und Kategorien. Port von `StreamEventFormatter`,
//! `RunOutputReader`, `ShellCommandSummary`, `TestRunExtractor` und `AbortReasonClassifier`
//! aus `packages/core/src/dispatch`.
//!
//! Alles hier sind reine Funktionen über Text: der Prozessstart und der Worker-Prozess
//! kommen getrennt. Protokollformat und Abbruchtexte sind Vertrag – die Oberfläche liest die
//! Art in eckigen Klammern und den Tabulator vor dem Detail, die Lauf-Statistik die
//! Abbruchgründe.

pub mod abort_category;
pub mod run_artifacts;
pub mod run_output;
pub mod shell_summary;
pub mod stream_formatter;
pub mod test_runs;
pub(crate) mod text;

pub use abort_category::{AbortCategory, AbortReasonClassifier, ABORT_CATEGORIES};
pub use run_artifacts::{RunArtifact, RunArtifactScanner};
pub use run_output::{ModelUsage, OutputSummary, RunOutputReader, RunUsage};
pub use shell_summary::{ShellCommandSummary, ShellKind, ShellSummary};
pub use stream_formatter::{RunLogLine, RunLogLineKind, StreamEventFormatter};
pub use test_runs::TestRunExtractor;
