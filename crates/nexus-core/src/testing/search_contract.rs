//! Der gemeinsame Vertrag der Suche über alle Boards; portiert aus
//! `packages/core/tests/unit/board/helpers/SearchContract.ts`.

/// Gemeinsame Fälle der Suche für jede Umsetzung von `BoardDatabase`.
///
/// Die Argumente sind dieselben wie bei [`crate::board_database_contract`].
///
/// ```
/// nexus_core::search_contract!(
///     in_memory_search,
///     || async { nexus_core::board::in_memory::InMemoryBoardDatabase::new() }
/// );
/// ```
#[macro_export]
macro_rules! search_contract {
    ($name:ident, $create:expr) => {
        $crate::search_contract!($name, $create, || async {});
    };
    ($name:ident, $create:expr, $reset:expr) => {
        $crate::search_contract!($name, $create, $reset, || async {});
    };
    ($name:ident, $create:expr, $reset:expr, $lock:expr) => {
        $crate::search_contract!($name, $create, $reset, $lock,);
    };
    ($name:ident, $create:expr, $reset:expr, $lock:expr, $(#[$attr:meta])*) => {
        mod $name {
            #![allow(unused_imports)]

            use ::std::sync::Arc;

            use $crate::actors::AuthorKind;
            use $crate::board::basics::{BoardActor, TicketStatus};
            use $crate::board::database::{BoardDatabase, BoardUnitOfWork, NewQuestionComment};
            use $crate::board::note_service::{AddNoteOptions, NewNoteText, NoteService};
            use $crate::board::question_types::{
                BoardQuestion, BoardQuestionDecision, BoardQuestionDecisionOutcome,
            };
            use $crate::board::search_service::{BoardSearchService, SearchInput};
            use $crate::board::search_types::{SearchField, SearchKind};
            use $crate::board::services::BoardServices;
            use $crate::board::ticket_service::{NewTicketInput, TicketPatch, WriteOptions};
            use $crate::clock::Clock;
            use $crate::questions::QuestionState;
            use $crate::testing::*;

            /// Die drei Boards des Grundbestands
            struct Boards {
                sprint: i32,
                zweites: i32,
                incidents: i32,
            }

            /// Grundbestand der Suche: zwei Projekte, drei Boards, Tickets, ein Kommentar,
            /// eine entschiedene Frage mit Kommentar, eine Notiz
            async fn setup() -> (
                Arc<impl BoardDatabase>,
                BoardSearchService<impl BoardDatabase>,
                Boards,
                impl Sized,
            ) {
                let guard = ($lock)().await;
                ($reset)().await;
                let db = Arc::new(($create)().await);
                $crate::testing::upsert_user(db.as_ref()).await;
                let clock = Clock::fixed(NOW.parse().expect("der Zeitpunkt der Vorlage ist gültig"));
                let actor = BoardActor {
                    user_id: USER_ID.into(),
                    name: "claude".into(),
                };
                let s = BoardServices::with_memory_paths(db.clone(), clock.clone());
                s.projects.create("ARC", "Arches", &actor).await.unwrap();
                s.projects.create("SV1", "Service", &actor).await.unwrap();
                let boards = Boards {
                    sprint: s.boards.create("ARC", "Sprint", &actor).await.unwrap().id,
                    zweites: s.boards.create("ARC", "Zweites", &actor).await.unwrap().id,
                    incidents: s.boards.create("SV1", "Incidents", &actor).await.unwrap().id,
                };

                s.tickets
                    .create(
                        boards.sprint,
                        &NewTicketInput {
                            description: Some("Formular mit Passwort".into()),
                            acceptance: Some("Test ist grün".into()),
                            ..NewTicketInput::titled("Login bauen")
                        },
                        &actor,
                        &WriteOptions::default(),
                    )
                    .await
                    .unwrap();
                s.tickets
                    .update(
                        "ARC-0001",
                        &TicketPatch {
                            plan: Some("Plan: OAuth einbauen".into()),
                            handbook: Some("Vorher npm test laufen lassen".into()),
                            ..TicketPatch::default()
                        },
                        &actor,
                        &WriteOptions::default(),
                    )
                    .await
                    .unwrap();
                s.tickets
                    .create(
                        boards.zweites,
                        &NewTicketInput::titled("Export"),
                        &actor,
                        &WriteOptions::default(),
                    )
                    .await
                    .unwrap();
                s.comments
                    .add("ARC-0002", "Zwischenstand zur Speicherwahl", &actor, false)
                    .await
                    .unwrap();
                s.tickets
                    .create(
                        boards.incidents,
                        &NewTicketInput {
                            status: Some(TicketStatus::InProgress),
                            ..NewTicketInput::titled("Incident Speicher voll")
                        },
                        &actor,
                        &WriteOptions::default(),
                    )
                    .await
                    .unwrap();

                // Die entschiedene Frage samt Kommentar unmittelbar über die Repositories:
                // `BoardQuestionService` bringt erst P3-3
                let project_id = $crate::tx!(db, |u| { u.projects().by_key("ARC").await })
                    .await
                    .unwrap()
                    .unwrap()
                    .id;
                let question = BoardQuestion {
                    title: "Welcher Speicher?".into(),
                    description: "Varianten".into(),
                    state: QuestionState::Answered,
                    ticket_ids: vec!["ARC-0002".into()],
                    decision: Some(BoardQuestionDecision {
                        outcome: BoardQuestionDecisionOutcome::Answered,
                        option: None,
                        text: Some("SQL Server".into()),
                        reason: "Lizenz vorhanden".into(),
                        by_id: USER_ID.into(),
                        by_name: "Florian".into(),
                        at: QUESTION_LATER.into(),
                    }),
                    ..question_fixture("ARC-F0001", project_id, 1)
                };
                $crate::tx!(db, [question], |u| {
                    u.questions().insert(&question).await?;
                    u.question_comments()
                        .insert(&NewQuestionComment {
                            question_id: "ARC-F0001".into(),
                            author_id: USER_ID.into(),
                            author_name: "Florian".into(),
                            author_kind: AuthorKind::Human,
                            body: "Kommentar zur Lizenz".into(),
                            created_at: QUESTION_LATER.into(),
                        })
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();

                s.notes
                    .add(
                        boards.sprint,
                        &NewNoteText {
                            title: "Befund: Cache leert sich".into(),
                            body: Some("zweite Zeile".into()),
                        },
                        &actor,
                        &AddNoteOptions::default(),
                    )
                    .await
                    .unwrap();
                s.tickets
                    .create(
                        boards.sprint,
                        &NewTicketInput {
                            description: Some(format!("{} Fundstelle am Ende", "x".repeat(9000))),
                            ..NewTicketInput::titled("Rabatt 100% auf _x_ [a]")
                        },
                        &actor,
                        &WriteOptions::default(),
                    )
                    .await
                    .unwrap();

                let search = BoardSearchService::new(db.clone());
                (db, search, boards, guard)
            }

            /// Art, ID und Feld der Treffer
            fn found(hits: &[$crate::board::search_types::SearchHit]) -> Vec<(SearchKind, &str, SearchField)> {
                hits.iter()
                    .map(|hit| (hit.kind, hit.id.as_str(), hit.field))
                    .collect()
            }

            /// Nur die IDs der Treffer
            fn ids(hits: &[$crate::board::search_types::SearchHit]) -> Vec<&str> {
                hits.iter().map(|hit| hit.id.as_str()).collect()
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn findet_titel_kommentare_und_fragen_uber_projekte_und_boards() {
                let (db, search, boards, _guard) = setup().await;
                let result = search.search(&SearchInput::query("speicher")).await.unwrap();
                assert_eq!(result.query, "speicher");
                assert_eq!(result.terms, ["speicher"]);
                assert!(!result.truncated);
                assert_eq!(
                    found(&result.hits),
                    [
                        (SearchKind::Ticket, "SV1-0001", SearchField::Title),
                        (SearchKind::Question, "ARC-F0001", SearchField::Title),
                        (SearchKind::Ticket, "ARC-0002", SearchField::Comment),
                    ]
                );
                let erster = &result.hits[0];
                assert_eq!(erster.ticket_id.as_deref(), Some("SV1-0001"));
                assert_eq!(erster.status, "in_progress");
                assert_eq!(erster.project.key, "SV1");
                assert_eq!(erster.project.name, "Service");
                assert_eq!(
                    erster.board.as_ref().map(|b| (b.id, b.name.as_str())),
                    Some((boards.incidents, "Incidents"))
                );
                let zweiter = &result.hits[1];
                assert_eq!(zweiter.ticket_id, None);
                assert_eq!(zweiter.board, None);
                assert_eq!(zweiter.status, "answered");
                assert_eq!(zweiter.project.key, "ARC");
                let dritter = &result.hits[2];
                assert_eq!(dritter.title, "Export");
                assert_eq!(dritter.board.as_ref().map(|b| b.name.as_str()), Some("Zweites"));
                assert!(dritter.at.is_some());
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn hebt_die_fundstelle_hervor() {
                let (db, search, _boards, _guard) = setup().await;
                let result = search.search(&SearchInput::query("OAUTH")).await.unwrap();
                let hit = &result.hits[0];
                assert_eq!(hit.id, "ARC-0001");
                assert_eq!(hit.field, SearchField::Plan);
                assert_eq!(hit.snippet, "Plan: OAuth einbauen");
                let text: Vec<u16> = hit.snippet.encode_utf16().collect();
                let markiert: Vec<String> = hit
                    .matches
                    .iter()
                    .map(|(from, to)| {
                        String::from_utf16(&text[*from as usize..*to as usize]).unwrap()
                    })
                    .collect();
                assert_eq!(markiert, ["OAuth"]);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn verlangt_alle_worter_im_selben_feld() {
                let (db, search, _boards, _guard) = setup().await;
                assert_eq!(
                    found(&search.search(&SearchInput::query("npm laufen")).await.unwrap().hits),
                    [(SearchKind::Ticket, "ARC-0001", SearchField::Handbook)]
                );
                assert!(search
                    .search(&SearchInput::query("Login Passwort"))
                    .await
                    .unwrap()
                    .hits
                    .is_empty());
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn findet_abnahme_entscheidung_und_notiz() {
                let (db, search, _boards, _guard) = setup().await;
                assert_eq!(
                    found(&search.search(&SearchInput::query("grün")).await.unwrap().hits),
                    [(SearchKind::Ticket, "ARC-0001", SearchField::Acceptance)]
                );
                assert_eq!(
                    found(&search.search(&SearchInput::query("lizenz")).await.unwrap().hits),
                    [(SearchKind::Question, "ARC-F0001", SearchField::Decision)]
                );
                let notizen = search.search(&SearchInput::query("cache")).await.unwrap();
                let note = &notizen.hits[0];
                assert_eq!(note.kind, SearchKind::Note);
                assert_eq!(note.title, "Befund: Cache leert sich");
                assert_eq!(note.board.as_ref().map(|b| b.name.as_str()), Some("Sprint"));
                assert_eq!(note.project.key, "ARC");
                assert_eq!(note.status, "open");
                assert_eq!(note.ticket_id, None);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn filtert_nach_art() {
                let (db, search, _boards, _guard) = setup().await;
                let nur_fragen = search
                    .search(&SearchInput {
                        kinds: vec![SearchKind::Question],
                        ..SearchInput::query("speicher")
                    })
                    .await
                    .unwrap();
                assert_eq!(ids(&nur_fragen.hits), ["ARC-F0001"]);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn findet_sonderzeichen_wortlich() {
                let (db, search, _boards, _guard) = setup().await;
                for begriff in ["100%", "_x_", "[a]"] {
                    assert_eq!(
                        ids(&search.search(&SearchInput::query(begriff)).await.unwrap().hits),
                        ["ARC-0003"],
                        "{begriff}"
                    );
                }
                assert!(search
                    .search(&SearchInput::query("Ra%tt"))
                    .await
                    .unwrap()
                    .hits
                    .is_empty());
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn liefert_die_fundstelle_auch_hinter_8000_zeichen() {
                let (db, search, _boards, _guard) = setup().await;
                let result = search.search(&SearchInput::query("Fundstelle")).await.unwrap();
                let hit = &result.hits[0];
                assert_eq!(hit.id, "ARC-0003");
                assert_eq!(hit.field, SearchField::Description);
                assert!(hit.snippet.starts_with('…'));
                assert!(hit.snippet.contains("Fundstelle am Ende"));
                assert_eq!(hit.matches.len(), 1);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn pruft_die_eingabe() {
                let (db, search, _boards, _guard) = setup().await;
                assert_eq!(
                    search.search(&SearchInput::query(" a ")).await.unwrap_err().code(),
                    Some("query-short")
                );
                assert_eq!(
                    search
                        .search(&SearchInput::query("x".repeat(101)))
                        .await
                        .unwrap_err()
                        .code(),
                    Some("query-long")
                );
                assert_eq!(
                    search
                        .search(&SearchInput::query("a b c d e f"))
                        .await
                        .unwrap_err()
                        .code(),
                    Some("query-long")
                );
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            $(#[$attr])*
            async fn begrenzt_die_treffer_und_meldet_das() {
                let (db, search, boards, _guard) = setup().await;
                let clock = Clock::fixed(NOW.parse().expect("der Zeitpunkt der Vorlage ist gültig"));
                let actor = BoardActor {
                    user_id: USER_ID.into(),
                    name: "claude".into(),
                };
                let s = BoardServices::with_memory_paths(db.clone(), clock);
                for i in 0..31 {
                    s.tickets
                        .create(
                            boards.zweites,
                            &NewTicketInput::titled(format!("Massen {i}")),
                            &actor,
                            &WriteOptions::default(),
                        )
                        .await
                        .unwrap();
                }
                let alle = search.search(&SearchInput::query("massen")).await.unwrap();
                assert_eq!(alle.hits.len(), 30);
                assert!(alle.truncated);
                let wenige = search
                    .search(&SearchInput {
                        limit: Some(5),
                        ..SearchInput::query("massen")
                    })
                    .await
                    .unwrap();
                assert_eq!(wenige.hits.len(), 5);
                assert!(wenige.truncated);
                db.close().await.unwrap();
            }
        }
    };
}
