//! Freigabe vor dem Merge; portiert aus
//! `packages/server/tests/unit/dispatch/DispatchReview.test.ts`.

mod common;
mod dispatch_support;

use std::path::Path;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::ticket_service::TicketPatch;
use nexus_core::dispatch::config::REVIEW_REASON;
use nexus_core::dispatch::markers;
use nexus_server::dispatch::merge_result::MergeRetryOutcome;
use serde_json::json;

use dispatch_support::{DispatchFixture, OK_OUTPUT};

/// Zeilen von stream-json: Werkzeugaufruf und sein Ergebnis
fn test_run(id: &str, command: &str, output: &str, is_error: bool) -> String {
    [
        json!({"type":"assistant","message":{"content":[
            {"type":"tool_use","id":id,"name":"Bash","input":{"command":command}}]}})
        .to_string(),
        json!({"type":"user","message":{"content":[
            {"type":"tool_result","tool_use_id":id,"content":output,"is_error":is_error}]}})
        .to_string(),
    ]
    .join("\n")
}

/// Ausgabe eines Laufs mit einem grünen und einem roten Testlauf
fn run_output() -> String {
    [
        test_run("t1", "npx vitest run", "Tests 3 passed", false),
        test_run("t2", "npm run lint", "1 error", true),
        OK_OUTPUT.to_string(),
    ]
    .join("\n")
}

/// Startet ein bereites Ticket, lässt den Agenten erledigen und wertet den Lauf aus
async fn to_review(f: &DispatchFixture, title: &str) -> String {
    let id = f.ready(title, TicketPatch::default()).await;
    assert_eq!(f.tick().await.started, vec![id.clone()]);
    f.finish_as_agent(&id, "feature.txt", "neu").await;
    f.end(&id, &run_output(), Some(0));
    let report = f.tick().await;
    assert_eq!(
        report
            .evaluated
            .iter()
            .map(|run| (run.ticket_id.as_str(), run.outcome.as_str()))
            .collect::<Vec<_>>(),
        vec![(id.as_str(), "review")]
    );
    assert!(report.merged.is_empty(), "{report:?}");
    id
}

/// Kommentar, der mit „[dispatch] Review:“ beginnt
async fn review_comment(f: &DispatchFixture, id: &str) -> String {
    let head = format!("{}:", markers::REVIEW);
    f.comments(id)
        .await
        .into_iter()
        .find(|body| body.starts_with(&head))
        .expect("Review-Kommentar")
}

#[tokio::test]
async fn ein_vom_lauf_erledigtes_ticket_wartet_auf_freigabe_statt_zu_mergen() {
    let f = DispatchFixture::create(json!({ "reviewBeforeMerge": true })).await;
    let id = to_review(&f, "Rabatt").await;

    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Blocked);
    assert_eq!(ticket.blocked_reason.as_deref(), Some(REVIEW_REASON));
    assert_eq!(ticket.assignee, None);
    assert_eq!(
        ticket.branch.as_deref(),
        Some(&*format!("ticket/{id}-rabatt"))
    );
    assert!(!f.repo.path().join("feature.txt").exists());
    assert!(Path::new(&ticket.worktree.clone().expect("Worktree")).exists());
    assert!(f
        .ctx
        .stores
        .state
        .read()
        .expect("Zustand")
        .merges
        .is_empty());

    let entry = f
        .ctx
        .stores
        .reviews
        .read(&id)
        .expect("Review-Eintrag lesen")
        .expect("Review-Eintrag");
    assert_eq!(
        entry
            .tests
            .iter()
            .map(|test| (test.command.as_str(), test.ok, test.output.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("npx vitest run", Some(true), "Tests 3 passed"),
            ("npm run lint", Some(false), "1 error"),
        ]
    );
    let review = review_comment(&f, &id).await;
    assert!(review.contains("1 Datei geändert (+1/−0)"), "{review}");
    assert!(review.contains("Tests: 1 grün, 1 rot"), "{review}");
    assert!(f
        .notices
        .all()
        .iter()
        .any(|notice| notice.kind == nexus_server::dispatch::DispatchNoticeKind::Review));

    // Ein weiterer Durchlauf ändert nichts
    let again = f.tick().await;
    assert!(
        again.started.is_empty() && again.merged.is_empty() && again.evaluated.is_empty(),
        "{again:?}"
    );
    assert_eq!(f.ticket(&id).await.status, TicketStatus::Blocked);
}

