//! Der gemeinsame Vertrag für jede Umsetzung von [`crate::board::database::BoardDatabase`];
//! portiert aus `packages/core/tests/unit/board/helpers/BoardDatabaseContract.ts`.

/// Gemeinsame Tests für jede Umsetzung von `BoardDatabase` (In-Memory, SQLite, SQL Server).
///
/// `create` legt eine leere Datenbank an, `reset` räumt eine gemeinsam genutzte Datenbank vor
/// jedem Fall auf, `lock` liefert eine Sperre, die der Fall bis zum Ende hält – SQL Server
/// teilt sich eine Test-Datenbank und braucht sie, In-Memory und SQLite nicht.
///
/// Die aufrufende Crate braucht `tokio` mit den Merkmalen `macros` und `rt-multi-thread` als
/// Dev-Abhängigkeit.
///
/// ```
/// # // In einer eigenen Datei unter tests/ steht nur diese eine Zeile:
/// nexus_core::board_database_contract!(
///     in_memory,
///     || async { nexus_core::board::in_memory::InMemoryBoardDatabase::new() }
/// );
/// ```
///
/// Nach `lock` dürfen Attribute folgen; sie stehen dann an jedem einzelnen Fall. So bekommen
/// die Verträge gegen SQL Server ihr `#[ignore]`, weil sie nur mit `HARNESS_TEST_DB` laufen:
///
/// ```ignore
/// nexus_core::board_database_contract!(
///     mssql, create, reset, lock,
///     #[ignore = "braucht HARNESS_TEST_DB"]
/// );
/// ```
#[macro_export]
macro_rules! board_database_contract {
    ($name:ident, $create:expr) => {
        $crate::board_database_contract!($name, $create, || async {});
    };
    ($name:ident, $create:expr, $reset:expr) => {
        $crate::board_database_contract!($name, $create, $reset, || async {});
    };
    ($name:ident, $create:expr, $reset:expr, $lock:expr) => {
        $crate::board_database_contract!($name, $create, $reset, $lock,);
    };
    ($name:ident, $create:expr, $reset:expr, $lock:expr, $(#[$attr:meta])*) => {
        mod $name {
            #![allow(unused_imports)]

            use ::std::collections::BTreeMap;

            use $crate::board::basics::{NoteState, TicketStatus};
            use $crate::board::database::{
                BoardDatabase, BoardUnitOfWork, NewComment, NewNote, NoteFilter, TicketFilter,
            };
            use $crate::board::types::{
                Deletion, HistoryEntry, Note, StepProgress, Ticket, TicketStep,
            };
            use $crate::testing::*;

            /// Leere Datenbank samt angelegtem Benutzer; die Sperre hält der Fall bis zum Ende
            async fn setup() -> (impl BoardDatabase, impl Sized) {
                let guard = ($lock)().await;
                ($reset)().await;
                let db = ($create)().await;
                $crate::testing::upsert_user(&db).await;
                (db, guard)
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn aktualisiert_den_namen_eines_benutzers() {
                let (db, _guard) = setup().await;
                $crate::tx!(db, |u| { u.users().upsert(&user_named("Flo")).await })
                    .await
                    .unwrap();
                let geladen = $crate::tx!(db, |u| { u.users().by_id(USER_ID).await })
                    .await
                    .unwrap();
                assert_eq!(geladen, Some(user_named("Flo")));
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn vergibt_ticketnummern_pro_projekt_fortlaufend() {
                let (db, _guard) = setup().await;
                let project_id = seed_board(&db).await.project_id;
                let nummern = $crate::tx!(db, |u| {
                    let erste = u.projects().take_ticket_number(project_id).await?;
                    let zweite = u.projects().take_ticket_number(project_id).await?;
                    Ok(vec![erste, zweite])
                })
                .await
                .unwrap();
                assert_eq!(nummern, [1, 2]);
                let projekt = $crate::tx!(db, |u| { u.projects().by_key("ARC").await })
                    .await
                    .unwrap();
                assert_eq!(projekt.unwrap().next_ticket_number, 3);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn speichert_tickets_mit_labels_und_abhangigkeiten_in_ihrer_reihenfolge() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0001", project_id, board_id, 1))
                        .await?;
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0002", project_id, board_id, 2))
                        .await?;
                    u.tickets()
                        .insert(&Ticket {
                            labels: vec!["ui".into(), "api".into()],
                            depends_on: vec!["ARC-0002".into(), "ARC-0001".into()],
                            title: "Umlaute äöü".into(),
                            ..ticket_fixture("ARC-0003", project_id, board_id, 3)
                        })
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();

                let ticket = $crate::tx!(db, |u| { u.tickets().by_id("ARC-0003", false).await })
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(ticket.labels, ["ui", "api"]);
                assert_eq!(ticket.depends_on, ["ARC-0002", "ARC-0001"]);
                assert_eq!(ticket.title, "Umlaute äöü");
                assert_eq!(ticket.rev, 1);

                let abhangige = $crate::tx!(db, |u| { u.tickets().dependents("ARC-0001").await })
                    .await
                    .unwrap();
                assert_eq!(abhangige, ["ARC-0003"]);

                let mut ids: Vec<String> = $crate::tx!(db, |u| {
                    u.tickets()
                        .list(&TicketFilter {
                            board_ids: Some(vec![board_id]),
                            ..TicketFilter::default()
                        })
                        .await
                })
                .await
                .unwrap()
                .into_iter()
                .map(|t| t.id)
                .collect();
                ids.sort();
                assert_eq!(ids, ["ARC-0001", "ARC-0002", "ARC-0003"]);

                let eines: Vec<String> = $crate::tx!(db, |u| {
                    u.tickets()
                        .list(&TicketFilter {
                            ids: Some(vec!["ARC-0002".into()]),
                            ..TicketFilter::default()
                        })
                        .await
                })
                .await
                .unwrap()
                .into_iter()
                .map(|t| t.id)
                .collect();
                assert_eq!(eines, ["ARC-0002"]);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn lehnt_ein_update_mit_veralteter_version_als_conflict_ab() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0001", project_id, board_id, 1))
                        .await
                })
                .await
                .unwrap();

                $crate::tx!(db, |u| {
                    let ticket = u.tickets().by_id("ARC-0001", true).await?.unwrap();
                    u.tickets()
                        .update(&Ticket {
                            title: "Neu".into(),
                            rev: 2,
                            ..ticket
                        })
                        .await
                })
                .await
                .unwrap();

                let fehler = $crate::tx!(db, |u| {
                    let ticket = u.tickets().by_id("ARC-0001", false).await?.unwrap();
                    u.tickets()
                        .update(&Ticket {
                            title: "Zu spät".into(),
                            rev: 2,
                            ..ticket
                        })
                        .await
                })
                .await
                .unwrap_err();
                assert_eq!(fehler.code(), Some("conflict"));

                let ticket = $crate::tx!(db, |u| { u.tickets().by_id("ARC-0001", false).await })
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(ticket.title, "Neu");
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn rollt_bei_einem_fehler_die_ganze_transaktion_zuruck() {
                let (db, _guard) = setup().await;
                let project_id = seed_board(&db).await.project_id;
                let fehler = $crate::tx!(db, |u| {
                    u.projects().take_ticket_number(project_id).await?;
                    Err::<(), $crate::error::Error>($crate::error::Error::unexpected_message(
                        "Abbruch",
                    ))
                })
                .await
                .unwrap_err();
                assert_eq!(fehler.to_string(), "Abbruch");

                let projekt = $crate::tx!(db, |u| { u.projects().by_id(project_id).await })
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(projekt.next_ticket_number, 1);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn entfernt_verweise_auf_ein_ticket_und_loscht_es() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0001", project_id, board_id, 1))
                        .await?;
                    u.tickets()
                        .insert(&Ticket {
                            depends_on: vec!["ARC-0001".into()],
                            ..ticket_fixture("ARC-0002", project_id, board_id, 2)
                        })
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();

                let beruhrt = $crate::tx!(db, |u| {
                    let ids = u.tickets().remove_dependency_on("ARC-0001").await?;
                    u.tickets().delete("ARC-0001").await?;
                    Ok(ids)
                })
                .await
                .unwrap();
                assert_eq!(beruhrt, ["ARC-0002"]);

                let weg = $crate::tx!(db, |u| { u.tickets().by_id("ARC-0001", false).await })
                    .await
                    .unwrap();
                assert_eq!(weg, None);
                let geblieben = $crate::tx!(db, |u| { u.tickets().by_id("ARC-0002", false).await })
                    .await
                    .unwrap()
                    .unwrap();
                assert!(geblieben.depends_on.is_empty());
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn liefert_die_hochste_sortorder_einer_spalte() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                let leer = $crate::tx!(db, |u| {
                    u.tickets()
                        .max_sort_order(board_id, TicketStatus::Backlog)
                        .await
                })
                .await
                .unwrap();
                assert_eq!(leer, None);

                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&Ticket {
                            sort_order: 4,
                            ..ticket_fixture("ARC-0001", project_id, board_id, 1)
                        })
                        .await
                })
                .await
                .unwrap();
                let hochste = $crate::tx!(db, |u| {
                    u.tickets()
                        .max_sort_order(board_id, TicketStatus::Backlog)
                        .await
                })
                .await
                .unwrap();
                assert_eq!(hochste, Some(4));
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn zahlt_offene_tickets_je_board_ohne_erledigte_und_nicht_mehr_notige() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0001", project_id, board_id, 1))
                        .await?;
                    u.tickets()
                        .insert(&Ticket {
                            status: TicketStatus::InProgress,
                            ..ticket_fixture("ARC-0002", project_id, board_id, 2)
                        })
                        .await?;
                    u.tickets()
                        .insert(&Ticket {
                            status: TicketStatus::Done,
                            ..ticket_fixture("ARC-0003", project_id, board_id, 3)
                        })
                        .await?;
                    u.tickets()
                        .insert(&Ticket {
                            status: TicketStatus::NotNeeded,
                            ..ticket_fixture("ARC-0004", project_id, board_id, 4)
                        })
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();

                let gezahlt = $crate::tx!(db, |u| {
                    u.tickets()
                        .open_count_by_board(&[board_id, board_id + 999])
                        .await
                })
                .await
                .unwrap();
                assert_eq!(gezahlt, BTreeMap::from([(board_id, 2i64)]));

                let ohne = $crate::tx!(db, |u| { u.tickets().open_count_by_board(&[]).await })
                    .await
                    .unwrap();
                assert_eq!(ohne, BTreeMap::new());
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn liefert_ohne_texte_dieselben_tickets_mit_labels_und_voraussetzungen() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&Ticket {
                            description: "lang".into(),
                            ..ticket_fixture("ARC-0001", project_id, board_id, 1)
                        })
                        .await?;
                    u.tickets()
                        .insert(&Ticket {
                            labels: vec!["a".into(), "b".into()],
                            depends_on: vec!["ARC-0001".into()],
                            ..ticket_fixture("ARC-0002", project_id, board_id, 2)
                        })
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();

                let leicht = $crate::tx!(db, |u| {
                    u.tickets()
                        .list(&TicketFilter {
                            without_texts: true,
                            ..TicketFilter::default()
                        })
                        .await
                })
                .await
                .unwrap();
                let gesehen: Vec<(String, TicketStatus, Vec<String>, Vec<String>)> = leicht
                    .iter()
                    .map(|t| {
                        (
                            t.id.clone(),
                            t.status,
                            t.labels.clone(),
                            t.depends_on.clone(),
                        )
                    })
                    .collect();
                assert_eq!(
                    gesehen,
                    vec![
                        (
                            "ARC-0001".to_string(),
                            TicketStatus::Backlog,
                            vec![],
                            vec![]
                        ),
                        (
                            "ARC-0002".to_string(),
                            TicketStatus::Backlog,
                            vec!["a".to_string(), "b".to_string()],
                            vec!["ARC-0001".to_string()]
                        ),
                    ]
                );
                // Vorhanden bleibt erkennbar, leer bleibt leer
                let texte: Vec<(bool, String)> = leicht
                    .iter()
                    .map(|t| (!t.description.trim().is_empty(), t.plan.clone()))
                    .collect();
                assert_eq!(
                    texte,
                    vec![(true, String::new()), (false, String::new())]
                );
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn speichert_kommentare_verlauf_notizen_und_loschungen() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0001", project_id, board_id, 1))
                        .await?;
                    u.comments()
                        .insert(&new_comment("ARC-0001", "claude", "Hallo"))
                        .await?;
                    u.history()
                        .insert(&[HistoryEntry {
                            by_name: "claude".into(),
                            note: Some("angelegt".into()),
                            ..status_entry("ARC-0001", NOW, "backlog")
                        }])
                        .await?;
                    let note = u
                        .notes()
                        .insert(&NewNote {
                            source: Some("ARC-0001".into()),
                            labels: vec!["cache".into()],
                            ..new_note(board_id, "Cache leert sich", "Befund")
                        })
                        .await?;
                    u.notes()
                        .update(&Note {
                            seen_count: 3,
                            seen_in: vec!["ARC-0002".into(), "ARC-0003".into()],
                            state: NoteState::Dropped,
                            reason: Some("egal".into()),
                            ..note
                        })
                        .await?;
                    u.deletions()
                        .insert(&Deletion {
                            at: NOW.into(),
                            by_id: USER_ID.into(),
                            by_name: "claude".into(),
                            ticket_id: "ARC-0009".into(),
                            ticket_title: "Weg".into(),
                            from_status: Some(TicketStatus::Backlog),
                        })
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();

                let kommentare = $crate::tx!(db, |u| { u.comments().list("ARC-0001").await })
                    .await
                    .unwrap();
                assert_eq!(kommentare.len(), 1);
                assert_eq!(kommentare[0].body, "Hallo");
                assert_eq!(kommentare[0].author_name, "claude");
                assert_eq!(kommentare[0].created_at, NOW);
                assert_eq!(kommentare[0].read_by_agent_at, None);

                let verlauf = $crate::tx!(db, |u| { u.history().list("ARC-0001").await })
                    .await
                    .unwrap();
                assert_eq!(verlauf.len(), 1);
                assert_eq!(verlauf[0].field, "status");
                assert_eq!(verlauf[0].to.as_deref(), Some("backlog"));
                assert_eq!(verlauf[0].note.as_deref(), Some("angelegt"));

                let verworfen = $crate::tx!(db, |u| {
                    u.notes()
                        .list(&NoteFilter {
                            states: Some(vec![NoteState::Dropped]),
                            ..NoteFilter::of_board(board_id)
                        })
                        .await
                })
                .await
                .unwrap();
                assert_eq!(verworfen.len(), 1);
                assert_eq!(verworfen[0].title, "Cache leert sich");
                assert_eq!(verworfen[0].body, "Befund");
                assert_eq!(verworfen[0].labels, ["cache"]);
                assert_eq!(verworfen[0].seen_count, 3);
                assert_eq!(verworfen[0].seen_in, ["ARC-0002", "ARC-0003"]);
                assert_eq!(verworfen[0].reason.as_deref(), Some("egal"));
                assert_eq!(verworfen[0].source.as_deref(), Some("ARC-0001"));

                let offene = $crate::tx!(db, |u| {
                    u.notes()
                        .list(&NoteFilter {
                            states: Some(vec![NoteState::Open]),
                            ..NoteFilter::of_board(board_id)
                        })
                        .await
                })
                .await
                .unwrap();
                assert_eq!(offene, vec![]);

                let geloschte = $crate::tx!(db, |u| { u.deletions().list().await })
                    .await
                    .unwrap();
                assert_eq!(geloschte.len(), 1);
                assert_eq!(geloschte[0].ticket_id, "ARC-0009");
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn loscht_ein_board_samt_notizen() {
                let (db, _guard) = setup().await;
                let board_id = seed_board(&db).await.board_id;
                $crate::tx!(db, |u| {
                    u.notes()
                        .insert(&NewNote {
                            author_name: "x".into(),
                            ..new_note(board_id, "", "n")
                        })
                        .await
                        .map(|_| ())
                })
                .await
                .unwrap();
                $crate::tx!(db, |u| { u.boards().delete(board_id).await })
                    .await
                    .unwrap();
                let board = $crate::tx!(db, |u| { u.boards().by_id(board_id).await })
                    .await
                    .unwrap();
                assert_eq!(board, None);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn andert_den_kennwert_bei_tickets_kommentaren_notizen_und_loschungen() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                let anderes = $crate::tx!(db, |u| {
                    u.boards()
                        .insert(&$crate::board::database::NewBoard {
                            project_id,
                            owner_id: USER_ID.into(),
                            name: "Anderes".into(),
                            repo_path: None,
                            created_at: NOW.into(),
                        })
                        .await
                })
                .await
                .unwrap();
                let anderes_vorher = db.change_marker(anderes.id).await.unwrap();
                let m0 = db.change_marker(board_id).await.unwrap();

                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0001", project_id, board_id, 1))
                        .await
                })
                .await
                .unwrap();
                let m1 = db.change_marker(board_id).await.unwrap();
                assert_ne!(m1, m0);

                $crate::tx!(db, |u| {
                    u.comments()
                        .insert(&new_comment("ARC-0001", "F", "x"))
                        .await
                        .map(|_| ())
                })
                .await
                .unwrap();
                let m2 = db.change_marker(board_id).await.unwrap();
                assert_ne!(m2, m1);

                $crate::tx!(db, |u| {
                    u.notes()
                        .insert(&NewNote {
                            author_name: "F".into(),
                            ..new_note(board_id, "", "n")
                        })
                        .await
                        .map(|_| ())
                })
                .await
                .unwrap();
                let m3 = db.change_marker(board_id).await.unwrap();
                assert_ne!(m3, m2);

                $crate::tx!(db, |u| {
                    u.tickets().delete("ARC-0001").await?;
                    u.deletions()
                        .insert(&Deletion {
                            at: NOW.into(),
                            by_id: USER_ID.into(),
                            by_name: "F".into(),
                            ticket_id: "ARC-0001".into(),
                            ticket_title: "Titel".into(),
                            from_status: Some(TicketStatus::Backlog),
                        })
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();
                let m4 = db.change_marker(board_id).await.unwrap();
                assert_ne!(m4, m3);
                assert_eq!(db.change_marker(board_id).await.unwrap(), m4);
                assert_eq!(db.change_marker(anderes.id).await.unwrap(), anderes_vorher);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn lasst_den_kennwert_bei_einer_zuruckgerollten_transaktion_unverandert() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                let vorher = db.change_marker(board_id).await.unwrap();
                let fehler = $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0001", project_id, board_id, 1))
                        .await?;
                    Err::<(), $crate::error::Error>($crate::error::Error::unexpected_message(
                        "Abbruch",
                    ))
                })
                .await
                .unwrap_err();
                assert_eq!(fehler.to_string(), "Abbruch");
                assert_eq!(db.change_marker(board_id).await.unwrap(), vorher);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn zahlt_die_kommentare_je_ticket_eines_boards() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0001", project_id, board_id, 1))
                        .await?;
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0002", project_id, board_id, 2))
                        .await?;
                    for body in ["a", "b"] {
                        u.comments()
                            .insert(&new_comment("ARC-0001", "F", body))
                            .await?;
                    }
                    Ok(())
                })
                .await
                .unwrap();
                let gezahlt = $crate::tx!(db, |u| { u.comments().count_by_board(board_id).await })
                    .await
                    .unwrap();
                assert_eq!(gezahlt, BTreeMap::from([("ARC-0001".to_string(), 2i64)]));
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn speichert_spec_aufgabe_branch_worktree_zielbranch_und_die_listen_am_ticket() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&Ticket {
                            spec: Some("001-login".into()),
                            spec_task: Some("T001".into()),
                            target_branch: Some("release/1".into()),
                            touches: vec!["src/login".into(), "src/äöü".into()],
                            consumes: vec!["GET /a".into()],
                            produces: vec!["<Login>".into()],
                            ..ticket_fixture("ARC-0001", project_id, board_id, 1)
                        })
                        .await
                })
                .await
                .unwrap();

                let ticket = $crate::tx!(db, |u| { u.tickets().by_id("ARC-0001", false).await })
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(ticket.spec.as_deref(), Some("001-login"));
                assert_eq!(ticket.spec_task.as_deref(), Some("T001"));
                assert_eq!(ticket.branch, None);
                assert_eq!(ticket.worktree, None);
                assert_eq!(ticket.target_branch.as_deref(), Some("release/1"));
                assert_eq!(ticket.touches, ["src/login", "src/äöü"]);
                assert_eq!(ticket.consumes, ["GET /a"]);
                assert_eq!(ticket.produces, ["<Login>"]);

                $crate::tx!(db, |u| {
                    let ticket = u.tickets().by_id("ARC-0001", true).await?.unwrap();
                    u.tickets()
                        .update(&Ticket {
                            branch: Some("ticket/ARC-0001-login".into()),
                            worktree: Some("C:/x/ARC-0001".into()),
                            target_branch: None,
                            touches: vec![],
                            rev: 2,
                            ..ticket
                        })
                        .await
                })
                .await
                .unwrap();

                let ticket = $crate::tx!(db, |u| { u.tickets().by_id("ARC-0001", false).await })
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(ticket.branch.as_deref(), Some("ticket/ARC-0001-login"));
                assert_eq!(ticket.worktree.as_deref(), Some("C:/x/ARC-0001"));
                assert_eq!(ticket.target_branch, None);
                assert!(ticket.touches.is_empty());
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn speichert_schritte_in_reihenfolge_hakt_ab_ersetzt_sie_und_zahlt_den_fortschritt(
            ) {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0001", project_id, board_id, 1))
                        .await?;
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0002", project_id, board_id, 2))
                        .await?;
                    u.steps()
                        .replace(
                            "ARC-0001",
                            &[
                                TicketStep {
                                    text: "Test schreiben".into(),
                                    done: false,
                                    detail: Some("```ts\nexpect(1).toBe(1);\n```".into()),
                                },
                                TicketStep {
                                    text: "Umsetzen".into(),
                                    done: false,
                                    detail: None,
                                },
                            ],
                        )
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();

                let schritte = $crate::tx!(db, |u| { u.steps().list("ARC-0001").await })
                    .await
                    .unwrap();
                assert_eq!(
                    schritte,
                    vec![
                        TicketStep {
                            text: "Test schreiben".into(),
                            done: false,
                            detail: Some("```ts\nexpect(1).toBe(1);\n```".into()),
                        },
                        TicketStep {
                            text: "Umsetzen".into(),
                            done: false,
                            detail: None,
                        },
                    ]
                );

                let gesetzt = $crate::tx!(db, |u| { u.steps().set_done("ARC-0001", 1, true).await })
                    .await
                    .unwrap();
                assert!(gesetzt);
                let daneben =
                    $crate::tx!(db, |u| { u.steps().set_done("ARC-0001", 5, true).await })
                        .await
                        .unwrap();
                assert!(!daneben);

                let fortschritt = $crate::tx!(db, |u| {
                    u.steps()
                        .progress(&["ARC-0001".to_string(), "ARC-0002".to_string()])
                        .await
                })
                .await
                .unwrap();
                assert_eq!(
                    fortschritt,
                    BTreeMap::from([(
                        "ARC-0001".to_string(),
                        StepProgress { done: 1, total: 2 }
                    )])
                );

                $crate::tx!(db, |u| {
                    u.steps()
                        .replace(
                            "ARC-0001",
                            &[TicketStep {
                                text: "Nur noch einer".into(),
                                done: true,
                                detail: None,
                            }],
                        )
                        .await
                })
                .await
                .unwrap();
                let schritte = $crate::tx!(db, |u| { u.steps().list("ARC-0001").await })
                    .await
                    .unwrap();
                assert_eq!(
                    schritte,
                    vec![TicketStep {
                        text: "Nur noch einer".into(),
                        done: true,
                        detail: None,
                    }]
                );
                let leer = $crate::tx!(db, |u| { u.steps().progress(&[]).await })
                    .await
                    .unwrap();
                assert_eq!(leer, BTreeMap::new());
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn verknupft_sessions_genau_einmal_und_andert_den_kennwert() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0001", project_id, board_id, 1))
                        .await?;
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0002", project_id, board_id, 2))
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();

                let vorher = db.change_marker(board_id).await.unwrap();
                let neu = $crate::tx!(db, |u| {
                    u.ticket_sessions().add("ARC-0002", "s-1", NOW).await
                })
                .await
                .unwrap();
                assert!(neu);
                assert_ne!(db.change_marker(board_id).await.unwrap(), vorher);

                let nochmal = $crate::tx!(db, |u| {
                    u.ticket_sessions().add("ARC-0002", "s-1", NOW).await
                })
                .await
                .unwrap();
                assert!(!nochmal);

                $crate::tx!(db, |u| {
                    u.ticket_sessions().add("ARC-0001", "s-1", NOW).await?;
                    u.ticket_sessions()
                        .add("ARC-0001", "s-2", "2026-10-13T11:00:00.000Z")
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();

                let tickets = $crate::tx!(db, |u| { u.ticket_sessions().tickets_of("s-1").await })
                    .await
                    .unwrap();
                assert_eq!(tickets, ["ARC-0001", "ARC-0002"]);
                let ohne = $crate::tx!(db, |u| { u.ticket_sessions().tickets_of("s-9").await })
                    .await
                    .unwrap();
                assert_eq!(ohne, Vec::<String>::new());

                let je_ticket = $crate::tx!(db, |u| {
                    u.ticket_sessions()
                        .by_tickets(&["ARC-0001".to_string(), "ARC-0002".to_string()])
                        .await
                })
                .await
                .unwrap();
                assert_eq!(
                    je_ticket,
                    BTreeMap::from([
                        (
                            "ARC-0001".to_string(),
                            vec!["s-1".to_string(), "s-2".to_string()]
                        ),
                        ("ARC-0002".to_string(), vec!["s-1".to_string()]),
                    ])
                );
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn liefert_kommentare_ohne_lesemarke_und_setzt_sie() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                let ids = $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0001", project_id, board_id, 1))
                        .await?;
                    let erster = u
                        .comments()
                        .insert(&new_comment("ARC-0001", "Florian", "Bitte prüfen"))
                        .await?;
                    u.comments()
                        .insert(&NewComment {
                            read_by_agent_at: Some(NOW.into()),
                            ..new_comment("ARC-0001", "claude", "Eigener")
                        })
                        .await?;
                    let dritter = u
                        .comments()
                        .insert(&new_comment("ARC-0001", "Florian", "Noch was"))
                        .await?;
                    Ok(vec![erster.id, dritter.id])
                })
                .await
                .unwrap();

                let ungelesen: Vec<String> = $crate::tx!(db, |u| {
                    u.comments().unread_by_agent(&["ARC-0001".to_string()]).await
                })
                .await
                .unwrap()
                .into_iter()
                .map(|c| c.body)
                .collect();
                assert_eq!(ungelesen, ["Bitte prüfen", "Noch was"]);

                let erster = ids[0];
                $crate::tx!(db, |u| {
                    u.comments()
                        .mark_read_by_agent(&[erster], "2026-10-13T12:00:00.000Z")
                        .await
                })
                .await
                .unwrap();

                let ungelesen: Vec<String> = $crate::tx!(db, |u| {
                    u.comments().unread_by_agent(&["ARC-0001".to_string()]).await
                })
                .await
                .unwrap()
                .into_iter()
                .map(|c| c.body)
                .collect();
                assert_eq!(ungelesen, ["Noch was"]);

                let alle = $crate::tx!(db, |u| { u.comments().list("ARC-0001").await })
                    .await
                    .unwrap();
                assert_eq!(
                    alle[0].read_by_agent_at.as_deref(),
                    Some("2026-10-13T12:00:00.000Z")
                );

                let leer = $crate::tx!(db, |u| { u.comments().unread_by_agent(&[]).await })
                    .await
                    .unwrap();
                assert_eq!(leer, vec![]);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn loscht_schritte_und_sessions_mit_dem_ticket() {
                let (db, _guard) = setup().await;
                let seed = seed_board(&db).await;
                let (project_id, board_id) = (seed.project_id, seed.board_id);
                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&ticket_fixture("ARC-0001", project_id, board_id, 1))
                        .await?;
                    u.steps()
                        .replace(
                            "ARC-0001",
                            &[TicketStep {
                                text: "A".into(),
                                done: false,
                                detail: None,
                            }],
                        )
                        .await?;
                    u.ticket_sessions().add("ARC-0001", "s-1", NOW).await?;
                    Ok(())
                })
                .await
                .unwrap();

                $crate::tx!(db, |u| { u.tickets().delete("ARC-0001").await })
                    .await
                    .unwrap();
                let schritte = $crate::tx!(db, |u| { u.steps().list("ARC-0001").await })
                    .await
                    .unwrap();
                assert_eq!(schritte, vec![]);
                let tickets = $crate::tx!(db, |u| { u.ticket_sessions().tickets_of("s-1").await })
                    .await
                    .unwrap();
                assert_eq!(tickets, Vec::<String>::new());
                db.close().await.unwrap();
            }
        }
    };
}
