//! Autopilot nach dem Lauf: Human Interaction wird automatisch abgenommen, die Freigabe vor dem
//! Merge entfällt; Tickets mit Ausnahme-Label verhalten sich wie ohne Autopilot.

mod common;
mod dispatch_support;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::ticket_service::{TicketPatch, WriteOptions};
use nexus_core::dispatch::config::REVIEW_REASON;
use nexus_server::dispatch::DispatchNoticeKind;
use serde_json::{json, Value};

use dispatch_support::{DispatchFixture, OK_OUTPUT};

/// Grund, mit dem der Agent das Ticket blockiert
const HUMAN: &str = "Human Interaction: Oberfläche ansehen";

/// Einstellungen mit Autopilot und Ausnahme-Label „ohne-autopilot“
fn autopilot(review_before_merge: bool) -> Value {
    json!({
        "reviewBeforeMerge": review_before_merge,
        "autopilot": { "enabled": true, "exceptLabel": "ohne-autopilot" }
    })
}

/// Labels eines Tickets mit Ausnahme-Label
fn manual() -> TicketPatch {
    TicketPatch {
        labels: Some(vec!["auto".into(), "ohne-autopilot".into()]),
        ..TicketPatch::default()
    }
}

/// Startet das Ticket, der Agent committet und blockiert es mit `HUMAN`
async fn block_as_agent(f: &DispatchFixture, title: &str, patch: TicketPatch) -> String {
    let id = f.ready(title, patch).await;
    assert_eq!(f.tick().await.started, vec![id.clone()]);
    let worktree = f.ticket(&id).await.worktree.expect("Worktree des Tickets");
    std::fs::write(std::path::Path::new(&worktree).join("feature.txt"), "neu").expect("Datei");
    f.repo.run_git(&["-C", &worktree, "add", "feature.txt"]);
    f.repo
        .run_git(&["-C", &worktree, "commit", "-q", "-m", "feature.txt"]);
    f.backend
        .services
        .tickets
        .update(
            &id,
            &TicketPatch {
                status: Some(TicketStatus::Blocked),
                blocked_reason: Some(Some(HUMAN.into())),
                ..TicketPatch::default()
            },
            &f.agent(&id),
            &WriteOptions::default(),
        )
        .await
        .expect("als Agent blockieren");
    f.end(&id, OK_OUTPUT, Some(0));
    id
}

/// Ausgänge der ausgewerteten Läufe
fn outcomes(report: &nexus_server::dispatch::DispatchReport) -> Vec<String> {
    report
        .evaluated
        .iter()
        .map(|run| run.outcome.clone())
        .collect()
}

#[tokio::test]
async fn autopilot_nimmt_human_interaction_selbst_ab_und_mergt() {
    let f = DispatchFixture::create(autopilot(false)).await;
    let id = block_as_agent(&f, "Oberfläche", TicketPatch::default()).await;
    let report = f.tick().await;

    assert_eq!(outcomes(&report), vec!["done"]);
    assert_eq!(report.merged, vec![id.clone()]);
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Done);
    assert_eq!(ticket.blocked_reason, None);
    let comments = f.comments(&id).await;
    assert_eq!(
        comments
            .iter()
            .filter(|body| body.as_str()
                == "Autopilot: automatisch abgenommen – Human Interaction: Oberfläche ansehen")
            .count(),
        1,
        "{comments:?}"
    );
    assert!(!f
        .notices
        .all()
        .iter()
        .any(|notice| notice.kind == DispatchNoticeKind::Blocked));
}

#[tokio::test]
async fn autopilot_uberspringt_die_freigabe_vor_dem_merge() {
    let f = DispatchFixture::create(autopilot(true)).await;
    let id = f.ready("Sofort", TicketPatch::default()).await;
    f.tick().await;
    f.finish_as_agent(&id, "feature.txt", "neu").await;
    f.end(&id, OK_OUTPUT, Some(0));

    let report = f.tick().await;

    assert_eq!(outcomes(&report), vec!["done"]);
    assert_eq!(report.merged, vec![id.clone()]);
    let ticket = f.ticket(&id).await;
    assert_ne!(ticket.blocked_reason.as_deref(), Some(REVIEW_REASON));
    assert!(f
        .ctx
        .stores
        .reviews
        .read(&id)
        .expect("Review-Eintrag lesen")
        .is_none());
}

#[tokio::test]
async fn mit_ausnahme_label_bleibt_human_interaction_blockiert() {
    let f = DispatchFixture::create(autopilot(false)).await;
    let id = block_as_agent(&f, "Manuell", manual()).await;

    let report = f.tick().await;

    assert_eq!(outcomes(&report), vec!["blocked"]);
    assert!(report.merged.is_empty(), "{report:?}");
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Blocked);
    assert_eq!(ticket.blocked_reason.as_deref(), Some(HUMAN));
    assert!(f
        .notices
        .all()
        .iter()
        .any(|notice| notice.kind == DispatchNoticeKind::Blocked));
}

#[tokio::test]
async fn mit_ausnahme_label_wartet_das_ticket_auf_freigabe() {
    let f = DispatchFixture::create(autopilot(true)).await;
    let id = f.ready("Manuell", manual()).await;
    f.tick().await;
    f.finish_as_agent(&id, "feature.txt", "neu").await;
    f.end(&id, OK_OUTPUT, Some(0));

    let report = f.tick().await;

    assert_eq!(outcomes(&report), vec!["review"]);
    assert!(report.merged.is_empty(), "{report:?}");
    assert_eq!(
        f.ticket(&id).await.blocked_reason.as_deref(),
        Some(REVIEW_REASON)
    );
}
