//! Portiert aus `packages/core/tests/unit/board/TimelineClassifier.test.ts`.

use super::*;
use crate::board::ticket_timeline::TimelineActorType;

const AT: &str = "2026-10-13T11:00:00.000Z";

/// Die Namen der Dispatch-Konfiguration aus TypeScript
fn names() -> TimelineAgentNames {
    TimelineAgentNames {
        dispatcher_name: "dispatcher".into(),
        agent_prefix: "junior-".into(),
    }
}

/// Ein Verlaufseintrag zu einem Feld
fn entry(field: &str, from: Option<&str>, to: Option<&str>) -> HistoryEntry {
    entry_by(field, from, to, "Florian", None)
}

/// Ein Verlaufseintrag mit Autor und Vermerk
fn entry_by(
    field: &str,
    from: Option<&str>,
    to: Option<&str>,
    by_name: &str,
    note: Option<&str>,
) -> HistoryEntry {
    HistoryEntry {
        ticket_id: "ARC-0001".into(),
        at: AT.into(),
        by_id: "u".into(),
        by_name: by_name.to_string(),
        field: field.to_string(),
        from: from.map(ToString::to_string),
        to: to.map(ToString::to_string),
        note: note.map(ToString::to_string),
    }
}

/// Ein Kommentar des Dispatchers
fn comment(body: &str) -> Comment {
    comment_by(body, "dispatcher")
}

/// Ein Kommentar mit Autor
fn comment_by(body: &str, author_name: &str) -> Comment {
    Comment {
        id: 1,
        ticket_id: "ARC-0001".into(),
        author_id: "u".into(),
        author_name: author_name.to_string(),
        body: body.to_string(),
        created_at: AT.into(),
        read_by_agent_at: None,
    }
}

#[test]
fn ordnet_jedes_feld_des_verlaufs_seiner_art_zu() {
    let c = TimelineClassifier::new(Some(names()));
    let erwartet = [
        ("status", TimelineKind::Status),
        ("blockedReason", TimelineKind::Status),
        ("assignee", TimelineKind::Claim),
        ("planningClaimBy", TimelineKind::Claim),
        ("question", TimelineKind::Question),
        ("branch", TimelineKind::Git),
        ("worktree", TimelineKind::Git),
        ("plan", TimelineKind::Content),
        ("handbook", TimelineKind::Content),
        ("acceptance", TimelineKind::Content),
        ("step", TimelineKind::Content),
        ("steps", TimelineKind::Content),
        ("description", TimelineKind::Content),
        ("priority", TimelineKind::Other),
        ("labels", TimelineKind::Other),
        ("dependsOn", TimelineKind::Other),
    ];
    for (field, kind) in erwartet {
        let event = c.history(&entry(field, Some("a"), Some("b")));
        assert_eq!(event.kind, kind, "{field}");
        assert_eq!(event.event_type, format!("field:{field}"));
        assert_eq!(event.field.as_deref(), Some(field));
        assert_eq!(event.from, Some(Some("a".into())));
        assert_eq!(event.to, Some(Some("b".into())));
    }
}

#[test]
fn erkennt_agenten_an_dispatcher_name_vorsilbe_und_claude_sonst_person() {
    let c = TimelineClassifier::new(Some(names()));
    assert_eq!(
        c.actor("dispatcher", None).actor_type,
        TimelineActorType::Agent
    );
    assert_eq!(
        c.actor("junior-ARC-0001", None).actor_type,
        TimelineActorType::Agent
    );
    assert_eq!(c.actor("claude", None).actor_type, TimelineActorType::Agent);
    assert_eq!(
        c.actor("Florian", None).actor_type,
        TimelineActorType::Person
    );
    assert_eq!(c.actor("dispatcher", None).name, "dispatcher");
    // Ohne Dispatch-Konfiguration zählt die Vorsilbe nicht
    assert_eq!(
        TimelineClassifier::new(None)
            .actor("junior-ARC-0001", None)
            .actor_type,
        TimelineActorType::Person
    );
}

#[test]
fn nimmt_bei_fragen_die_art_des_autors() {
    let c = TimelineClassifier::new(Some(names()));
    assert_eq!(
        c.actor("Florian", Some(AuthorKind::Session)).actor_type,
        TimelineActorType::Agent
    );
    assert_eq!(
        c.actor("Florian", Some(AuthorKind::Subagent)).actor_type,
        TimelineActorType::Agent
    );
    assert_eq!(
        c.actor("junior-x", Some(AuthorKind::Human)).actor_type,
        TimelineActorType::Person
    );
}

