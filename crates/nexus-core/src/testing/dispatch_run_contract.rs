//! Der gemeinsame Vertrag der Lauf- und Auswertungs-Repositories; portiert aus
//! `packages/core/tests/unit/board/helpers/DispatchRunContract.ts`.

/// Gemeinsame Tests der Dispatcher-Läufe für jede Umsetzung von `BoardDatabase`.
///
/// Die Argumente sind dieselben wie bei [`crate::board_database_contract`].
///
/// ```
/// nexus_core::dispatch_run_contract!(
///     in_memory_runs,
///     || async { nexus_core::board::in_memory::InMemoryBoardDatabase::new() }
/// );
/// ```
#[macro_export]
macro_rules! dispatch_run_contract {
    ($name:ident, $create:expr) => {
        $crate::dispatch_run_contract!($name, $create, || async {});
    };
    ($name:ident, $create:expr, $reset:expr) => {
        $crate::dispatch_run_contract!($name, $create, $reset, || async {});
    };
    ($name:ident, $create:expr, $reset:expr, $lock:expr) => {
        mod $name {
            #![allow(unused_imports)]

            use ::std::collections::BTreeMap;

            use $crate::board::basics::TicketStatus;
            use $crate::board::database::{BoardDatabase, BoardUnitOfWork};
            use $crate::board::dispatch_run_types::{
                DispatchCommentCounts, DispatchConfigVersion, DispatchProfileReason, DispatchRun,
                DispatchRunFilter, DispatchRunOutcome, DispatchRunSource,
            };
            use $crate::board::types::{HistoryEntry, Ticket};
            use $crate::testing::*;

            /// Leere Datenbank samt angelegtem Benutzer; die Sperre hält der Fall bis zum Ende
            async fn setup() -> (impl BoardDatabase, impl Sized) {
                let guard = ($lock)().await;
                ($reset)().await;
                let db = ($create)().await;
                $crate::testing::upsert_user(&db).await;
                (db, guard)
            }

            /// Läufe eines Projekts im Zeitraum
            fn range(project_id: i32, from: &str, to: &str) -> DispatchRunFilter {
                DispatchRunFilter {
                    project_id,
                    board_id: None,
                    from: from.to_string(),
                    to: to.to_string(),
                }
            }

            /// Eine Fassung der Dispatcher-Einstellung
            fn version(hash: &str, first_seen_at: &str, content: &str) -> DispatchConfigVersion {
                DispatchConfigVersion {
                    hash: hash.to_string(),
                    dispatcher: "dispatcher".into(),
                    first_seen_at: first_seen_at.to_string(),
                    content: content.to_string(),
                }
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn speichert_laufe_vollstandig_lehnt_doppelte_ids_ab_und_filtert() {
                let (db, _guard) = setup().await;
                let seed = seed_dispatch(&db).await;
                let r1 = DispatchRun {
                    board_id: Some(seed.b1),
                    started_at: "2026-10-01T00:00:00.000Z".into(),
                    ..run_fixture("ARC-0001-work-1", "ARC-0001", seed.arc)
                };
                let r2 = DispatchRun {
                    board_id: Some(seed.b2),
                    started_at: "2026-10-05T12:00:00.000Z".into(),
                    outcome: DispatchRunOutcome::Abort,
                    abort_category: Some("timeout".into()),
                    abort_reason: Some("Zeitlimit äöü".into()),
                    cost_usd: None,
                    model: None,
                    ended_at: None,
                    duration_ms: None,
                    num_turns: None,
                    exit_code: None,
                    timed_out: true,
                    input_tokens: None,
                    output_tokens: None,
                    cache_read_tokens: None,
                    cache_write_tokens: None,
                    model_usage: None,
                    escalated_from: Some("klein".into()),
                    attempt: Some(2),
                    ..run_fixture("ARC-0002-work-1", "ARC-0002", seed.arc)
                };
                let r3 = DispatchRun {
                    source: DispatchRunSource::Comments,
                    started_at: "2026-09-01T00:00:00.000Z".into(),
                    ..run_fixture("c42", "OTH-0001", seed.oth)
                };

                let angelegt = $crate::tx!(db, [r1, r2, r3], |u| {
                    let a = u.dispatch_runs().insert(&r1).await?;
                    let b = u.dispatch_runs().insert(&r2).await?;
                    let c = u.dispatch_runs().insert(&r3).await?;
                    Ok(vec![a, b, c])
                })
                .await
                .unwrap();
                assert_eq!(angelegt, [true, true, true]);

                let nochmal = $crate::tx!(db, [r1], |u| {
                    u.dispatch_runs()
                        .insert(&DispatchRun {
                            outcome: DispatchRunOutcome::Abort,
                            ..r1
                        })
                        .await
                })
                .await
                .unwrap();
                assert!(!nochmal);

                let arc = seed.arc;
                let alle = $crate::tx!(db, |u| {
                    u.dispatch_runs()
                        .list(&range(
                            arc,
                            "2026-10-01T00:00:00.000Z",
                            "2026-10-06T00:00:00.000Z",
                        ))
                        .await
                })
                .await
                .unwrap();
                assert_eq!(alle, vec![r1.clone(), r2.clone()]);

                // Ende ausschließlich
                let keine = $crate::tx!(db, |u| {
                    u.dispatch_runs()
                        .list(&range(
                            arc,
                            "2026-10-01T00:00:00.001Z",
                            "2026-10-05T12:00:00.000Z",
                        ))
                        .await
                })
                .await
                .unwrap();
                assert_eq!(keine, vec![]);

                // Board-Filter
                let b2 = seed.b2;
                let nur_b2: Vec<String> = $crate::tx!(db, |u| {
                    u.dispatch_runs()
                        .list(&DispatchRunFilter {
                            board_id: Some(b2),
                            ..range(arc, "2026-01-01T00:00:00.000Z", "2027-01-01T00:00:00.000Z")
                        })
                        .await
                })
                .await
                .unwrap()
                .into_iter()
                .map(|r| r.id)
                .collect();
                assert_eq!(nur_b2, ["ARC-0002-work-1"]);

                let erster = $crate::tx!(db, |u| {
                    u.dispatch_runs()
                        .first_started(arc, DispatchRunSource::Dispatcher)
                        .await
                })
                .await
                .unwrap();
                assert_eq!(erster.as_deref(), Some("2026-10-01T00:00:00.000Z"));
                let ohne = $crate::tx!(db, |u| {
                    u.dispatch_runs()
                        .first_started(arc, DispatchRunSource::Comments)
                        .await
                })
                .await
                .unwrap();
                assert_eq!(ohne, None);

                let mut vorhanden = $crate::tx!(db, |u| {
                    u.dispatch_runs()
                        .existing(&[
                            "c42".to_string(),
                            "c43".to_string(),
                            "ARC-0001-work-1".to_string(),
                        ])
                        .await
                })
                .await
                .unwrap();
                vorhanden.sort();
                assert_eq!(vorhanden, ["ARC-0001-work-1", "c42"]);
                let leer = $crate::tx!(db, |u| { u.dispatch_runs().existing(&[]).await })
                    .await
                    .unwrap();
                assert_eq!(leer, Vec::<String>::new());
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn liefert_laufe_aller_projekte_im_zeitraum_altester_zuerst() {
                let (db, _guard) = setup().await;
                let seed = seed_dispatch(&db).await;
                let a = DispatchRun {
                    started_at: "2026-10-02T00:00:00.000Z".into(),
                    ..run_fixture("A", "ARC-0001", seed.arc)
                };
                let b = DispatchRun {
                    started_at: "2026-10-01T00:00:00.000Z".into(),
                    ..run_fixture("B", "OTH-0001", seed.oth)
                };
                let c = DispatchRun {
                    started_at: "2026-10-09T00:00:00.000Z".into(),
                    ..run_fixture("C", "OTH-0002", seed.oth)
                };
                $crate::tx!(db, [a, b, c], |u| {
                    for lauf in [&a, &b, &c] {
                        u.dispatch_runs().insert(lauf).await?;
                    }
                    Ok(())
                })
                .await
                .unwrap();

                let im_zeitraum = $crate::tx!(db, |u| {
                    u.dispatch_runs()
                        .list_all("2026-10-01T00:00:00.000Z", "2026-10-09T00:00:00.000Z")
                        .await
                })
                .await
                .unwrap();
                assert_eq!(im_zeitraum, vec![b.clone(), a.clone()]);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn gibt_die_dashboard_details_eines_laufs_unverandert_zuruck() {
                let (db, _guard) = setup().await;
                let seed = seed_dispatch(&db).await;
                let voll = DispatchRun {
                    board_id: Some(seed.b1),
                    started_at: "2026-10-02T00:00:00.000Z".into(),
                    max_turns: Some(250),
                    timeout_minutes: Some(90),
                    profile_reason: Some(DispatchProfileReason::Rule),
                    rule_index: Some(3),
                    config_hash: Some("a".repeat(64)),
                    denied_count: Some(2),
                    denied_tools: Some("Bash,WebFetch".into()),
                    ..run_fixture("ARC-0010-work-1", "ARC-0010", seed.arc)
                };
                let leer = DispatchRun {
                    board_id: Some(seed.b1),
                    started_at: "2026-10-02T01:00:00.000Z".into(),
                    ..run_fixture("ARC-0011-work-1", "ARC-0011", seed.arc)
                };
                $crate::tx!(db, [voll, leer], |u| {
                    u.dispatch_runs().insert(&voll).await?;
                    u.dispatch_runs().insert(&leer).await?;
                    Ok(())
                })
                .await
                .unwrap();

                let arc = seed.arc;
                let geladen = $crate::tx!(db, |u| {
                    u.dispatch_runs()
                        .list(&range(
                            arc,
                            "2026-10-02T00:00:00.000Z",
                            "2026-10-03T00:00:00.000Z",
                        ))
                        .await
                })
                .await
                .unwrap();
                assert_eq!(geladen, vec![voll.clone(), leer.clone()]);
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn findet_erledigte_tickets_ihre_statuswechsel_und_dispatch_kommentare() {
                let (db, _guard) = setup().await;
                let seed = seed_dispatch(&db).await;
                let (arc, oth, b1, b2, b3) = (seed.arc, seed.oth, seed.b1, seed.b2, seed.b3);
                $crate::tx!(db, |u| {
                    u.tickets()
                        .insert(&Ticket {
                            status: TicketStatus::Done,
                            ..ticket_fixture("ARC-0001", arc, b1, 1)
                        })
                        .await?;
                    u.tickets()
                        .insert(&Ticket {
                            status: TicketStatus::Done,
                            ..ticket_fixture("ARC-0002", arc, b2, 2)
                        })
                        .await?;
                    u.tickets()
                        .insert(&Ticket {
                            status: TicketStatus::Done,
                            ..ticket_fixture("ARC-0003", arc, b1, 3)
                        })
                        .await?;
                    u.tickets()
                        .insert(&Ticket {
                            status: TicketStatus::Done,
                            ..ticket_fixture("OTH-0001", oth, b3, 1)
                        })
                        .await?;
                    u.history()
                        .insert(&[
                            status_entry("ARC-0001", "2026-10-02T10:00:00.000Z", "done"),
                            HistoryEntry {
                                field: "title".into(),
                                ..status_entry("ARC-0001", "2026-10-02T11:00:00.000Z", "x")
                            },
                            status_entry("ARC-0002", "2026-10-03T10:00:00.000Z", "done"),
                            // ARC-0003: erst im Zeitraum, zuletzt danach erledigt – zählt nicht
                            status_entry("ARC-0003", "2026-10-03T10:00:00.000Z", "done"),
                            status_entry("ARC-0003", "2026-10-04T10:00:00.000Z", "in_progress"),
                            status_entry("ARC-0003", "2026-10-20T10:00:00.000Z", "done"),
                            status_entry("OTH-0001", "2026-10-03T10:00:00.000Z", "done"),
                        ])
                        .await?;
                    for body in [
                        "[dispatch] Start: an junior-ARC-0001 übergeben",
                        "[dispatch] Abbruch Versuch 1/3: x.",
                        "[dispatch] Start: wieder",
                        "Mensch: [dispatch] Abbruch",
                    ] {
                        u.comments()
                            .insert(&new_comment("ARC-0001", "dispatcher", body))
                            .await?;
                    }
                    u.comments()
                        .insert(&new_comment(
                            "OTH-0001",
                            "dispatcher",
                            "[dispatch] Start: an junior-OTH-0001",
                        ))
                        .await?;
                    Ok(())
                })
                .await
                .unwrap();

                let erledigt = $crate::tx!(db, |u| {
                    u.dispatch_stats()
                        .done_ticket_ids(&range(
                            arc,
                            "2026-10-01T00:00:00.000Z",
                            "2026-10-10T00:00:00.000Z",
                        ))
                        .await
                })
                .await
                .unwrap();
                assert_eq!(erledigt, ["ARC-0001", "ARC-0002"]);

                let nur_b2 = $crate::tx!(db, |u| {
                    u.dispatch_stats()
                        .done_ticket_ids(&DispatchRunFilter {
                            board_id: Some(b2),
                            ..range(arc, "2026-10-01T00:00:00.000Z", "2026-10-10T00:00:00.000Z")
                        })
                        .await
                })
                .await
                .unwrap();
                assert_eq!(nur_b2, ["ARC-0002"]);

                let verlauf = $crate::tx!(db, |u| {
                    u.dispatch_stats()
                        .status_history(&["ARC-0001".to_string(), "ARC-0003".to_string()])
                        .await
                })
                .await
                .unwrap();
                let gesehen: Vec<(String, Option<String>)> = verlauf
                    .iter()
                    .map(|h| (h.ticket_id.clone(), h.to.clone()))
                    .collect();
                assert_eq!(
                    gesehen,
                    vec![
                        ("ARC-0001".to_string(), Some("done".to_string())),
                        ("ARC-0003".to_string(), Some("done".to_string())),
                        ("ARC-0003".to_string(), Some("in_progress".to_string())),
                        ("ARC-0003".to_string(), Some("done".to_string())),
                    ]
                );
                let leer = $crate::tx!(db, |u| { u.dispatch_stats().status_history(&[]).await })
                    .await
                    .unwrap();
                assert_eq!(leer, vec![]);

                let gezahlt = $crate::tx!(db, |u| {
                    u.dispatch_stats()
                        .dispatch_comment_counts(&["ARC-0001".to_string(), "ARC-0002".to_string()])
                        .await
                })
                .await
                .unwrap();
                assert_eq!(
                    gezahlt,
                    BTreeMap::from([(
                        "ARC-0001".to_string(),
                        DispatchCommentCounts {
                            starts: 2,
                            aborts: 1
                        }
                    )])
                );
                let keine = $crate::tx!(db, |u| {
                    u.dispatch_stats().dispatch_comment_counts(&[]).await
                })
                .await
                .unwrap();
                assert_eq!(keine, BTreeMap::new());

                let kommentare: Vec<String> =
                    $crate::tx!(db, |u| { u.dispatch_stats().dispatch_comments(arc).await })
                        .await
                        .unwrap()
                        .into_iter()
                        .map(|c| c.body)
                        .collect();
                assert_eq!(
                    kommentare,
                    [
                        "[dispatch] Start: an junior-ARC-0001 übergeben",
                        "[dispatch] Abbruch Versuch 1/3: x.",
                        "[dispatch] Start: wieder",
                    ]
                );
                db.close().await.unwrap();
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn legt_jede_konfig_version_einmal_an_und_liefert_sie_nach_zeitraum_sortiert() {
                let (db, _guard) = setup().await;
                let alt = version(
                    "aaaa000000000001",
                    "2026-10-01T08:00:00.000Z",
                    r#"{"defaultProfile":"standard"}"#,
                );
                let neu = version(
                    "bbbb000000000002",
                    "2026-10-05T09:00:00.000Z",
                    r#"{"defaultProfile":"klein","umlaute":"äöü"}"#,
                );
                let spaet = version(
                    "cccc000000000003",
                    "2026-10-20T09:00:00.000Z",
                    r#"{"defaultProfile":"standard"}"#,
                );
                let angelegt = $crate::tx!(db, [alt, neu, spaet], |u| {
                    let a = u.dispatch_runs().record_config_version(&alt).await?;
                    let b = u.dispatch_runs().record_config_version(&neu).await?;
                    let c = u.dispatch_runs().record_config_version(&spaet).await?;
                    Ok(vec![a, b, c])
                })
                .await
                .unwrap();
                assert_eq!(angelegt, [true, true, true]);

                // Derselbe Hash ein zweites Mal: nichts geschrieben, der erste Zeitpunkt bleibt
                let nochmal = $crate::tx!(db, [alt], |u| {
                    u.dispatch_runs()
                        .record_config_version(&DispatchConfigVersion {
                            first_seen_at: "2026-10-09T00:00:00.000Z".into(),
                            dispatcher: "anderer".into(),
                            ..alt
                        })
                        .await
                })
                .await
                .unwrap();
                assert!(!nochmal);

                let im_zeitraum = $crate::tx!(db, |u| {
                    u.dispatch_runs()
                        .list_config_versions(
                            "2026-10-01T00:00:00.000Z",
                            "2026-10-10T00:00:00.000Z",
                        )
                        .await
                })
                .await
                .unwrap();
                assert_eq!(im_zeitraum, vec![alt.clone(), neu.clone()]);

                let nur_neu = $crate::tx!(db, |u| {
                    u.dispatch_runs()
                        .list_config_versions(
                            "2026-10-05T09:00:00.000Z",
                            "2026-10-20T09:00:00.000Z",
                        )
                        .await
                })
                .await
                .unwrap();
                assert_eq!(nur_neu, vec![neu.clone()]);

                let keine = $crate::tx!(db, |u| {
                    u.dispatch_runs()
                        .list_config_versions(
                            "2027-01-01T00:00:00.000Z",
                            "2027-02-01T00:00:00.000Z",
                        )
                        .await
                })
                .await
                .unwrap();
                assert_eq!(keine, vec![]);
                db.close().await.unwrap();
            }
        }
    };
}
