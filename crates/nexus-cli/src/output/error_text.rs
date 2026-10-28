//! Text eines erwartbaren Fehlers für die Kommandozeile.
//! Portiert aus `packages/cli/src/output/CliErrorText.ts`.

use nexus_core::HarnessError;

/// Sperren, die sich in der CLI mit `--force` übergehen lassen, und der Hinweis dazu; die Meldung aus dem
/// Kern bleibt neutral (auch fürs Web)
const FORCE_HINTS: [(&str, &str); 3] = [
    ("question-open", "Mit --force übergehen."),
    ("dependency-open", "Mit --force übergehen."),
    ("ticket-duplicate", "Mit --force trotzdem anlegen."),
];

/// Text eines erwartbaren Fehlers für die Kommandozeile: Kernmeldung plus CLI-eigene Hinweise.
#[derive(Debug, Clone, Copy, Default)]
pub struct CliErrorText;

impl CliErrorText {
    /// Meldung mit CLI-Hinweisen, ohne Code
    pub fn of(&self, error: &HarnessError) -> String {
        match FORCE_HINTS.iter().find(|(code, _)| *code == error.code) {
            Some((_, hint)) => format!("{} {hint}", error.message),
            None => error.message.clone(),
        }
    }

    /// Klartext für stderr: Meldung und Code in eckigen Klammern, z. B. „… [board-repo-mismatch]“
    pub fn line(&self, error: &HarnessError) -> String {
        format!("{} [{}]", self.of(error), error.code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erganzt_bei_offener_frage_oder_voraussetzung_den_hinweis_auf_force() {
        let text = CliErrorText;
        assert_eq!(
            text.of(&HarnessError::new(
                "question-open",
                "ARC-0001 wartet auf Frage ARC-F0001. Erst die Frage entscheiden lassen."
            )),
            "ARC-0001 wartet auf Frage ARC-F0001. Erst die Frage entscheiden lassen. Mit --force übergehen."
        );
        assert_eq!(
            text.of(&HarnessError::new(
                "dependency-open",
                "ARC-0002 wartet auf ARC-0001. Erst die Voraussetzung erledigen."
            )),
            "ARC-0002 wartet auf ARC-0001. Erst die Voraussetzung erledigen. Mit --force übergehen."
        );
    }

    #[test]
    fn erganzt_bei_einer_dublette_den_hinweis_dass_force_trotzdem_anlegt() {
        assert_eq!(
            CliErrorText.line(&HarnessError::new(
                "ticket-duplicate",
                "Ein offenes Ticket mit diesem Titel gibt es schon: ARC-0012."
            )),
            "Ein offenes Ticket mit diesem Titel gibt es schon: ARC-0012. Mit --force trotzdem anlegen. [ticket-duplicate]"
        );
    }

    #[test]
    fn lasst_andere_meldungen_unverandert() {
        assert_eq!(
            CliErrorText.of(&HarnessError::new("conflict", "Bitte neu lesen.")),
            "Bitte neu lesen."
        );
    }

    #[test]
    fn hangt_fur_den_klartext_den_code_in_eckigen_klammern_an() {
        assert_eq!(
            CliErrorText.line(&HarnessError::new("board-repo-mismatch", "Falsches Board.")),
            "Falsches Board. [board-repo-mismatch]"
        );
        assert_eq!(
            CliErrorText.line(&HarnessError::new(
                "dependency-open",
                "ARC-0002 wartet auf ARC-0001."
            )),
            "ARC-0002 wartet auf ARC-0001. Mit --force übergehen. [dependency-open]"
        );
    }
}
