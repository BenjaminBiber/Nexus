//! Portiert `packages/core/src/customization/CustomizeArgs.ts`.

use crate::error::{HarnessError, Result};

use super::copier::CopyTarget;
use super::overview::ExtensionPoint;

/// Aufruftext des Befehls
pub const NEXUS_USAGE: &str = "Aufruf: nexus customize check | copy workflow|skill <name>|output-style <name> [--rebase] | reset workflow|skill <name>|output-style <name>";

/// Befehl mit Ziel, wie ihn die Argumente nennen
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomizeCommand {
    Check,
    Copy { target: CopyTarget, rebase: bool },
    Reset { target: CopyTarget },
}

/// Liest die Argumente von `nexus customize …`.
#[derive(Debug, Clone)]
pub struct CustomizeArgs {
    usage: String,
}

impl Default for CustomizeArgs {
    fn default() -> Self {
        Self::new()
    }
}

impl CustomizeArgs {
    /// Leser mit dem Aufruftext des Rust-Befehls
    pub fn new() -> Self {
        Self::with_usage(NEXUS_USAGE)
    }

    /// Leser mit einem eigenen Aufruftext
    pub fn with_usage(usage: impl Into<String>) -> Self {
        Self {
            usage: usage.into(),
        }
    }

    /// Befehl mit Ziel; unvollständige, unbekannte oder überzählige Argumente ergeben
    /// `customize-usage` mit dem Aufruftext
    pub fn parse(&self, argv: &[String]) -> Result<CustomizeCommand> {
        let Some((command, rest)) = argv.split_first() else {
            return Err(self.usage());
        };
        match command.as_str() {
            "check" if rest.is_empty() => Ok(CustomizeCommand::Check),
            "copy" => {
                // --rebase darf an beliebiger Stelle stehen, aber nur einmal
                let positional: Vec<String> = rest
                    .iter()
                    .filter(|arg| *arg != "--rebase")
                    .cloned()
                    .collect();
                let flags = rest.len() - positional.len();
                if flags > 1 {
                    return Err(self.usage());
                }
                Ok(CustomizeCommand::Copy {
                    target: self.target(&positional)?,
                    rebase: flags == 1,
                })
            }
            "reset" => Ok(CustomizeCommand::Reset {
                target: self.target(rest)?,
            }),
            _ => Err(self.usage()),
        }
    }

    /// Ziel aus genau den übrigen Argumenten: workflow, skill <name> oder output-style <name>
    fn target(&self, rest: &[String]) -> Result<CopyTarget> {
        let point = rest.first().map(String::as_str);
        let name = rest.get(1).map(String::as_str);
        match (point, name, rest.len()) {
            (Some("workflow"), _, 1) => Ok(CopyTarget::new(ExtensionPoint::Workflow, "workflow")),
            (Some("skill"), Some(name), 2) if !name.is_empty() && !name.starts_with('-') => {
                Ok(CopyTarget::new(ExtensionPoint::Skill, name))
            }
            (Some("output-style"), Some(name), 2) if !name.is_empty() && !name.starts_with('-') => {
                Ok(CopyTarget::new(ExtensionPoint::OutputStyle, name))
            }
            _ => Err(self.usage()),
        }
    }

    /// Fehler mit dem Aufruftext
    fn usage(&self) -> crate::error::Error {
        HarnessError::new("customize-usage", self.usage.clone()).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argumente(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    fn code(argv: &[&str]) -> Option<String> {
        CustomizeArgs::new()
            .parse(&argumente(argv))
            .err()
            .and_then(|error| error.code().map(ToString::to_string))
    }

    #[test]
    fn liest_check_copy_und_reset() {
        let args = CustomizeArgs::new();
        assert_eq!(
            args.parse(&argumente(&["check"])).unwrap(),
            CustomizeCommand::Check
        );
        assert_eq!(
            args.parse(&argumente(&["copy", "skill", "harness-grill", "--rebase"]))
                .unwrap(),
            CustomizeCommand::Copy {
                target: CopyTarget::new(ExtensionPoint::Skill, "harness-grill"),
                rebase: true,
            }
        );
        assert_eq!(
            args.parse(&argumente(&["copy", "output-style", "kurz"]))
                .unwrap(),
            CustomizeCommand::Copy {
                target: CopyTarget::new(ExtensionPoint::OutputStyle, "kurz"),
                rebase: false,
            }
        );
        assert_eq!(
            args.parse(&argumente(&["reset", "workflow"])).unwrap(),
            CustomizeCommand::Reset {
                target: CopyTarget::new(ExtensionPoint::Workflow, "workflow"),
            }
        );
    }

    #[test]
    fn lehnt_unvollstandige_aufrufe_ab() {
        for argv in [
            vec![],
            vec!["copy", "skill"],
            vec!["reset", "agent", "x"],
            vec!["löschen"],
        ] {
            assert_eq!(code(&argv).as_deref(), Some("customize-usage"), "{argv:?}");
        }
    }

    #[test]
    fn lehnt_unbekannte_optionen_und_uberzahlige_argumente_ab() {
        let invalid = [
            vec!["copy", "skill", "harness-grill", "--rebas"],
            vec!["copy", "skill", "--rebas"],
            vec!["copy", "skill", "harness-grill", "--rebase", "--rebase"],
            vec!["reset", "skill", "x", "--rebase"],
            vec!["check", "--foo"],
            vec!["copy", "workflow", "extra"],
            vec!["copy", "skill", "harness-grill", "extra"],
        ];
        for argv in invalid {
            assert_eq!(code(&argv).as_deref(), Some("customize-usage"), "{argv:?}");
        }
    }

    #[test]
    fn nimmt_rebase_an_beliebiger_stelle() {
        let args = CustomizeArgs::new();
        assert_eq!(
            args.parse(&argumente(&["copy", "--rebase", "skill", "harness-grill"]))
                .unwrap(),
            CustomizeCommand::Copy {
                target: CopyTarget::new(ExtensionPoint::Skill, "harness-grill"),
                rebase: true,
            }
        );
        assert_eq!(
            args.parse(&argumente(&["copy", "workflow", "--rebase"]))
                .unwrap(),
            CustomizeCommand::Copy {
                target: CopyTarget::new(ExtensionPoint::Workflow, "workflow"),
                rebase: true,
            }
        );
    }

    #[test]
    fn nennt_den_aufruftext_des_aufrufers() {
        let eigener = "Aufruf: mein-werkzeug check";
        let error = CustomizeArgs::with_usage(eigener)
            .parse(&argumente(&["löschen"]))
            .unwrap_err();
        assert_eq!(error.to_string(), eigener);
    }
}