#[test]
fn liest_das_profil_aus_dem_start_kommentar() {
    let c = TimelineClassifier::new(Some(names()));
    let body = format!(
        "{}: an junior-ARC-0001 übergeben – Branch ticket/ARC-0001-x, Profil standard (Modell sonnet, Effort high, max. 80 Schritte, 60 min). Grund: Regel.",
        markers::START
    );
    let event = c.comment(&comment(&body));
    assert_eq!(event.kind, TimelineKind::Run);
    assert_eq!(event.event_type, "dispatch:start");
    assert_eq!(
        event.reference.and_then(|r| r.profile).as_deref(),
        Some("standard")
    );
    assert_eq!(
        c.comment(&comment(&body)).actor.actor_type,
        TimelineActorType::Agent
    );
}

#[test]
fn liest_den_commit_aus_dem_merge_kommentar() {
    let c = TimelineClassifier::new(Some(names()));
    let event = c.comment(&comment(&format!("{} in main (abc1234).", markers::MERGED)));
    assert_eq!(event.kind, TimelineKind::Git);
    assert_eq!(event.event_type, "merge");
    assert_eq!(
        event.reference.and_then(|r| r.commit).as_deref(),
        Some("abc1234")
    );
}

#[test]
fn ordnet_die_ubrigen_dispatcher_kommentare_zu() {
    let c = TimelineClassifier::new(Some(names()));
    let fälle = [
        (
            format!("{} Versuch 1/3: Zeitlimit.", markers::ABORT),
            TimelineKind::Run,
            "dispatch:abort",
        ),
        (
            format!("{}: Nutzungslimit", markers::PAUSE),
            TimelineKind::Run,
            "dispatch:pause",
        ),
        (
            format!("{}: wartet auf ARC-F0001", markers::QUESTION),
            TimelineKind::Run,
            "dispatch:question",
        ),
        (
            format!("{} fehlgeschlagen: npm ci", markers::SETUP),
            TimelineKind::Run,
            "dispatch:setup",
        ),
        (
            format!("{}: wartet auf Freigabe", markers::REVIEW),
            TimelineKind::Run,
            "dispatch:review",
        ),
        (
            format!("{} (von Florian): bitte Tests", markers::REVIEW_RETURN),
            TimelineKind::Run,
            "dispatch:review-return",
        ),
        (
            format!(
                "{} in a.ts beim Merge in main – Merge-Session gestartet (Profil schwer).",
                markers::MERGE_CONFLICT
            ),
            TimelineKind::Git,
            "dispatch:merge-conflict",
        ),
        (
            format!("{} auf ARC-0002", markers::MERGE_WAIT),
            TimelineKind::Git,
            "dispatch:merge-wait",
        ),
        (
            format!("{} nicht möglich: lokale Änderungen", markers::MERGE),
            TimelineKind::Git,
            "dispatch:merge",
        ),
    ];
    for (body, kind, event_type) in fälle {
        let event = c.comment(&comment(&body));
        assert_eq!(event.kind, kind, "{body}");
        assert_eq!(event.event_type, event_type, "{body}");
        assert_eq!(event.text.as_deref(), Some(body.as_str()));
    }
}

#[test]
fn lasst_normale_kommentare_als_kommentar_auch_vom_dispatcher() {
    let c = TimelineClassifier::new(Some(names()));
    let von_mensch = c.comment(&comment_by("Bitte prüfen", "Florian"));
    assert_eq!(von_mensch.kind, TimelineKind::Comment);
    assert_eq!(von_mensch.event_type, "comment");
    assert_eq!(von_mensch.actor.actor_type, TimelineActorType::Person);
    let ohne_marker = c.comment(&comment("Hinweis ohne Marker"));
    assert_eq!(ohne_marker.kind, TimelineKind::Comment);
    assert_eq!(ohne_marker.event_type, "comment");
}

#[test]
fn liest_die_fragen_id_aus_dem_vermerk_eines_frage_eintrags() {
    let c = TimelineClassifier::new(Some(names()));
    let event = c.history(&entry_by(
        "question",
        Some("open"),
        Some("answered"),
        "Florian",
        Some("Entschieden: ARC-F0002 „Speicher?“"),
    ));
    assert_eq!(
        event.reference.and_then(|r| r.question_id).as_deref(),
        Some("ARC-F0002")
    );
}
