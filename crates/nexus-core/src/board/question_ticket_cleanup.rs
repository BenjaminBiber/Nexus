//! Entfernt beim Löschen eines Tickets dessen Bezüge an Fragen (betroffen und Abhängigkeit)
//! und schreibt den Verlauf der Fragen.
//! Portiert aus `packages/core/src/board/QuestionTicketCleanup.ts`.

use crate::board::basics::BoardActor;
use crate::board::database::BoardUnitOfWork;
use crate::board::question_types::{BoardQuestion, BoardQuestionHistoryEntry, BoardQuestionHistoryType};
use crate::error::{Error, Result};

/// Das Nötigste eines Tickets für den Verlauf der Fragen (`Pick<Ticket, 'id' | 'title'>`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedTicket {
    pub id: String,
    pub title: String,
}

/// Räumt die Bezüge der Fragen auf ein gelöschtes Ticket ab.
pub struct QuestionTicketCleanup;

impl QuestionTicketCleanup {
    /// Läuft vor dem Löschen des Tickets; jede betroffene Frage bekommt eine neue Version und
    /// einen Eintrag „ticket-deleted“. Sperrreihenfolge wie beim Setzen der Tickets einer Frage:
    /// erst die Fragezeilen (sortiert), dann die Bezüge.
    pub async fn remove(
        u: &mut dyn BoardUnitOfWork,
        ticket: &DeletedTicket,
        actor: &BoardActor,
        at: &str,
    ) -> Result<Vec<String>> {
        let mut referencing = u.questions().ids_referencing_ticket(&ticket.id).await?;
        referencing.sort();
        for question_id in &referencing {
            u.questions().by_id(question_id, true).await?;
        }
        let affected = u.questions().remove_ticket(&ticket.id).await?;
        for question_id in &affected {
            // Frisch gelesen (ohne den entfernten Bezug), damit update ihn nicht zurückschreibt
            let current = u
                .questions()
                .by_id(question_id, true)
                .await?
                .ok_or_else(|| Error::unexpected_message(format!("Frage {question_id} fehlt")))?;
            u.questions()
                .update(&BoardQuestion {
                    rev: current.rev + 1,
                    updated_at: at.to_string(),
                    ..current
                })
                .await?;
            u.question_history()
                .insert(&[BoardQuestionHistoryEntry {
                    question_id: question_id.clone(),
                    at: at.to_string(),
                    by_id: actor.user_id.clone(),
                    by_name: actor.name.clone(),
                    by_kind: None,
                    entry_type: BoardQuestionHistoryType::TicketDeleted,
                    text: format!("Ticket {} „{}“ gelöscht, Bezug entfernt.", ticket.id, ticket.title),
                }])
                .await?;
        }
        Ok(affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::recording_unit::RecordingUnit;

    /// Der Handelnde der TypeScript-Tests
    fn actor() -> BoardActor {
        BoardActor {
            user_id: "u-1".into(),
            name: "Florian".into(),
        }
    }

    /// Das gelöschte Ticket der TypeScript-Tests
    fn ticket() -> DeletedTicket {
        DeletedTicket {
            id: "ARC-0001".into(),
            title: "Login".into(),
        }
    }

    #[tokio::test]
    async fn sperrt_die_betroffenen_fragen_sortiert_bevor_es_die_bezuge_entfernt() {
        let mut unit = RecordingUnit::new(&["ARC-F0002", "ARC-F0001"]);

        let affected = QuestionTicketCleanup::remove(
            &mut unit,
            &ticket(),
            &actor(),
            "2026-01-01T00:00:00.000Z",
        )
        .await
        .unwrap();

        assert_eq!(affected, ["ARC-F0001", "ARC-F0002"]);
        assert_eq!(
            unit.calls()[..4],
            [
                "read ARC-0001",
                "lock ARC-F0001",
                "lock ARC-F0002",
                "remove ARC-0001"
            ]
        );
        // Danach frisch gelesen (ohne den entfernten Bezug), damit update die Kindzeilen nicht
        // zurückschreibt
        assert_eq!(
            unit.calls()[4..],
            [
                "lock ARC-F0001",
                "update ARC-F0001",
                "lock ARC-F0002",
                "update ARC-F0002"
            ]
        );
    }

    #[tokio::test]
    async fn ohne_betroffene_fragen_sperrt_es_nichts_und_schreibt_keinen_verlauf() {
        let mut unit = RecordingUnit::new(&[]);

        let affected = QuestionTicketCleanup::remove(&mut unit, &ticket(), &actor(), "x")
            .await
            .unwrap();

        assert!(affected.is_empty());
        assert_eq!(unit.calls(), ["read ARC-0001", "remove ARC-0001"]);
    }
}
