//! Offene eigene Fragen für die Meldungen;
//! portiert aus `packages/core/tests/unit/board/BoardQuestionNotifiable.test.ts`.

use super::any_rev;
use crate::actors::AuthorKind;
use crate::board::basics::BoardActor;
use crate::board::database::{BoardDatabase, NewBoard};
use crate::board::question_service::NewBoardQuestionInput;
use crate::board::question_types::BoardQuestionActor;
use crate::board::ticket_service::{NewTicketInput, WriteOptions};
use crate::board::types::BoardUser;
use crate::questions::{ResearchInput, ResearchOptionInput};
use crate::testing::{BoardFixture, NOW, USER_ID};
use crate::tx;

/// Der Kollege, dessen Board nicht dem Testbenutzer gehört
const OTHER_ID: &str = "22222222-2222-2222-2222-222222222222";

#[tokio::test]
async fn liefert_nur_offene_eigene_fragen_mit_recherchen_und_repo_pfaden() {
    let colleague = BoardQuestionActor {
        actor: BoardActor {
            user_id: OTHER_ID.into(),
            name: "Kollege".into(),
        },
        kind: AuthorKind::Human,
        session: None,
    };
    let f = BoardFixture::create().await;
    f.boards
        .set_repo_path(f.board_id, Some("C:\\Projects\\Arches"))
        .await
        .unwrap();
    let project_id = f.project_id().await;
    let foreign = tx!(f.db, [project_id], |u| {
        u.users()
            .upsert(&BoardUser {
                id: OTHER_ID.into(),
                name: "Kollege".into(),
            })
            .await?;
        u.boards()
            .insert(&NewBoard {
                project_id,
                owner_id: OTHER_ID.into(),
                name: "Fremd".into(),
                repo_path: None,
                created_at: NOW.into(),
            })
            .await
    })
    .await
    .unwrap();
    for (board_id, title) in [(f.board_id, "Eigen"), (foreign.id, "Fremd")] {
        f.tickets
            .create(
                board_id,
                &NewTicketInput::titled(title),
                &f.actor,
                &WriteOptions::default(),
            )
            .await
            .unwrap();
    }

    f.questions
        .create(
            "ARC",
            &NewBoardQuestionInput {
                ticket_ids: Some(vec!["ARC-0001".into()]),
                occasion: Some("Planung".into()),
                ..NewBoardQuestionInput::titled("Eigene Frage")
            },
            &f.agent,
        )
        .await
        .unwrap();
    f.questions
        .add_research(
            "ARC-F0001",
            &ResearchInput::options(vec![ResearchOptionInput {
                recommended: Some(true),
                ..ResearchOptionInput::new("Datei", "JSON")
            }]),
            &f.agent,
        )
        .await
        .unwrap();
    f.questions
        .create(
            "ARC",
            &NewBoardQuestionInput {
                ticket_ids: Some(vec!["ARC-0002".into()]),
                ..NewBoardQuestionInput::titled("Fremde Frage")
            },
            &colleague,
        )
        .await
        .unwrap();
    f.questions
        .create(
            "ARC",
            &NewBoardQuestionInput::titled("Ohne Ticket"),
            &colleague,
        )
        .await
        .unwrap();
    f.questions
        .create(
            "ARC",
            &NewBoardQuestionInput {
                ticket_ids: Some(vec!["ARC-0001".into()]),
                ..NewBoardQuestionInput::titled("Verworfen")
            },
            &f.agent,
        )
        .await
        .unwrap();
    f.questions
        .discard("ARC-F0004", Some("doppelt"), &f.human, &any_rev())
        .await
        .unwrap();

    let own = f.questions.notifiable(USER_ID).await.unwrap();
    assert_eq!(own.len(), 1);
    let question = &own[0];
    assert_eq!(question.id, "ARC-F0001");
    assert_eq!(question.title, "Eigene Frage");
    assert_eq!(question.occasion.as_deref(), Some("Planung"));
    assert_eq!(question.origin.kind, AuthorKind::Session);
    assert_eq!(question.research.len(), 1);
    assert_eq!(question.research[0].kind, AuthorKind::Session);
    assert_eq!(question.research[0].options[0].id, "O1");
    assert!(question.research[0].options[0].recommended);
    assert_eq!(question.project_key, "ARC");
    assert_eq!(question.project_name, "Arches");
    assert_eq!(question.repo_paths, ["C:\\Projects\\Arches"]);

    assert_eq!(
        f.questions
            .notifiable(OTHER_ID)
            .await
            .unwrap()
            .iter()
            .map(|question| question.id.as_str())
            .collect::<Vec<_>>(),
        ["ARC-F0002", "ARC-F0003"]
    );
    assert!(f
        .questions
        .notifiable("33333333-3333-3333-3333-333333333333")
        .await
        .unwrap()
        .is_empty());
}