#[tokio::test]
async fn ohne_freigabe_mergt_derselbe_ablauf_sofort() {
    let f = DispatchFixture::create(json!({ "reviewBeforeMerge": false })).await;
    let id = f.ready("Sofort", TicketPatch::default()).await;
    f.tick().await;
    f.finish_as_agent(&id, "feature.txt", "neu").await;
    f.end(&id, &run_output(), Some(0));

    let report = f.tick().await;
    assert_eq!(
        report
            .evaluated
            .iter()
            .map(|run| run.outcome.as_str())
            .collect::<Vec<_>>(),
        vec!["done"]
    );
    assert_eq!(report.merged, vec![id.clone()]);
    assert!(f
        .ctx
        .stores
        .reviews
        .read(&id)
        .expect("Review-Eintrag lesen")
        .is_none());
}

#[tokio::test]
async fn ubernehmen_mergt_wie_bisher_und_loscht_den_review_eintrag() {
    let f = DispatchFixture::create(json!({ "reviewBeforeMerge": true })).await;
    let id = to_review(&f, "Rabatt").await;

    let accepted = f
        .service
        .accept(&id, Some("Florian"))
        .await
        .expect("Abnehmen");
    assert_eq!(accepted.result.outcome, MergeRetryOutcome::Merged);
    let detail = f.detail(&id).await;
    assert_eq!(detail.ticket.status, TicketStatus::Done);
    assert_eq!(detail.ticket.blocked_reason, None);
    assert_eq!(detail.ticket.branch, None);
    assert_eq!(detail.ticket.worktree, None);
    assert!(f.repo.path().join("feature.txt").exists());
    assert!(
        detail.history.iter().any(|entry| entry.field == "status"
            && entry.to.as_deref() == Some("done")
            && entry.note.as_deref() == Some("abgenommen")
            && entry.by_name == "Florian"),
        "{:?}",
        detail.history
    );
    assert!(f
        .ctx
        .stores
        .reviews
        .read(&id)
        .expect("Review-Eintrag lesen")
        .is_none());
}

#[tokio::test]
async fn ubernehmen_mit_squash_ergibt_einen_commit() {
    let f = DispatchFixture::create(json!({
        "reviewBeforeMerge": true,
        "mergeStrategy": "squash",
    }))
    .await;
    let before: i64 = f
        .repo
        .git_out(&["rev-list", "--count", "HEAD"])
        .parse()
        .expect("Zahl der Commits");
    let id = to_review(&f, "Rabatt").await;

    let accepted = f
        .service
        .accept(&id, Some("Florian"))
        .await
        .expect("Abnehmen");
    assert_eq!(accepted.result.outcome, MergeRetryOutcome::Merged);
    let after: i64 = f
        .repo
        .git_out(&["rev-list", "--count", "HEAD"])
        .parse()
        .expect("Zahl der Commits");
    assert_eq!(after, before + 1);
}

