//! Portiert aus `packages/core/tests/unit/board/DispatchRunService.test.ts`.

use super::*;
use crate::board::basics::TicketStatus;
use crate::board::dispatch_run_types::DispatchRunOutcome;
use crate::board::ticket_service::{ClaimOptions, MoveOptions, NewTicketInput, WriteOptions};
use crate::testing::{run_fixture, BoardFixture};

/// Ein Zeitpunkt in UTC
fn utc(text: &str) -> DateTime<Utc> {
    text.parse().expect("der Zeitpunkt ist gültig")
}

/// Eine Fassung der Dispatcher-Einstellung
fn version(hash: &str, first_seen_at: &str) -> DispatchConfigVersion {
    DispatchConfigVersion {
        hash: hash.to_string(),
        dispatcher: "d".into(),
        first_seen_at: first_seen_at.to_string(),
        content: "{}".into(),
    }
}

#[tokio::test]
async fn erfasst_laufe_einmal_und_wertet_statuswechsel_kommentare_und_laufe_eines_zeitraums_aus() {
    let f = BoardFixture::create().await;
    let service = DispatchRunService::new(f.db.clone());
    let project_id = f.project_id().await;
    for titel in ["A", "B"] {
        f.tickets
            .create(
                f.board_id,
                &NewTicketInput {
                    status: Some(TicketStatus::Planned),
                    labels: (titel == "A").then(|| vec!["ui".into()]),
                    ..NewTicketInput::titled(titel)
                },
                &f.actor,
                &WriteOptions::default(),
            )
            .await
            .unwrap();
    }
    f.advance(60);
    f.tickets
        .claim(
            "ARC-0001",
            "junior-ARC-0001",
            &f.actor,
            &ClaimOptions::default(),
        )
        .await
        .unwrap();
    f.comments
        .add(
            "ARC-0001",
            "[dispatch] Start: an junior-ARC-0001 übergeben",
            &f.actor,
            false,
        )
        .await
        .unwrap();
    f.advance(120);
    f.tickets
        .move_to(
            "ARC-0001",
            TicketStatus::Done,
            &f.actor,
            &MoveOptions {
                acceptance: Some("ok".into()),
                ..MoveOptions::default()
            },
        )
        .await
        .unwrap();

    let run = DispatchRun {
        board_id: Some(f.board_id),
        started_at: "2026-10-13T11:00:00.000Z".into(),
        cost_usd: Some(1.5),
        ..run_fixture("ARC-0001-work", "ARC-0001", project_id)
    };
    assert!(service.record(&run).await.unwrap());
    assert!(!service.record(&run).await.unwrap());

    let filter = DispatchStatsFilter {
        project_id,
        board_id: None,
        from: utc("2026-10-12T00:00:00.000Z"),
        to: utc("2026-10-14T00:00:00.000Z"),
        kind: None,
    };
    let stats = service.stats(&filter).await.unwrap();
    assert_eq!(stats.kpis.runs, 1);
    assert_eq!(stats.kpis.subjects, 1);
    assert_eq!(stats.kpis.done, 1);
    assert_eq!(stats.kpis.success_rate, Some(1.0));
    assert_eq!(stats.kpis.first_try_rate, Some(1.0));
    assert_eq!(
        stats.runs_since.as_deref(),
        Some("2026-10-13T11:00:00.000Z")
    );
    assert_eq!(stats.by_profile.len(), 1);
    assert_eq!(stats.by_profile[0].profile, "standard");
    assert_eq!(stats.by_profile[0].runs, 1);
    assert_eq!(stats.by_profile[0].done, 1);
    assert_eq!(stats.config_changed_at, None);

    // Andere Lauf-Art ohne Treffer
    assert_eq!(
        service
            .stats(&DispatchStatsFilter {
                kind: Some(DispatchRunKind::Merge),
                ..filter.clone()
            })
            .await
            .unwrap()
            .kpis
            .runs,
        0
    );

    // Neueste Fassung der Einstellung vor dem Ende des Zeitraums
    for (hash, at) in [
        ("a", "2026-10-01T08:00:00.000Z"),
        ("b", "2026-10-13T08:00:00.000Z"),
        ("c", "2026-10-20T08:00:00.000Z"),
    ] {
        service
            .record_config_version(&version(hash, at))
            .await
            .unwrap();
    }
    assert_eq!(
        service.stats(&filter).await.unwrap().config_changed_at,
        Some("2026-10-13T08:00:00.000Z".into())
    );
}

#[tokio::test]
async fn tragt_laufe_aus_dispatch_kommentaren_nach_und_bleibt_dabei_wiederholbar() {
    let f = BoardFixture::create().await;
    let service = DispatchRunService::new(f.db.clone());
    let project_id = f.project_id().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput {
                status: Some(TicketStatus::Planned),
                ..NewTicketInput::titled("A")
            },
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .unwrap();
    f.comments
        .add(
            "ARC-0001",
            "[dispatch] Start: an junior-ARC-0001 übergeben – Branch ticket/ARC-0001-a, Profil standard (Modell opus, Effort high, max. 80 Schritte, 90 min). Grund: Standardprofil standard.",
            &f.actor,
            false,
        )
        .await
        .unwrap();
    f.advance(30);
    f.comments
        .add(
            "ARC-0001",
            "[dispatch] Abbruch Versuch 1/3: Zeitlimit von 90 min überschritten.",
            &f.actor,
            false,
        )
        .await
        .unwrap();

    // Der Probelauf schreibt nichts
    let probe = service.backfill(project_id, true).await.unwrap();
    assert_eq!(probe.found, 1);
    assert_eq!(probe.created.len(), 1);
    assert_eq!(probe.created[0].outcome, DispatchRunOutcome::Abort);
    assert_eq!(probe.created[0].abort_category.as_deref(), Some("timeout"));
    assert_eq!(
        service
            .backfill(project_id, true)
            .await
            .unwrap()
            .created
            .len(),
        1
    );

    // Der erste echte Lauf legt an, der zweite findet nichts Neues mehr
    assert_eq!(
        service
            .backfill(project_id, false)
            .await
            .unwrap()
            .created
            .len(),
        1
    );
    let zweiter = service.backfill(project_id, false).await.unwrap();
    assert_eq!(zweiter.found, 1);
    assert!(zweiter.created.is_empty());

    let stats = service
        .stats(&DispatchStatsFilter {
            project_id,
            board_id: None,
            from: utc("2026-10-12T00:00:00.000Z"),
            to: utc("2026-10-14T00:00:00.000Z"),
            kind: None,
        })
        .await
        .unwrap();
    assert_eq!(stats.kpis.runs, 1);
    assert_eq!(stats.kpis.aborted, 1);
    assert_eq!(stats.backfilled_runs, 1);
}
