//! Teile der Dispatcher-Einstellungen, die der Fachkern schon vor dem Dispatcher braucht.
//! Portiert aus `packages/core/src/dispatch/DispatchConfig.ts`; die vollständigen
//! Einstellungen folgen mit dem Dispatcher.

use serde::{Deserialize, Serialize};

/// Vorsilbe des Blockiert-Grunds, wenn der Merge scheiterte („Erneut mergen“ im Panel)
pub const MERGE_REASON_PREFIX: &str = "Merge:";

/// Worktrees des Dispatchers: ein Worktree je Ticket oder Tickets einer Spec in einem
/// gemeinsamen Worktree (alt)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JoinWorktrees {
    /// Jedes Ticket im eigenen Worktree
    #[default]
    None,
    /// Tickets derselben Spec teilen einen Worktree
    SameSpec,
}

impl JoinWorktrees {
    /// Wert, wie er in den Einstellungen steht
    pub fn as_str(&self) -> &'static str {
        match self {
            JoinWorktrees::None => "none",
            JoinWorktrees::SameSpec => "sameSpec",
        }
    }
}

/// Erlaubte Werte von `joinWorktrees`
pub const JOIN_WORKTREES: [JoinWorktrees; 2] = [JoinWorktrees::None, JoinWorktrees::SameSpec];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schreibt_die_werte_wie_typescript() {
        assert_eq!(
            serde_json::to_string(&JoinWorktrees::SameSpec).unwrap(),
            "\"sameSpec\""
        );
        assert_eq!(JoinWorktrees::None.as_str(), "none");
        assert_eq!(
            serde_json::from_str::<JoinWorktrees>("\"none\"").unwrap(),
            JoinWorktrees::None
        );
        assert_eq!(JOIN_WORKTREES.len(), 2);
    }
}
