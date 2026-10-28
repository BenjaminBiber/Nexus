//! Eigenheiten der Speicher-Datenbank, die kein Vertrag abdeckt: Rückabwicklung samt
//! Kennwerten, genau ein Aufruf der Arbeit und die Kopie der gelesenen Werte.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::*;
use crate::board::database::{NewBoard, NewProject};
use crate::board::types::BoardUser;
use crate::error::Error;

const NOW: &str = "2026-10-13T10:00:00.000Z";
const USER_ID: &str = "11111111-1111-1111-1111-111111111111";

/// Legt Projekt und Board an und liefert beide Kennungen
async fn seed(db: &InMemoryBoardDatabase) -> (i32, i32) {
    crate::tx!(db, |u| {
        let project = u
            .projects()
            .insert(&NewProject {
                key: "ARC".into(),
                name: "Arches".into(),
                created_at: NOW.into(),
                created_by: USER_ID.into(),
            })
            .await?;
        let board = u
            .boards()
            .insert(&NewBoard {
                project_id: project.id,
                owner_id: USER_ID.into(),
                name: "Sprint".into(),
                repo_path: None,
                created_at: NOW.into(),
            })
            .await?;
        Ok((project.id, board.id))
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn fuhrt_die_arbeit_genau_einmal_aus() {
    let db = InMemoryBoardDatabase::new();
    let zahler = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&zahler);
    crate::tx!(db, [calls], |u| {
        calls.fetch_add(1, Ordering::SeqCst);
        u.users()
            .upsert(&BoardUser {
                id: USER_ID.into(),
                name: "Florian".into(),
            })
            .await
    })
    .await
    .unwrap();
    assert_eq!(zahler.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn ein_fehler_in_der_arbeit_verwirft_alle_anderungen() {
    let db = InMemoryBoardDatabase::new();
    let (project_id, _) = seed(&db).await;

    let fehler = crate::tx!(db, |u| {
        u.projects().rename(project_id, "Neuer Name").await?;
        u.projects().take_ticket_number(project_id).await?;
        Err::<(), Error>(Error::unexpected_message("Abbruch"))
    })
    .await
    .unwrap_err();
    assert_eq!(fehler.to_string(), "Abbruch");

    let project = crate::tx!(db, |u| { u.projects().by_id(project_id).await })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(project.name, "Arches");
    assert_eq!(project.next_ticket_number, 1);
}

#[tokio::test]
async fn kennwerte_steigen_nur_bei_erfolgreichen_transaktionen() {
    let db = InMemoryBoardDatabase::new();
    let (project_id, board_id) = seed(&db).await;
    let nach_dem_anlegen = db.change_marker(board_id).await.unwrap();

    let fehler = crate::tx!(db, |u| {
        u.boards()
            .insert(&NewBoard {
                project_id,
                owner_id: USER_ID.into(),
                name: "Zweites".into(),
                repo_path: None,
                created_at: NOW.into(),
            })
            .await?;
        Err::<(), Error>(Error::unexpected_message("Abbruch"))
    })
    .await;
    assert!(fehler.is_err());

    assert_eq!(db.change_marker(board_id).await.unwrap(), nach_dem_anlegen);
    assert_eq!(db.question_marker(project_id).await.unwrap(), "0");
    // Ein Board, das es nie gab, steht bei 0
    assert_eq!(db.change_marker(9999).await.unwrap(), "0");
}

#[tokio::test]
async fn gelesene_werte_sind_kopien_und_andern_die_datenbank_nicht() {
    let db = InMemoryBoardDatabase::new();
    let (project_id, _) = seed(&db).await;

    let mut project = crate::tx!(db, |u| { u.projects().by_id(project_id).await })
        .await
        .unwrap()
        .unwrap();
    project.name = "Verbogen".into();

    let erneut = crate::tx!(db, |u| { u.projects().by_id(project_id).await })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(erneut.name, "Arches");
}

#[tokio::test]
async fn zwei_projekte_mit_demselben_schlussel_gehen_nicht() {
    let db = InMemoryBoardDatabase::new();
    seed(&db).await;
    let fehler = crate::tx!(db, |u| {
        u.projects()
            .insert(&NewProject {
                key: "ARC".into(),
                name: "Noch mal".into(),
                created_at: NOW.into(),
                created_by: USER_ID.into(),
            })
            .await
            .map(|_| ())
    })
    .await
    .unwrap_err();
    assert_eq!(fehler.code(), Some("project-exists"));
    assert_eq!(fehler.to_string(), "Projekt ARC gibt es schon.");
}