#[tokio::test]
async fn zuruck_an_den_agenten_setzt_im_selben_worktree_fort_und_zahlt_nicht_als_versuch() {
    let f = DispatchFixture::create(json!({ "reviewBeforeMerge": true })).await;
    let id = to_review(&f, "Rabatt").await;
    let ticket = f.ticket(&id).await;
    let (branch, worktree) = (ticket.branch.clone(), ticket.worktree.clone());

    let returned = f
        .service
        .return_to_agent(&id, "Bitte auch den Fehlerfall testen", Some("Florian"))
        .await
        .expect("Zurück an den Agenten");
    assert_eq!(returned.result.outcome, MergeRetryOutcome::Returned);
    assert!(returned.result.text.contains(&id), "{:?}", returned.result);
    let ticket = f.ticket(&id).await;
    assert_eq!(ticket.status, TicketStatus::Planned);
    assert_eq!(ticket.blocked_reason, None);
    assert_eq!(ticket.assignee, None);
    assert_eq!(ticket.branch, branch);
    assert_eq!(ticket.worktree, worktree);
    assert_eq!(
        f.comments(&id).await.last().expect("Kommentar"),
        &format!(
            "{} (von Florian): Bitte auch den Fehlerfall testen",
            markers::REVIEW_RETURN
        )
    );

    assert_eq!(f.tick().await.started, vec![id.clone()]);
    let record = f.run(&id);
    assert_eq!(Some(record.cwd.clone()), worktree);
    assert!(
        record.prompt.contains("Rückmeldung aus dem Review"),
        "{}",
        record.prompt
    );
    assert!(
        record.prompt.contains("Bitte auch den Fehlerfall testen"),
        "{}",
        record.prompt
    );
    assert_eq!(record.run.profile, "standard");
    assert!(!f
        .comments(&id)
        .await
        .iter()
        .any(|body| body.starts_with(markers::ABORT)));

    // Zweiter Lauf: wieder im Review, neue Tests
    f.finish_as_agent(&id, "fehlerfall.txt", "neu").await;
    f.end(
        &id,
        &[
            test_run("t9", "npm test", "alles grün", false),
            OK_OUTPUT.to_string(),
        ]
        .join("\n"),
        Some(0),
    );
    assert_eq!(
        f.tick()
            .await
            .evaluated
            .iter()
            .map(|run| run.outcome.as_str())
            .collect::<Vec<_>>(),
        vec!["review"]
    );
    let entry = f
        .ctx
        .stores
        .reviews
        .read(&id)
        .expect("Review-Eintrag lesen")
        .expect("Review-Eintrag");
    assert_eq!(
        entry
            .tests
            .iter()
            .map(|test| (test.command.as_str(), test.ok))
            .collect::<Vec<_>>(),
        vec![("npm test", Some(true))]
    );

    // Im nächsten Start ist die alte Rückmeldung erledigt
    f.service
        .return_to_agent(&id, "Noch der Name", Some("Florian"))
        .await
        .expect("Zurück an den Agenten");
    f.tick().await;
    let third = f.run(&id);
    assert!(third.prompt.contains("Noch der Name"), "{}", third.prompt);
    assert!(
        !third.prompt.contains("Bitte auch den Fehlerfall testen"),
        "{}",
        third.prompt
    );
}

#[tokio::test]
async fn zuruck_an_den_agenten_lehnt_leeren_text_tickets_ohne_review_und_laufende_tickets_ab() {
    let f = DispatchFixture::create(json!({ "reviewBeforeMerge": true })).await;
    let id = to_review(&f, "Rabatt").await;
    let refused = |error: nexus_core::Error| assert_eq!(error.code(), Some("return-not-possible"));

    refused(
        f.service
            .return_to_agent(&id, "   ", Some("Florian"))
            .await
            .expect_err("leerer Text"),
    );
    let plain = f.ready("Normal", TicketPatch::default()).await;
    refused(
        f.service
            .return_to_agent(&plain, "x", Some("Florian"))
            .await
            .expect_err("kein Review"),
    );
    f.tick().await;
    refused(
        f.service
            .return_to_agent(&plain, "x", Some("Florian"))
            .await
            .expect_err("Lauf aktiv"),
    );
}

