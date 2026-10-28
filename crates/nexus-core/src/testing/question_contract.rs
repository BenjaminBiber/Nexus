//! Der gemeinsame Vertrag der Fragen-Repositories; portiert aus
//! `packages/core/tests/unit/board/helpers/QuestionRepositoryContract.ts`.

/// Gemeinsame Tests der Fragen-Repositories für jede Umsetzung von `BoardDatabase`.
///
/// Die Argumente sind dieselben wie bei [`crate::board_database_contract`].
///
/// ```
/// nexus_core::question_repository_contract!(
///     in_memory_questions,
///     || async { nexus_core::board::in_memory::InMemoryBoardDatabase::new() }
/// );
/// ```
#[macro_export]
macro_rules! question_repository_contract {
    ($name:ident, $create:expr) => {
        $crate::question_repository_contract!($name, $create, || async {});
    };
    ($name:ident, $create:expr, $reset:expr) => {
        $crate::question_repository_contract!($name, $create, $reset, || async {});
    };
    ($name:ident, $create:expr, $reset:expr, $lock:expr) => {
        mod $name {
            #![allow(unused_imports)]

            use ::std::collections::BTreeMap;

            use $crate::actors::AuthorKind;
            use $crate::board::database::{
                BoardDatabase, BoardQuestionFilter, BoardUnitOfWork, NewProject,
                NewQuestionComment, NewQuestionResearch,
            };
            use $crate::board::question_types::{
                BoardQuestion, BoardQuestionDecision, BoardQuestionDecisionOutcome,
                BoardQuestionHistoryEntry, BoardQuestionHistoryType, BoardQuestionOption,
                BoardQuestionResearchSummary,
            };
            use $crate::questions::QuestionState;
            use $crate::testing::*;

            /// Leere Datenbank samt angelegtem Benutzer; die Sperre hält der Fall bis zum Ende
            async fn setup() -> (impl BoardDatabase, impl Sized) {
                let guard = ($lock)().await;
                ($reset)().await;
                let db = ($create)().await;
                $crate::testing::upsert_user(&db).await;
                (db, guard)
            }

            /// Eine Option mit Standardwerten
            fn option(id: &str, title: &str, text: &str) -> BoardQuestionOption {
                BoardQuestionOption {
                    id: id.into(),
                    title: title.into(),
                    text: text.into(),
                    pros: Vec::new(),
                    cons: Vec::new(),
                    recommended: false,
                }
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn vergibt_fragenummern_je_projekt_fortlaufend_unabhangig_von_ticketnummern() {
                let (db, _guard) = setup().await;
                let project_id = seed_questions(&db).await.project_id;
                let nummern = $crate::tx!(db, |u| {
                    let erste = u.projects().take_question_number(project_id).await?;
                    let zweite = u.projects().take_question_number(project_id).await?;
                    Ok(vec![erste, zweite])
                })
                .await
                .unwrap();
                assert_eq!(nummern, [1, 2]);

                let projekt = $crate::tx!(db, |u| { u.projects().by_key("ARC").await })
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(projekt.next_question_number, 3);
                assert_eq!(projekt.next_ticket_number, 1);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn speichert_eine_frage_mit_tags_tickets_abhangigkeiten_und_entscheidung() {
                let (db, _guard) = setup().await;
                let project_id = seed_questions(&db).await.project_id;
                let frage = BoardQuestion {
                    title: "Umlaute äöü".into(),
                    description: "# Text".into(),
                    occasion: Some("Grill".into()),
                    origin_session: Some("s-1".into()),
                    tags: vec!["speicher".into(), "ui".into()],
                    ticket_ids: vec!["ARC-0001".into(), "ARC-0002".into()],
                    depends_on: vec!["ARC-F0002".into(), "ARC-0001".into()],
                    ..question_fixture("ARC-F0001", project_id, 1)
                };
                $crate::tx!(db, [frage], |u| {
                    u.questions()
                        .insert(&question_fixture("ARC-F0002", project_id, 2))
                        .await?;
                    u.questions().insert(&frage).await?;
                    Ok(())
                })
                .await
                .unwrap();

                let geladen =
                    $crate::tx!(db, |u| { u.questions().by_id("ARC-F0001", false).await })
                        .await
                        .unwrap();
                assert_eq!(geladen, Some(frage.clone()));

                let entschieden = BoardQuestion {
                    state: QuestionState::Answered,
                    rev: 2,
                    updated_at: QUESTION_LATER.into(),
                    decision: Some(BoardQuestionDecision {
                        outcome: BoardQuestionDecisionOutcome::Answered,
                        option: Some("O2".into()),
                        text: None,
                        reason: "passt".into(),
                        by_id: USER_ID.into(),
                        by_name: "Florian".into(),
                        at: QUESTION_LATER.into(),
                    }),
                    ..frage
                };
                $crate::tx!(db, [entschieden], |u| {
                    u.questions().update(&entschieden).await
                })
                .await
                .unwrap();
                let geladen =
                    $crate::tx!(db, |u| { u.questions().by_id("ARC-F0001", false).await })
                        .await
                        .unwrap();
                assert_eq!(geladen, Some(entschieden));

                let fehlt = $crate::tx!(db, |u| { u.questions().by_id("ARC-F0099", false).await })
                    .await
                    .unwrap();
                assert_eq!(fehlt, None);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn lehnt_ein_update_mit_veralteter_version_als_conflict_ab() {
                let (db, _guard) = setup().await;
                let project_id = seed_questions(&db).await.project_id;
                $crate::tx!(db, |u| {
                    u.questions()
                        .insert(&question_fixture("ARC-F0001", project_id, 1))
                        .await
                })
                .await
                .unwrap();

                $crate::tx!(db, |u| {
                    let frage = u.questions().by_id("ARC-F0001", true).await?.unwrap();
                    u.questions()
                        .update(&BoardQuestion {
                            title: "Neu".into(),
                            rev: 2,
                            ..frage
                        })
                        .await
                })
                .await
                .unwrap();

                let fehler = $crate::tx!(db, |u| {
                    let frage = u.questions().by_id("ARC-F0001", false).await?.unwrap();
                    u.questions()
                        .update(&BoardQuestion {
                            title: "Zu spät".into(),
                            rev: 2,
                            ..frage
                        })
                        .await
                })
                .await
                .unwrap_err();
                assert_eq!(fehler.code(), Some("conflict"));

                let frage = $crate::tx!(db, |u| { u.questions().by_id("ARC-F0001", false).await })
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(frage.title, "Neu");
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn filtert_nach_projekt_ids_betroffenen_tickets_und_zustanden() {
                let (db, _guard) = setup().await;
                let project_id = seed_questions(&db).await.project_id;
                let anderes = $crate::tx!(db, |u| {
                    u.projects()
                        .insert(&NewProject {
                            key: "OTH".into(),
                            name: "Other".into(),
                            created_at: QUESTION_NOW.into(),
                            created_by: USER_ID.into(),
                        })
                        .await
                })
                .await
                .unwrap();
                let anderes_id = anderes.id;
                $crate::tx!(db, |u| {
                    u.questions()
                        .insert(&BoardQuestion {
                            ticket_ids: vec!["ARC-0001".into()],
                            ..question_fixture("ARC-F0001", project_id, 1)
                        })
                        .await?;
                    u.questions()
                        .insert(&BoardQuestion {
                            state: QuestionState::Discarded,
                            ticket_ids: vec!["ARC-0001".into()],
                            ..question_fixture("ARC-F0002", project_id, 2)
                        })
                        .await?;
                    u.questions()
                        .insert(&question_fixture("OTH-F0001", anderes_id, 1))
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();

                /// IDs der Fragen zu einem Filter
                async fn ids<D: BoardDatabase>(db: &D, filter: BoardQuestionFilter) -> Vec<String> {
                    $crate::tx!(db, [filter], |u| { u.questions().list(&filter).await })
                        .await
                        .unwrap()
                        .into_iter()
                        .map(|q| q.id)
                        .collect()
                }

                assert_eq!(
                    ids(
                        &db,
                        BoardQuestionFilter {
                            project_id: Some(project_id),
                            ..BoardQuestionFilter::default()
                        }
                    )
                    .await,
                    ["ARC-F0001", "ARC-F0002"]
                );
                assert_eq!(
                    ids(
                        &db,
                        BoardQuestionFilter {
                            ids: Some(vec!["OTH-F0001".into()]),
                            ..BoardQuestionFilter::default()
                        }
                    )
                    .await,
                    ["OTH-F0001"]
                );
                assert_eq!(
                    ids(
                        &db,
                        BoardQuestionFilter {
                            ticket_ids: Some(vec!["ARC-0001".into()]),
                            states: Some(vec![QuestionState::Open]),
                            ..BoardQuestionFilter::default()
                        }
                    )
                    .await,
                    ["ARC-F0001"]
                );
                assert_eq!(
                    ids(
                        &db,
                        BoardQuestionFilter {
                            ticket_ids: Some(vec![]),
                            ..BoardQuestionFilter::default()
                        }
                    )
                    .await,
                    Vec::<String>::new()
                );
                assert_eq!(ids(&db, BoardQuestionFilter::default()).await.len(), 3);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn speichert_recherchen_mit_optionen_kommentare_und_verlauf_in_reihenfolge() {
                let (db, _guard) = setup().await;
                let project_id = seed_questions(&db).await.project_id;
                $crate::tx!(db, |u| {
                    u.questions()
                        .insert(&question_fixture("ARC-F0001", project_id, 1))
                        .await?;
                    u.question_research()
                        .insert(&NewQuestionResearch {
                            question_id: "ARC-F0001".into(),
                            at: QUESTION_NOW.into(),
                            by_id: USER_ID.into(),
                            by_name: "claude".into(),
                            kind: AuthorKind::Session,
                            context: "Kontext".into(),
                            options: vec![
                                BoardQuestionOption {
                                    pros: vec!["einfach".into()],
                                    ..option("O1", "Datei", "JSON")
                                },
                                BoardQuestionOption {
                                    cons: vec!["Aufwand".into(), "Schema".into()],
                                    recommended: true,
                                    ..option("O2", "DB", "SQL")
                                },
                            ],
                        })
                        .await?;
                    u.question_research()
                        .insert(&NewQuestionResearch {
                            question_id: "ARC-F0001".into(),
                            at: QUESTION_LATER.into(),
                            by_id: USER_ID.into(),
                            by_name: "claude".into(),
                            kind: AuthorKind::Subagent,
                            context: String::new(),
                            options: vec![option("O3", "Cache", "Speicher")],
                        })
                        .await?;
                    u.question_comments()
                        .insert(&NewQuestionComment {
                            question_id: "ARC-F0001".into(),
                            author_id: USER_ID.into(),
                            author_name: "Florian".into(),
                            author_kind: AuthorKind::Human,
                            body: "Hallo äöü".into(),
                            created_at: QUESTION_NOW.into(),
                        })
                        .await?;
                    u.question_history()
                        .insert(&[
                            BoardQuestionHistoryEntry {
                                question_id: "ARC-F0001".into(),
                                at: QUESTION_NOW.into(),
                                by_id: USER_ID.into(),
                                by_name: "claude".into(),
                                by_kind: Some(AuthorKind::Session),
                                entry_type: BoardQuestionHistoryType::Created,
                                text: "Frage angelegt".into(),
                            },
                            BoardQuestionHistoryEntry {
                                question_id: "ARC-F0001".into(),
                                at: QUESTION_LATER.into(),
                                by_id: USER_ID.into(),
                                by_name: "Florian".into(),
                                by_kind: None,
                                entry_type: BoardQuestionHistoryType::TicketDeleted,
                                text: "Ticket ARC-0009 gelöscht".into(),
                            },
                        ])
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();

                let recherchen =
                    $crate::tx!(db, |u| { u.question_research().list("ARC-F0001").await })
                        .await
                        .unwrap();
                let gesehen: Vec<(AuthorKind, Vec<String>)> = recherchen
                    .iter()
                    .map(|r| (r.kind, r.options.iter().map(|o| o.id.clone()).collect()))
                    .collect();
                assert_eq!(
                    gesehen,
                    vec![
                        (
                            AuthorKind::Session,
                            vec!["O1".to_string(), "O2".to_string()]
                        ),
                        (AuthorKind::Subagent, vec!["O3".to_string()]),
                    ]
                );
                assert!(recherchen[0].id > 0);
                assert_eq!(recherchen[0].question_id, "ARC-F0001");
                assert_eq!(recherchen[0].context, "Kontext");
                assert_eq!(recherchen[0].by_name, "claude");
                assert_eq!(recherchen[0].at, QUESTION_NOW);
                assert_eq!(recherchen[0].options[0].pros, ["einfach"]);
                assert!(!recherchen[0].options[0].recommended);
                assert_eq!(recherchen[0].options[1].cons, ["Aufwand", "Schema"]);
                assert!(recherchen[0].options[1].recommended);

                let zusammenfassung = $crate::tx!(db, |u| {
                    u.question_research()
                        .summaries(&["ARC-F0001".to_string(), "ARC-F0009".to_string()])
                        .await
                })
                .await
                .unwrap();
                assert_eq!(
                    zusammenfassung,
                    BTreeMap::from([(
                        "ARC-F0001".to_string(),
                        BoardQuestionResearchSummary {
                            count: 2,
                            option_count: 1,
                        }
                    )])
                );
                let leer = $crate::tx!(db, |u| { u.question_research().summaries(&[]).await })
                    .await
                    .unwrap();
                assert_eq!(leer, BTreeMap::new());

                let kommentare =
                    $crate::tx!(db, |u| { u.question_comments().list("ARC-F0001").await })
                        .await
                        .unwrap();
                assert_eq!(kommentare.len(), 1);
                assert!(kommentare[0].id > 0);
                assert_eq!(kommentare[0].body, "Hallo äöü");
                assert_eq!(kommentare[0].author_kind, AuthorKind::Human);
                assert_eq!(kommentare[0].author_name, "Florian");
                assert_eq!(kommentare[0].created_at, QUESTION_NOW);

                let verlauf = $crate::tx!(db, |u| { u.question_history().list("ARC-F0001").await })
                    .await
                    .unwrap();
                let gesehen: Vec<(BoardQuestionHistoryType, Option<AuthorKind>, String)> = verlauf
                    .iter()
                    .map(|h| (h.entry_type, h.by_kind, h.text.clone()))
                    .collect();
                assert_eq!(
                    gesehen,
                    vec![
                        (
                            BoardQuestionHistoryType::Created,
                            Some(AuthorKind::Session),
                            "Frage angelegt".to_string()
                        ),
                        (
                            BoardQuestionHistoryType::TicketDeleted,
                            None,
                            "Ticket ARC-0009 gelöscht".to_string()
                        ),
                    ]
                );
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn entfernt_bezuge_auf_ein_ticket_und_liefert_die_betroffenen_fragen() {
                let (db, _guard) = setup().await;
                let project_id = seed_questions(&db).await.project_id;
                $crate::tx!(db, |u| {
                    u.questions()
                        .insert(&BoardQuestion {
                            ticket_ids: vec!["ARC-0001".into()],
                            ..question_fixture("ARC-F0001", project_id, 1)
                        })
                        .await?;
                    u.questions()
                        .insert(&BoardQuestion {
                            depends_on: vec!["ARC-0001".into(), "ARC-0002".into()],
                            ..question_fixture("ARC-F0002", project_id, 2)
                        })
                        .await?;
                    u.questions()
                        .insert(&BoardQuestion {
                            ticket_ids: vec!["ARC-0002".into()],
                            ..question_fixture("ARC-F0003", project_id, 3)
                        })
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();

                // Lesen allein ändert nichts; es liefert dieselben Fragen wie das Entfernen
                let betroffen = $crate::tx!(db, |u| {
                    u.questions().ids_referencing_ticket("ARC-0001").await
                })
                .await
                .unwrap();
                assert_eq!(betroffen, ["ARC-F0001", "ARC-F0002"]);
                let keine = $crate::tx!(db, |u| {
                    u.questions().ids_referencing_ticket("ARC-0009").await
                })
                .await
                .unwrap();
                assert_eq!(keine, Vec::<String>::new());
                let frage = $crate::tx!(db, |u| { u.questions().by_id("ARC-F0002", false).await })
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(frage.depends_on, ["ARC-0001", "ARC-0002"]);

                let entfernt =
                    $crate::tx!(db, |u| { u.questions().remove_ticket("ARC-0001").await })
                        .await
                        .unwrap();
                assert_eq!(entfernt, ["ARC-F0001", "ARC-F0002"]);

                let danach = $crate::tx!(db, |u| {
                    u.questions()
                        .list(&BoardQuestionFilter {
                            project_id: Some(project_id),
                            ..BoardQuestionFilter::default()
                        })
                        .await
                })
                .await
                .unwrap();
                let gesehen: Vec<(String, Vec<String>, Vec<String>, i32)> = danach
                    .iter()
                    .map(|q| {
                        (
                            q.id.clone(),
                            q.ticket_ids.clone(),
                            q.depends_on.clone(),
                            q.rev,
                        )
                    })
                    .collect();
                assert_eq!(
                    gesehen,
                    vec![
                        ("ARC-F0001".to_string(), vec![], vec![], 1),
                        (
                            "ARC-F0002".to_string(),
                            vec![],
                            vec!["ARC-0002".to_string()],
                            1
                        ),
                        (
                            "ARC-F0003".to_string(),
                            vec!["ARC-0002".to_string()],
                            vec![],
                            1
                        ),
                    ]
                );
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn andert_den_fragen_kennwert_und_den_board_kennwert_nur_bei_verknupften_tickets()
            {
                let (db, _guard) = setup().await;
                let seed = seed_questions(&db).await;
                let (project_id, board_id, other_id) =
                    (seed.project_id, seed.board_id, seed.other_id);
                let q0 = db.question_marker(project_id).await.unwrap();
                let b0 = db.change_marker(board_id).await.unwrap();
                let o0 = db.change_marker(other_id).await.unwrap();

                $crate::tx!(db, |u| {
                    u.questions()
                        .insert(&question_fixture("ARC-F0001", project_id, 1))
                        .await
                })
                .await
                .unwrap();
                let q1 = db.question_marker(project_id).await.unwrap();
                assert_ne!(q1, q0);
                assert_eq!(db.change_marker(board_id).await.unwrap(), b0);

                $crate::tx!(db, |u| {
                    let frage = u.questions().by_id("ARC-F0001", true).await?.unwrap();
                    u.questions()
                        .update(&BoardQuestion {
                            ticket_ids: vec!["ARC-0001".into()],
                            rev: 2,
                            ..frage
                        })
                        .await
                })
                .await
                .unwrap();
                let b1 = db.change_marker(board_id).await.unwrap();
                assert_ne!(b1, b0);
                assert_ne!(db.question_marker(project_id).await.unwrap(), q1);
                assert_eq!(db.change_marker(other_id).await.unwrap(), o0);

                $crate::tx!(db, |u| {
                    let frage = u.questions().by_id("ARC-F0001", true).await?.unwrap();
                    u.questions()
                        .update(&BoardQuestion {
                            state: QuestionState::Answered,
                            rev: 3,
                            ..frage
                        })
                        .await
                })
                .await
                .unwrap();
                assert_ne!(db.change_marker(board_id).await.unwrap(), b1);
                assert_eq!(db.change_marker(other_id).await.unwrap(), o0);

                // Ohne Änderung bleibt der Kennwert gleich (auch nach einem reinen Lesezugriff)
                let stabil = db.question_marker(project_id).await.unwrap();
                $crate::tx!(db, |u| {
                    u.questions()
                        .list(&BoardQuestionFilter {
                            project_id: Some(project_id),
                            ..BoardQuestionFilter::default()
                        })
                        .await
                        .map(|_| ())
                })
                .await
                .unwrap();
                assert_eq!(db.question_marker(project_id).await.unwrap(), stabil);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn verweigert_das_loschen_eines_tickets_mit_verknupfter_frage() {
                let (db, _guard) = setup().await;
                let project_id = seed_questions(&db).await.project_id;
                $crate::tx!(db, |u| {
                    u.questions()
                        .insert(&BoardQuestion {
                            ticket_ids: vec!["ARC-0001".into()],
                            ..question_fixture("ARC-F0001", project_id, 1)
                        })
                        .await
                })
                .await
                .unwrap();

                let fehler = $crate::tx!(db, |u| { u.tickets().delete("ARC-0001").await })
                    .await
                    .unwrap_err();
                assert!(fehler.harness().is_some(), "erwartbarer Fehler: {fehler}");
                let ticket = $crate::tx!(db, |u| { u.tickets().by_id("ARC-0001", false).await })
                    .await
                    .unwrap();
                assert!(ticket.is_some());

                $crate::tx!(db, |u| {
                    u.questions().remove_ticket("ARC-0001").await.map(|_| ())
                })
                .await
                .unwrap();
                $crate::tx!(db, |u| { u.tickets().delete("ARC-0001").await })
                    .await
                    .unwrap();
                let ticket = $crate::tx!(db, |u| { u.tickets().by_id("ARC-0001", false).await })
                    .await
                    .unwrap();
                assert_eq!(ticket, None);
                db.close().await.unwrap();
            }
        }
    };
}
