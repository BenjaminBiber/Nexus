//! Unterscheidet Kommentare von Agenten und von Menschen.
//! Portiert aus `packages/cli/src/board/CommentAuthorKind.ts`.

/// Namen der Agenten, die selbst Kommentare schreiben
const AGENT_NAMES: [&str; 2] = ["claude", "dispatcher"];
/// Präfixe der vom Dispatcher gestarteten Agenten (siehe `DispatchService.agentPrefix`,
/// `PlanRunner`, `ResearchRunner`)
const AGENT_PREFIXES: [&str; 3] = ["junior-", "planner-", "research-"];

/// Unterscheidet Kommentare von Agenten und von Menschen.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommentAuthorKind;

impl CommentAuthorKind {
    /// Wahr, wenn der Name zu einem Agenten gehört
    pub fn is_agent(&self, author_name: &str) -> bool {
        let name = author_name.trim().to_lowercase();
        if name.is_empty() {
            return false;
        }
        AGENT_NAMES.contains(&name.as_str())
            || AGENT_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erkennt_feste_agentennamen_und_prafixe() {
        for name in [
            "claude",
            "Claude",
            "dispatcher",
            "junior-AIH-0070",
            "planner-AIH-0042",
            "research-ARC-F0001",
        ] {
            assert!(CommentAuthorKind.is_agent(name), "{name}");
        }
    }

    #[test]
    fn halt_andere_namen_fur_menschen() {
        for name in ["Florian", "benjamin.biber", "", "juniorin"] {
            assert!(!CommentAuthorKind.is_agent(name), "{name}");
        }
    }
}