#[tokio::test]
async fn review_ansicht_liefert_dateien_tests_und_patch_und_lehnt_unbekannte_pfade_ab() {
    let f = DispatchFixture::create(json!({ "reviewBeforeMerge": true })).await;
    let id = to_review(&f, "Rabatt").await;

    let view = f.service.review(&id).await.expect("Review-Ansicht");
    assert_eq!(view.ticket_id, id);
    assert_eq!(
        view.branch.as_deref(),
        Some(&*format!("ticket/{id}-rabatt"))
    );
    assert!(view.available);
    assert_eq!(view.reason, None);
    assert_eq!(view.files.len(), 1);
    assert_eq!(view.files[0].path, "feature.txt");
    assert_eq!(view.files[0].status, "A");
    assert_eq!(view.files[0].additions, 1);
    assert_eq!(
        view.tests.iter().map(|test| test.ok).collect::<Vec<_>>(),
        vec![Some(true), Some(false)]
    );
    assert!(
        view.run_log
            .as_deref()
            .is_some_and(|log| log.ends_with(".log")),
        "{:?}",
        view.run_log
    );

    let patch = f
        .service
        .review_file(&id, "feature.txt")
        .await
        .expect("Patch");
    assert!(patch.patch.contains("+neu"), "{}", patch.patch);
    let error = f
        .service
        .review_file(&id, "--output=x")
        .await
        .expect_err("unbekannter Pfad");
    assert_eq!(error.code(), Some("review-file-unknown"));
}

#[tokio::test]
async fn review_ansicht_mit_zielbranch_zeigt_den_diff_gegen_den_zielbranch() {
    let f = DispatchFixture::create(json!({ "reviewBeforeMerge": true })).await;
    // release/1 hat einen Stand mehr als der ausgecheckte Branch; das Ticket zweigt beim Start
    // von ihm ab
    let main = f.repo.git_out(&["rev-parse", "--abbrev-ref", "HEAD"]);
    f.repo.run_git(&["checkout", "-q", "-b", "release/1"]);
    std::fs::write(f.repo.path().join("release.txt"), "release").expect("Datei");
    f.repo.run_git(&["add", "release.txt"]);
    f.repo
        .run_git(&["commit", "-q", "-m", "Stand von release/1"]);
    f.repo.run_git(&["checkout", "-q", &main]);

    let id = f
        .ready(
            "Ziel",
            TicketPatch {
                target_branch: Some(Some("release/1".into())),
                ..TicketPatch::default()
            },
        )
        .await;
    assert_eq!(f.tick().await.started, vec![id.clone()]);
    f.finish_as_agent(&id, "feature.txt", "neu").await;
    f.end(&id, &run_output(), Some(0));
    assert_eq!(
        f.tick()
            .await
            .evaluated
            .iter()
            .map(|run| run.outcome.as_str())
            .collect::<Vec<_>>(),
        vec!["review"]
    );

    // Kommentar beim Gate: nur die Änderung des Tickets (feature.txt), nicht release.txt
    let comment = review_comment(&f, &id).await;
    assert!(comment.contains("1 Datei geändert (+1/−0)"), "{comment}");
    // Panel: Ziel ist der Zielbranch, Dateiliste ohne release.txt
    let view = f.service.review(&id).await.expect("Review-Ansicht");
    assert!(view.available);
    assert_eq!(view.target.as_deref(), Some("release/1"));
    assert_eq!(view.files.len(), 1);
    assert_eq!(view.files[0].path, "feature.txt");
    assert_eq!(view.files[0].status, "A");
    assert_eq!(view.base, Some(f.repo.git_out(&["rev-parse", "release/1"])));
    assert!(f
        .service
        .review_file(&id, "feature.txt")
        .await
        .expect("Patch")
        .patch
        .contains("+neu"));
    let error = f
        .service
        .review_file(&id, "release.txt")
        .await
        .expect_err("fremde Datei");
    assert_eq!(error.code(), Some("review-file-unknown"));
}
