//! Betrifft eine Frage den Benutzer („Eigene“)? Gemeinsame Regel für Fragenliste, Zähler,
//! Meldungen und Recherche-Automatik.
//! Portiert aus `packages/core/src/board/QuestionConcern.ts`.

use std::collections::HashSet;

/// Was zur Frage bekannt sein muss: wer sie gestellt hat und auf welchen Boards ihre Tickets
/// liegen
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcernSubject {
    pub created_by_id: String,
    pub board_ids: Vec<i32>,
}

/// Die Regel für „Eigene“.
pub struct QuestionConcern;

impl QuestionConcern {
    /// Selbst gestellt (auch von einem Agent unter dem eigenen Benutzer) oder mit einem Ticket
    /// auf einem eigenen Board verknüpft
    pub fn concerns(
        question: &ConcernSubject,
        user_id: &str,
        own_board_ids: &HashSet<i32>,
    ) -> bool {
        question.created_by_id == user_id
            || question
                .board_ids
                .iter()
                .any(|id| own_board_ids.contains(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::USER_ID;

    /// Der Kollege aus `BoardQuestionMine.test.ts`
    const OTHER_ID: &str = "22222222-2222-2222-2222-222222222222";

    #[test]
    fn betrifft_den_benutzer_wenn_er_sie_gestellt_hat_oder_ein_ticket_auf_eigenem_board_liegt() {
        let own: HashSet<i32> = HashSet::from([1]);
        let subject = |created_by: &str, board_ids: Vec<i32>| ConcernSubject {
            created_by_id: created_by.to_string(),
            board_ids,
        };

        assert!(QuestionConcern::concerns(
            &subject(USER_ID, Vec::new()),
            USER_ID,
            &own
        ));
        assert!(QuestionConcern::concerns(
            &subject(OTHER_ID, vec![2, 1]),
            USER_ID,
            &own
        ));
        assert!(!QuestionConcern::concerns(
            &subject(OTHER_ID, vec![2]),
            USER_ID,
            &own
        ));
        assert!(!QuestionConcern::concerns(
            &subject(OTHER_ID, Vec::new()),
            USER_ID,
            &own
        ));
    }
}
