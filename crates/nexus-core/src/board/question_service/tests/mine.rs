//! Eigene Fragen in Liste, Zählern und Meldungen;
//! portiert aus `packages/core/tests/unit/board/BoardQuestionMine.test.ts`.

use super::{any_rev, ask_titled, ids};
use crate::actors::AuthorKind;
use crate::board::basics::BoardActor;
use crate::board::database::{BoardDatabase, NewBoard};
use crate::board::question_service::{BoardQuestionListFilter, NewBoardQuestionInput};
use crate::board::question_types::BoardQuestionActor;
use crate::board::ticket_service::{NewTicketInput, WriteOptions};
use crate::board::types::BoardUser;
use crate::testing::{BoardFixture, NOW, USER_ID};
use crate::tx;

/// Der Kollege, dessen Board nicht dem Testbenutzer gehört
const OTHER_ID: &str = "22222222-2222-2222-2222-222222222222";

/// Der Kollege als Handelnder
fn colleague() -> BoardQuestionActor {
    BoardQuestionActor {
        actor: BoardActor {
            user_id: OTHER_ID.into(),
            name: "Kollege".into(),
        },
        kind: AuthorKind::Human,
        session: None,
    }
}

/// Eigenes Board mit Ticket ARC-0001, fremdes Board mit Ticket ARC-0002
async fn setup() -> BoardFixture {
    let f = BoardFixture::create().await;
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
    .expect("das fremde Board lässt sich anlegen");
    for (board_id, title) in [(f.board_id, "Eigen"), (foreign.id, "Fremd")] {
        f.tickets
            .create(
                board_id,
                &NewTicketInput::titled(title),
                &f.actor,
                &WriteOptions::default(),
            )
            .await
            .expect("das Ticket lässt sich anlegen");
    }
    f
}

/// Legt eine Frage des Kollegen zu diesen Tickets an
async fn by_colleague(f: &BoardFixture, title: &str, ticket_ids: &[&str]) {
    f.questions
        .create(
            "ARC",
            &NewBoardQuestionInput {
                ticket_ids: Some(ticket_ids.iter().map(|id| id.to_string()).collect()),
                ..NewBoardQuestionInput::titled(title)
            },
            &colleague(),
        )
        .await
        .expect("die Frage lässt sich anlegen");
}

#[tokio::test]
async fn kennzeichnet_in_der_liste_eigene_fragen_und_zahlt_offene_eigene_in_counts_mine() {
    let f = setup().await;
    by_colleague(&f, "Zu meinem Ticket", &["ARC-0001"]).await;
    ask_titled(&f, "Selbst gestellt").await;
    by_colleague(&f, "Zum fremden Ticket", &["ARC-0002"]).await;
    by_colleague(&f, "Fremd ohne Ticket", &[]).await;
    ask_titled(&f, "Eigen, verworfen").await;
    f.questions
        .discard("ARC-F0005", Some("doppelt"), &f.human, &any_rev())
        .await
        .unwrap();

    let list = f
        .questions
        .list("ARC", &BoardQuestionListFilter::default(), Some(USER_ID))
        .await
        .unwrap();
    let mut mine: Vec<(&str, bool)> = list
        .questions
        .iter()
        .map(|item| (item.view.question.id.as_str(), item.mine))
        .collect();
    mine.sort();
    assert_eq!(
        mine,
        [
            ("ARC-F0001", true),
            ("ARC-F0002", true),
            ("ARC-F0003", false),
            ("ARC-F0004", false),
            ("ARC-F0005", true),
        ]
    );
    assert_eq!(
        (
            list.counts.open,
            list.counts.answered,
            list.counts.discarded,
            list.counts.mine
        ),
        (4, 0, 1, 2)
    );
    assert_eq!(
        f.questions
            .list("ARC", &BoardQuestionListFilter::default(), Some(OTHER_ID))
            .await
            .unwrap()
            .counts
            .mine,
        3
    );
    // Ohne Betrachter ist nichts eigen
    assert_eq!(
        f.questions
            .list("ARC", &BoardQuestionListFilter::default(), None)
            .await
            .unwrap()
            .counts
            .mine,
        0
    );
}

#[tokio::test]
async fn meldet_auch_selbst_gestellte_offene_fragen_ohne_ticket_keine_fremden() {
    let f = setup().await;
    ask_titled(&f, "Selbst gestellt").await;
    by_colleague(&f, "Fremd ohne Ticket", &[]).await;
    by_colleague(&f, "Zum fremden Ticket", &["ARC-0002"]).await;

    assert_eq!(
        f.questions
            .notifiable(USER_ID)
            .await
            .unwrap()
            .iter()
            .map(|question| question.id.as_str())
            .collect::<Vec<_>>(),
        ["ARC-F0001"]
    );
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
}

#[tokio::test]
async fn meldet_selbst_gestellte_fragen_auch_ohne_eigenes_board() {
    let f = setup().await;
    let stranger = BoardQuestionActor {
        actor: BoardActor {
            user_id: "33333333-3333-3333-3333-333333333333".into(),
            name: "Neu".into(),
        },
        kind: AuthorKind::Session,
        session: Some("s".into()),
    };
    let user = BoardUser {
        id: stranger.actor.user_id.clone(),
        name: "Neu".into(),
    };
    tx!(f.db, [user], |u| { u.users().upsert(&user).await })
        .await
        .unwrap();
    f.questions
        .create(
            "ARC",
            &NewBoardQuestionInput::titled("Ohne Board"),
            &stranger,
        )
        .await
        .unwrap();

    assert_eq!(
        f.questions
            .notifiable(&stranger.actor.user_id)
            .await
            .unwrap()
            .iter()
            .map(|question| question.id.as_str())
            .collect::<Vec<_>>(),
        ["ARC-F0001"]
    );
    // Die Liste des Testbenutzers bleibt davon unberührt
    assert!(ids(&f
        .questions
        .list("ARC", &BoardQuestionListFilter::default(), Some(USER_ID))
        .await
        .unwrap()
        .questions)
    .contains(&"ARC-F0001"));
}
