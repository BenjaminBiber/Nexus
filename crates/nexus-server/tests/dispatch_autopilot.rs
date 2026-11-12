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

/// Externer Grund (keine Human Interaction), mit dem der Agent das Ticket blockiert
const EXTERN: &str = "Zugang zur Testumgebung fehlt";

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
    block_as_agent_with(f, title, patch, HUMAN).await
}

/// Startet das Ticket, der Agent committet und blockiert es mit diesem Grund
async fn block_as_agent_with(
    f: &DispatchFixture,
    title: &str,
    patch: TicketPatch,
    reason: &str,
) -> String {
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
                blocked_reason: Some(Some(reason.into())),
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
async fn blockiert_der_agent_mit_externem_grund_meldet_der_autopilot_stuck() {
    let f = DispatchFixture::create(autopilot(false)).await;
    let id = block_as_agent_with(&f, "Ohne Zugang", TicketPatch::default(), EXTERN).await;

    let report = f.tick().await;

    assert_eq!(outcomes(&report), vec!["blocked"]);
    assert_eq!(f.ticket(&id).await.status, TicketStatus::Blocked);
    let notices = f.notices.all();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert_eq!(notices[0].kind, DispatchNoticeKind::Stuck);
    assert_eq!(notices[0].ticket_id.as_deref(), Some(id.as_str()));
    assert_eq!(
        notices[0].text,
        format!("Autopilot steckt fest: {id} – {EXTERN}")
    );
}

#[tokio::test]
async fn blockiert_der_agent_mit_ausnahme_label_bleibt_es_bei_blocked() {
    let f = DispatchFixture::create(autopilot(false)).await;
    block_as_agent_with(&f, "Manuell ohne Zugang", manual(), EXTERN).await;

    f.tick().await;

    let notices = f.notices.all();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert_eq!(notices[0].kind, DispatchNoticeKind::Blocked);
    assert_eq!(notices[0].text, EXTERN);
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

/// Vorsilbe des Kommentars, mit dem der Autopilot einen Merge nachholt
const CATCH_UP: &str = "[dispatch] Merge: Autopilot holt den Merge nach";

/// Der Agent blockiert mit Human Interaction, ein Mensch nimmt das Ticket ohne Merge-Auftrag ab
/// (wie vor dem Autopilot); das Ticket steht auf Erledigt und trägt noch seinen Branch
async fn accepted_by_human(f: &DispatchFixture, title: &str, patch: TicketPatch) -> String {
    let id = block_as_agent(f, title, patch).await;
    f.tick().await;
    f.backend
        .services
        .tickets
        .accept(&id, &f.actor(), &WriteOptions::default())
        .await
        .expect("von Hand abnehmen");
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Done);
    assert!(ticket.branch.is_some());
    id
}

/// Zahl der Nachhol-Kommentare am Ticket
async fn catch_up_comments(f: &DispatchFixture, id: &str) -> usize {
    f.comments(id)
        .await
        .iter()
        .filter(|body| body.starts_with(CATCH_UP))
        .count()
}

#[tokio::test]
async fn autopilot_holt_den_merge_eines_abgenommenen_dispatcher_tickets_nach() {
    let f = DispatchFixture::create(json!({ "reviewBeforeMerge": false })).await;
    let id = accepted_by_human(&f, "Abgenommen", TicketPatch::default()).await;
    f.write_config(autopilot(false));

    let report = f.tick().await;

    assert_eq!(report.merged, vec![id.clone()]);
    let ticket = f.ticket(&id).await;
    assert!(ticket.branch.unwrap_or_default().is_empty());
    assert_eq!(catch_up_comments(&f, &id).await, 1);
}

#[tokio::test]
async fn ohne_autopilot_bleibt_das_erledigte_ticket_liegen() {
    let f = DispatchFixture::create(json!({ "reviewBeforeMerge": false })).await;
    let id = accepted_by_human(&f, "Liegt", TicketPatch::default()).await;

    let report = f.tick().await;

    assert!(report.merged.is_empty());
    assert!(f.ticket(&id).await.branch.is_some());
    assert_eq!(catch_up_comments(&f, &id).await, 0);
}

#[tokio::test]
async fn mit_ausnahme_label_wird_nichts_nachgeholt() {
    let f = DispatchFixture::create(json!({ "reviewBeforeMerge": false })).await;
    let id = accepted_by_human(&f, "Manuell", manual()).await;
    f.write_config(autopilot(false));

    let report = f.tick().await;

    assert!(report.merged.is_empty());
    assert!(f.ticket(&id).await.branch.is_some());
    assert_eq!(catch_up_comments(&f, &id).await, 0);
}

#[tokio::test]
async fn von_hand_abgeschlossene_tickets_holt_der_autopilot_nicht_nach() {
    let f = DispatchFixture::create(json!({ "reviewBeforeMerge": false })).await;
    let id = f.ready("Von Hand", TicketPatch::default()).await;
    let repo = f.repo.path().to_string_lossy().to_string();
    f.repo
        .run_git(&["-C", &repo, "branch", "ticket/von-hand", "HEAD"]);
    f.backend
        .services
        .tickets
        .update(
            &id,
            &TicketPatch {
                status: Some(TicketStatus::Done),
                branch: Some(Some("ticket/von-hand".into())),
                assignee: Some(Some("claude".into())),
                acceptance: Some("von Hand geprüft".into()),
                ..TicketPatch::default()
            },
            &f.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("von Hand abschließen");
    f.write_config(autopilot(false));

    let report = f.tick().await;

    assert!(report.merged.is_empty());
    assert!(f.ticket(&id).await.branch.is_some());
    assert_eq!(catch_up_comments(&f, &id).await, 0);
}

#[tokio::test]
async fn branch_den_es_im_repo_nicht_gibt_bleibt_still() {
    let f = DispatchFixture::create(json!({ "reviewBeforeMerge": false })).await;
    let id = accepted_by_human(&f, "Fremder Branch", TicketPatch::default()).await;
    f.backend
        .services
        .tickets
        .update(
            &id,
            &TicketPatch {
                branch: Some(Some("ticket/gibt-es-nicht".into())),
                ..TicketPatch::default()
            },
            &f.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Branch umsetzen");
    f.write_config(autopilot(false));

    let first = f.tick().await;
    let second = f.tick().await;

    assert!(first.merged.is_empty() && second.merged.is_empty());
    assert_eq!(catch_up_comments(&f, &id).await, 0);
    assert_eq!(f.ticket(&id).await.status, TicketStatus::Done);
    assert!(f
        .ctx
        .stores
        .state
        .read()
        .expect("Zustand")
        .merges
        .is_empty());
}

#[tokio::test]
async fn wartender_merge_wird_nicht_doppelt_vorgemerkt() {
    let f = DispatchFixture::create(json!({ "reviewBeforeMerge": false })).await;
    let id = accepted_by_human(&f, "Wartet", TicketPatch::default()).await;
    let repo = f.repo.path().to_string_lossy().to_string();
    std::fs::write(f.repo.path().join("vorgemerkt.txt"), "x").expect("Datei");
    f.repo.run_git(&["-C", &repo, "add", "vorgemerkt.txt"]);
    f.write_config(autopilot(false));

    let first = f.tick().await;
    let second = f.tick().await;

    assert!(first.merged.is_empty() && second.merged.is_empty());
    assert_eq!(catch_up_comments(&f, &id).await, 1);
    assert!(f
        .ctx
        .stores
        .state
        .read()
        .expect("Zustand")
        .merges
        .contains_key(&id));
    assert_eq!(f.ticket(&id).await.status, TicketStatus::Done);
}
