//! Tests der Fragen-Dienste; portiert aus `packages/core/tests/unit/board/`.
//! Je Modul eine der Testdateien aus TypeScript.

mod decision;
mod mine;
mod notifiable;
mod service;

use crate::board::question_service::{
    BoardQuestionDetail, BoardQuestionWriteOptions, NewBoardQuestionInput,
};
use crate::board::question_types::BoardQuestionHistoryType;
use crate::error::Result;
use crate::testing::BoardFixture;

/// Ohne erwartete Version schreiben
fn any_rev() -> BoardQuestionWriteOptions {
    BoardQuestionWriteOptions::default()
}

/// Legt eine Frage im Projekt ARC unter dem Session-Agenten an
async fn ask(f: &BoardFixture, input: NewBoardQuestionInput) -> Result<BoardQuestionDetail> {
    f.questions.create("ARC", &input, &f.agent).await
}

/// Legt eine Frage mit nur einem Titel an
async fn ask_titled(f: &BoardFixture, title: &str) -> BoardQuestionDetail {
    ask(f, NewBoardQuestionInput::titled(title))
        .await
        .expect("die Frage lässt sich anlegen")
}

/// Art und Text des letzten Verlaufseintrags
fn last_history(detail: &BoardQuestionDetail) -> (BoardQuestionHistoryType, &str) {
    let entry = detail
        .history
        .last()
        .expect("der Verlauf hat mindestens „angelegt“");
    (entry.entry_type, entry.text.as_str())
}

/// Die IDs einer Liste von Fragen
fn ids(detail: &[crate::board::question_service::BoardQuestionListItem]) -> Vec<&str> {
    detail
        .iter()
        .map(|item| item.view.question.id.as_str())
        .collect()
}
