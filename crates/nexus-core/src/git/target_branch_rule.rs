//! Portiert `packages/core/src/git/TargetBranchRule.ts`.

use crate::board::basics::MAX_BRANCH_LENGTH;
use crate::board::limits::BoardLimits;
use crate::error::{HarnessError, Result};

/// Regeln für den Namen eines Zielbranches, nachgebildet nach
/// „git check-ref-format --branch“, aber ohne Git-Aufruf.
#[derive(Debug, Clone, Copy, Default)]
pub struct TargetBranchRule;

impl TargetBranchRule {
    /// Neue Regel (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Liefert den getrimmten Namen, `None` bei leerem Wert; der Fehler ist
    /// „target-branch-invalid“ oder „value-too-long“
    pub fn normalize(&self, value: Option<&str>) -> Result<Option<String>> {
        let name = value.unwrap_or("").trim();
        if name.is_empty() {
            return Ok(None);
        }
        BoardLimits::text(name, MAX_BRANCH_LENGTH, "Der Zielbranch")?;
        if !is_valid_name(name) {
            return Err(HarnessError::new(
                "target-branch-invalid",
                format!("„{name}“ ist kein gültiger Branch-Name."),
            )
            .into());
        }
        // „ticket“ kollidiert mit refs/heads/ticket/*, „Ticket/…“ trifft unter Windows
        // die Branches der Tickets
        if is_ticket_namespace(name) {
            return Err(HarnessError::new(
                "target-branch-invalid",
                format!("„{name}“ ist der Branch eines Tickets; dorthin wird nicht gemergt."),
            )
            .into());
        }
        Ok(Some(name.to_string()))
    }
}

/// Liegt der Name im Namensraum der Ticket-Branches (ohne Groß-/Kleinschreibung)?
fn is_ticket_namespace(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower == "ticket" || lower.starts_with("ticket/")
}

/// Erfüllt der Name die Namensregeln von Git?
fn is_valid_name(name: &str) -> bool {
    if name.starts_with('-') || name == "@" || name == "HEAD" {
        return false;
    }
    // Leerzeichen, Steuerzeichen und die Zeichen ~ ^ : ? * [ \ sind in Branch-Namen
    // verboten (git check-ref-format)
    let forbidden = name.chars().any(|c| {
        c <= ' ' || c == '\u{7f}' || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
    });
    if forbidden || name.contains("..") || name.contains("@{") {
        return false;
    }
    if name.ends_with('.') {
        return false;
    }
    // Jeder Abschnitt zwischen den Schrägstrichen: nicht leer (deckt „//“ und
    // Schrägstrich am Anfang/Ende ab), kein Punkt am Anfang, nicht „.lock“ am Ende
    name.split('/').all(|section| {
        !section.is_empty() && !section.starts_with('.') && !section.ends_with(".lock")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Portiert aus `packages/core/tests/unit/git/TargetBranchRule.test.ts`
    #[test]
    fn lasst_gultige_namen_durch_und_leert_leerraum() {
        let rule = TargetBranchRule::new();

        assert_eq!(
            rule.normalize(Some(" release/2026.10 ")).unwrap(),
            Some("release/2026.10".into())
        );
        assert_eq!(
            rule.normalize(Some("feature/zielbranch")).unwrap(),
            Some("feature/zielbranch".into())
        );
        assert_eq!(rule.normalize(Some("")).unwrap(), None);
        assert_eq!(rule.normalize(Some("   ")).unwrap(), None);
        assert_eq!(rule.normalize(None).unwrap(), None);
    }

    #[test]
    fn lehnt_ungultige_namen_ab() {
        let rule = TargetBranchRule::new();
        let zu_lang = "x".repeat(401);
        let names = [
            "-x",
            "a..b",
            "a b",
            "a~b",
            "a^b",
            "a:b",
            "a?b",
            "a*b",
            "a[b",
            "a\\b",
            "a@{b",
            "@",
            ".a",
            "a/.b",
            "a.lock",
            "a/b.lock/c",
            "a/",
            "a.",
            "a//b",
            "HEAD",
            "ticket/ARC-0001-x",
            zu_lang.as_str(),
        ];

        for name in names {
            let code = rule.normalize(Some(name)).unwrap_err().code().map(String::from);
            assert!(
                matches!(
                    code.as_deref(),
                    Some("target-branch-invalid" | "value-too-long")
                ),
                "„{name}“ sollte abgelehnt werden, war aber {code:?}"
            );
        }
    }

    #[test]
    fn nennt_bei_ticket_den_grund() {
        let error = TargetBranchRule::new()
            .normalize(Some("ticket/ARC-1"))
            .unwrap_err();

        assert!(error.to_string().contains("Branch eines Tickets"));
    }

    /// „ticket“ kollidiert mit refs/heads/ticket/*, „Ticket/…“ trifft unter Windows die
    /// Branches der Tickets
    #[test]
    fn lehnt_den_namensraum_der_tickets_ohne_rucksicht_auf_schreibweise_ab() {
        let rule = TargetBranchRule::new();

        for name in ["ticket", "Ticket/ARC-1", "TICKET/x"] {
            let error = rule.normalize(Some(name)).unwrap_err();
            assert_eq!(error.code(), Some("target-branch-invalid"));
            assert!(error.to_string().contains("Branch eines Tickets"));
        }
    }

    #[test]
    fn lasst_ahnliche_namen_durch() {
        let rule = TargetBranchRule::new();

        for name in ["tickets/x", "my-ticket/x"] {
            assert_eq!(rule.normalize(Some(name)).unwrap(), Some(name.to_string()));
        }
    }
}
