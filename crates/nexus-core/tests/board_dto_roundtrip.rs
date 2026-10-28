//! Rundreise der Board-DTOs: JSON aus den TypeScript-Testfixtures einlesen, wieder
//! ausgeben und vergleichen. Schlägt ein Test fehl, weicht ein Rust-Struct vom JSON ab,
//! das die heutige Oberfläche und die CLI erwarten.
//!
//! Die JSON-Texte stammen aus
//! `packages/core/tests/unit/board/helpers/BoardDatabaseContract.ts` (`ticketFixture`),
//! `QuestionRepositoryContract.ts` (`questionFixture`),
//! `DispatchRunContract.ts` (`runFixture`) und `SearchContract.ts`.

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use nexus_core::board::{
    dispatch_run_types::{
        DispatchProfileReason, DispatchRun, DispatchRunKind, DispatchRunOutcome, DispatchRunSource,
        DispatchSeriesUnit,
    },
    inbox::types::{InboxItem, InboxKind, InboxList},
    question_types::{
        BoardQuestion, BoardQuestionActor, BoardQuestionComment, BoardQuestionDecisionOutcome,
        BoardQuestionDetail, BoardQuestionHistoryEntry, BoardQuestionHistoryType,
        BoardQuestionResearch, BoardQuestionView, NotifiableBoardQuestion,
    },
    search_types::{SearchField, SearchHit, SearchKind, SearchRequest, SearchResult, SearchRow},
    types::{Board, Comment, Deletion, HistoryEntry, Note, Project, Ticket, TicketStep},
    NoteState, Priority, TicketStatus,
};
use nexus_core::questions::QuestionState;
use nexus_core::AuthorKind;

/// Liest das JSON als `T`, schreibt es wieder und vergleicht beide Fassungen
fn roundtrip<T: DeserializeOwned + Serialize>(json: &str) {
    let erwartet: Value = serde_json::from_str(json).expect("Fixture ist kein gültiges JSON");
    let typisiert: T = serde_json::from_value(erwartet.clone()).expect("JSON passt nicht zum Typ");
    assert_eq!(
        serde_json::to_value(&typisiert).expect("Serialisieren schlug fehl"),
        erwartet
    );
}

/// Vergleicht die JSON-Schreibweise aller Varianten mit den Literalen aus TypeScript
fn varianten<T: Serialize>(werte: &[T], literale: &[&str]) {
    let geschrieben: Vec<Value> = werte
        .iter()
        .map(|wert| serde_json::to_value(wert).unwrap())
        .collect();
    let erwartet: Vec<Value> = literale.iter().map(|text| Value::from(*text)).collect();
    assert_eq!(geschrieben, erwartet);
}

// `ticketFixture({ id: 'ARC-0003', projectId: 1, boardId: 1, number: 3, labels: […], dependsOn: […] })`
const TICKET: &str = r#"{
  "id": "ARC-0003", "projectId": 1, "boardId": 1, "number": 3, "title": "Umlaute äöü",
  "description": "", "status": "backlog", "priority": "normal", "labels": ["ui", "api"],
  "dependsOn": ["ARC-0002", "ARC-0001"], "sortOrder": 0, "assignee": null, "needsPlanning": true,
  "acceptance": "", "plan": "", "handbook": "", "diagram": "",
  "spec": null, "specTask": null, "branch": null, "worktree": null, "targetBranch": null,
  "touches": [], "consumes": [], "produces": [],
  "blockedReason": null, "plannedAt": null, "plannedBy": null,
  "planningClaimBy": null, "planningClaimAt": null,
  "createdAt": "2026-10-13T10:00:00.000Z", "createdById": "11111111-1111-1111-1111-111111111111",
  "createdByName": "Florian", "updatedAt": "2026-10-13T10:00:00.000Z", "rev": 1
}"#;

// Dasselbe Ticket, aber mit allen Zusatzangaben gefüllt
// (mehr Rauten am Rand, weil im Markdown `"##` vorkommt)
const TICKET_VOLL: &str = r###"{
  "id": "ARC-0004", "projectId": 1, "boardId": 2, "number": 4, "title": "Geplant",
  "description": "# Text", "status": "in_progress", "priority": "urgent", "labels": ["profil:standard"],
  "dependsOn": [], "sortOrder": 10, "assignee": "junior-ARC-0004", "needsPlanning": false,
  "acceptance": "Tests grün", "plan": "## Ziel", "handbook": "## Kontext", "diagram": "flowchart LR",
  "spec": "001-login", "specTask": "T012", "branch": "ticket/ARC-0004", "worktree": "/tmp/wt",
  "targetBranch": "nexus/tauri", "touches": ["src"], "consumes": ["api"], "produces": ["dto"],
  "blockedReason": "Human Interaction: ansehen", "plannedAt": "2026-10-13T09:00:00.000Z",
  "plannedBy": "claude", "planningClaimBy": "claude", "planningClaimAt": "2026-10-13T08:00:00.000Z",
  "createdAt": "2026-10-13T10:00:00.000Z", "createdById": "11111111-1111-1111-1111-111111111111",
  "createdByName": "Florian", "updatedAt": "2026-10-13T11:00:00.000Z", "rev": 7
}"###;

#[test]
fn ticket_fahrt_rund() {
    roundtrip::<Ticket>(TICKET);
    roundtrip::<Ticket>(TICKET_VOLL);
}

#[test]
fn projekt_board_und_benutzer_fahren_rund() {
    roundtrip::<Project>(
        r#"{"id": 1, "key": "ARC", "name": "Arches", "nextTicketNumber": 3,
            "nextQuestionNumber": 1, "createdAt": "2026-10-13T10:00:00.000Z",
            "createdBy": "11111111-1111-1111-1111-111111111111"}"#,
    );
    roundtrip::<Board>(
        r#"{"id": 1, "projectId": 1, "ownerId": "11111111-1111-1111-1111-111111111111",
            "name": "Sprint", "repoPath": null, "createdAt": "2026-10-13T10:00:00.000Z"}"#,
    );
    roundtrip::<Board>(
        r#"{"id": 2, "projectId": 1, "ownerId": "11111111-1111-1111-1111-111111111111",
            "name": "Anderes", "repoPath": "/repo/arches", "createdAt": "2026-10-13T10:00:00.000Z"}"#,
    );
}

#[test]
fn kommentar_verlauf_notiz_und_loschung_fahren_rund() {
    // Comments.Id ist bigint; die Zahl liegt über dem Bereich von int
    roundtrip::<Comment>(
        r#"{"id": 9007199254740991, "ticketId": "ARC-0001",
            "authorId": "11111111-1111-1111-1111-111111111111", "authorName": "Florian",
            "body": "Text äöü", "createdAt": "2026-10-13T10:00:00.000Z", "readByAgentAt": null}"#,
    );
    roundtrip::<HistoryEntry>(
        r#"{"ticketId": "ARC-0001", "at": "2026-10-13T10:00:00.000Z",
            "byId": "11111111-1111-1111-1111-111111111111", "byName": "Florian",
            "field": "status", "from": "backlog", "to": "in_progress", "note": null}"#,
    );
    roundtrip::<Note>(
        r#"{"id": 7, "boardId": 1, "authorId": "11111111-1111-1111-1111-111111111111",
            "authorName": "Florian", "title": "Suche findet keine Umlaute", "body": "Details",
            "source": "ARC-0001", "labels": ["ui"], "seenCount": 2, "seenIn": ["ARC-0002"],
            "state": "taken", "ticketId": "ARC-0009", "reason": "übernommen",
            "createdAt": "2026-10-13T10:00:00.000Z"}"#,
    );
    roundtrip::<Deletion>(
        r#"{"at": "2026-10-13T10:00:00.000Z", "byId": "11111111-1111-1111-1111-111111111111",
            "byName": "Florian", "ticketId": "ARC-0001", "ticketTitle": "Titel",
            "fromStatus": "backlog"}"#,
    );
    roundtrip::<Deletion>(
        r#"{"at": "2026-10-13T10:00:00.000Z", "byId": "11111111-1111-1111-1111-111111111111",
            "byName": "Florian", "ticketId": "ARC-0001", "ticketTitle": "Titel",
            "fromStatus": null}"#,
    );
}

#[test]
fn schritt_lasst_die_anleitung_weg_wenn_sie_fehlt() {
    // `detail?: string` in TypeScript: fehlt das Feld, darf es auch nicht geschrieben werden
    roundtrip::<TicketStep>(r#"{"text": "ts-rs einbinden", "done": true}"#);
    roundtrip::<TicketStep>(
        r#"{"text": "ts-rs einbinden", "done": false, "detail": "cargo add ts-rs"}"#,
    );
    let ohne: TicketStep = serde_json::from_str(r#"{"text": "x", "done": false}"#).unwrap();
    assert_eq!(
        serde_json::to_string(&ohne).unwrap(),
        r#"{"text":"x","done":false}"#
    );
}

// `questionFixture({ id: 'ARC-F0001', projectId: 1, number: 1, … })` samt Entscheidung
const FRAGE: &str = r##"{
  "id": "ARC-F0001", "projectId": 1, "number": 1, "title": "Umlaute äöü", "description": "# Text",
  "state": "answered", "occasion": "Grill", "originKind": "session", "originName": "claude",
  "originSession": "s-1", "tags": ["speicher", "ui"], "ticketIds": ["ARC-0001", "ARC-0002"],
  "dependsOn": ["ARC-F0002", "ARC-0001"],
  "decision": {"outcome": "answered", "option": "O1", "text": null, "reason": "passt",
               "byId": "11111111-1111-1111-1111-111111111111", "byName": "Florian",
               "at": "2026-10-14T11:00:00.000Z"},
  "createdAt": "2026-10-14T10:00:00.000Z", "createdById": "11111111-1111-1111-1111-111111111111",
  "updatedAt": "2026-10-14T11:00:00.000Z", "rev": 2
}"##;

// Dieselbe Frage als Anzeigefassung: die Felder der Frage stehen flach daneben
const FRAGE_ANSICHT: &str = r#"{
  "id": "ARC-F0002", "projectId": 1, "number": 2, "title": "Welcher Speicher?", "description": "",
  "state": "open", "occasion": null, "originKind": "session", "originName": "claude",
  "originSession": null, "tags": [], "ticketIds": ["ARC-0001"], "dependsOn": [], "decision": null,
  "createdAt": "2026-10-14T10:00:00.000Z", "createdById": "11111111-1111-1111-1111-111111111111",
  "updatedAt": "2026-10-14T10:00:00.000Z", "rev": 1,
  "projectKey": "ARC",
  "tickets": [{"id": "ARC-0001", "title": "Titel", "status": "backlog", "boardId": 1}],
  "waitingOn": [{"id": "ARC-F0003", "state": "open"}],
  "ready": false, "researched": true, "optionCount": 2
}"#;

#[test]
fn frage_fahrt_rund() {
    roundtrip::<BoardQuestion>(FRAGE);
    roundtrip::<BoardQuestionView>(FRAGE_ANSICHT);
}

#[test]
fn fragen_detail_hangt_recherche_kommentare_und_verlauf_an() {
    let ansicht: Value = serde_json::from_str(FRAGE_ANSICHT).unwrap();
    let mut detail = ansicht.as_object().unwrap().clone();
    detail.insert(
        "research".into(),
        serde_json::json!([{
            "id": 4294967296_i64, "questionId": "ARC-F0002", "at": "2026-10-14T10:30:00.000Z",
            "byId": "11111111-1111-1111-1111-111111111111", "byName": "claude", "kind": "session",
            "context": "Kontext",
            "options": [{"id": "O1", "title": "SQLite", "text": "…", "pros": ["einfach"],
                         "cons": ["Grenzen"], "recommended": true}]
        }]),
    );
    detail.insert(
        "comments".into(),
        serde_json::json!([{
            "id": 12, "questionId": "ARC-F0002",
            "authorId": "11111111-1111-1111-1111-111111111111", "authorName": "Florian",
            "authorKind": "human", "body": "Text", "createdAt": "2026-10-14T10:40:00.000Z"
        }]),
    );
    detail.insert(
        "history".into(),
        serde_json::json!([{
            "questionId": "ARC-F0002", "at": "2026-10-14T10:00:00.000Z",
            "byId": "11111111-1111-1111-1111-111111111111", "byName": "claude",
            "byKind": "session", "type": "created", "text": "angelegt"
        }, {
            "questionId": "ARC-F0002", "at": "2026-10-14T10:50:00.000Z",
            "byId": "11111111-1111-1111-1111-111111111111", "byName": "System",
            "byKind": null, "type": "ticket-deleted", "text": "ARC-0002 gelöscht"
        }]),
    );
    roundtrip::<BoardQuestionDetail>(&Value::Object(detail).to_string());
}

#[test]
fn recherche_kommentar_und_verlauf_einer_frage_fahren_rund() {
    // QuestionResearch.Id und QuestionComments.Id sind bigint
    roundtrip::<BoardQuestionResearch>(
        r#"{"id": 4294967296, "questionId": "ARC-F0001", "at": "2026-10-14T10:30:00.000Z",
            "byId": "11111111-1111-1111-1111-111111111111", "byName": "claude", "kind": "subagent",
            "context": "Kontext äöü",
            "options": [{"id": "O1", "title": "A", "text": "t", "pros": [], "cons": [],
                         "recommended": false}]}"#,
    );
    roundtrip::<BoardQuestionComment>(
        r#"{"id": 4294967297, "questionId": "ARC-F0001",
            "authorId": "11111111-1111-1111-1111-111111111111", "authorName": "Florian",
            "authorKind": "human", "body": "Text", "createdAt": "2026-10-14T10:40:00.000Z"}"#,
    );
    roundtrip::<BoardQuestionHistoryEntry>(
        r#"{"questionId": "ARC-F0001", "at": "2026-10-14T10:00:00.000Z",
            "byId": "11111111-1111-1111-1111-111111111111", "byName": "System", "byKind": null,
            "type": "ticket-deleted", "text": "ARC-0002 gelöscht"}"#,
    );
}

#[test]
fn fragen_akteur_schreibt_die_felder_des_board_akteurs_flach() {
    roundtrip::<BoardQuestionActor>(
        r#"{"userId": "11111111-1111-1111-1111-111111111111", "name": "claude",
            "kind": "session", "session": "s-1"}"#,
    );
}

#[test]
fn meldbare_frage_fahrt_rund() {
    roundtrip::<NotifiableBoardQuestion>(
        r#"{"id": "ARC-F0001", "title": "Welcher Speicher?", "occasion": null,
            "origin": {"kind": "session"},
            "research": [{"kind": "subagent",
                          "options": [{"id": "O1", "title": "A", "text": "t", "pros": ["p"],
                                       "cons": ["c"], "recommended": true}]}],
            "projectKey": "ARC", "projectName": "Arches", "repoPaths": ["/repo/arches"]}"#,
    );
}

// `runFixture({ id: 'ARC-0001-work-1', subjectId: 'ARC-0001', projectId: 1, boardId: 1 })`
const LAUF: &str = r#"{
  "id": "ARC-0001-work-1", "kind": "work", "subjectId": "ARC-0001", "projectId": 1, "boardId": 1,
  "agent": "junior-X", "attempt": 1, "profile": "standard", "escalatedFrom": null,
  "requestedModel": "opus", "model": "claude-opus-5-5", "effort": "high",
  "startedAt": "2026-10-13T10:00:00.000Z", "endedAt": "2026-10-13T10:05:00.000Z",
  "durationMs": 300000, "numTurns": 12, "exitCode": 0, "timedOut": false, "outcome": "done",
  "abortCategory": null, "abortReason": null, "costUsd": 0.123456, "inputTokens": 10,
  "outputTokens": 2000000000000, "cacheReadTokens": 30, "cacheWriteTokens": 40,
  "modelUsage": "{\"claude-opus-5-5\":{\"costUsd\":0.123456}}", "source": "dispatcher",
  "maxTurns": null, "timeoutMinutes": null, "profileReason": null, "ruleIndex": null,
  "configHash": null, "deniedCount": null, "deniedTools": null
}"#;

// Abgebrochener Lauf: alle Zahlen leer, dafür die Angaben aus dispatch-run-details
const LAUF_ABBRUCH: &str = r#"{
  "id": "ARC-0002-work-1", "kind": "merge", "subjectId": "ARC-0002", "projectId": 1,
  "boardId": null, "agent": "junior-X", "attempt": 2, "profile": "gross",
  "escalatedFrom": "klein", "requestedModel": null, "model": null, "effort": null,
  "startedAt": "2026-10-05T12:00:00.000Z", "endedAt": null, "durationMs": null, "numTurns": null,
  "exitCode": null, "timedOut": true, "outcome": "abort-final", "abortCategory": "timeout",
  "abortReason": "Zeitlimit äöü", "costUsd": null, "inputTokens": null, "outputTokens": null,
  "cacheReadTokens": null, "cacheWriteTokens": null, "modelUsage": null, "source": "comments",
  "maxTurns": 250, "timeoutMinutes": 90, "profileReason": "rule", "ruleIndex": 3,
  "configHash": "abc123", "deniedCount": 2, "deniedTools": "Bash,Write"
}"#;

#[test]
fn dispatch_lauf_fahrt_rund() {
    roundtrip::<DispatchRun>(LAUF);
    roundtrip::<DispatchRun>(LAUF_ABBRUCH);
}

#[test]
fn grosse_token_zahlen_bleiben_ganze_zahlen() {
    let lauf: DispatchRun = serde_json::from_str(LAUF).unwrap();
    assert_eq!(lauf.output_tokens, Some(2_000_000_000_000));
    assert!(serde_json::to_string(&lauf)
        .unwrap()
        .contains(r#""outputTokens":2000000000000"#));
}

#[test]
fn auswertung_und_filter_fahren_rund() {
    roundtrip::<nexus_core::board::DispatchConfigVersion>(
        r#"{"hash": "abc123", "dispatcher": "benjamin", "firstSeenAt": "2026-10-13T10:00:00.000Z",
            "content": "{\"profiles\":1}"}"#,
    );
    // boardId ist optional (`?`): fehlt es, darf es auch nicht geschrieben werden
    roundtrip::<nexus_core::board::DispatchRunFilter>(
        r#"{"projectId": 1, "from": "2026-10-01T00:00:00.000Z", "to": "2026-10-06T00:00:00.000Z"}"#,
    );
    roundtrip::<nexus_core::board::DispatchRunFilter>(
        r#"{"projectId": 1, "boardId": 2, "from": "2026-10-01T00:00:00.000Z",
            "to": "2026-10-06T00:00:00.000Z"}"#,
    );
    roundtrip::<nexus_core::board::DispatchCommentCounts>(r#"{"starts": 3, "aborts": 1}"#);
    roundtrip::<nexus_core::board::DispatchStats>(
        r#"{
          "kind": "work", "runsSince": "2026-09-01T00:00:00.000Z", "backfilledRuns": 4,
          "configChangedAt": null,
          "kpis": {"runs": 10, "subjects": 6, "done": 7, "aborted": 3, "solved": 5,
                   "successRate": 0.8333333333333334, "firstTryRate": null,
                   "escalationRate": 0.25, "finalFailures": 1},
          "series": {"unit": "week", "points": [{"start": "2026-09-28", "done": 2, "aborted": 1,
                                                 "other": 0}]},
          "byProfile": [{"profile": "standard", "runs": 10, "done": 7, "escalated": 2,
                         "final": 1, "other": 0}],
          "ladder": {"finished": 6, "firstTry": 0.5, "afterEscalation": null, "final": 0.125},
          "abortReasons": [{"category": "timeout", "label": "Zeitlimit", "count": 2}],
          "configurableAbortShare": null, "usagePauses": 1, "staleRuns": 0
        }"#,
    );
}

#[test]
fn suche_fahrt_rund() {
    roundtrip::<SearchRequest>(
        r#"{"terms": ["umlaut", "äöü"], "kinds": ["ticket", "question", "note"],
            "perSource": 30}"#,
    );
    roundtrip::<SearchRow>(
        r#"{"kind": "question", "id": "ARC-F0001", "ticketId": null, "field": "question-comment",
            "title": "Welcher Speicher?", "projectId": 1, "boardId": null, "status": "open",
            "text": "Ausschnitt", "textStart": 0, "textLength": 10, "at": null}"#,
    );
    roundtrip::<nexus_core::board::SearchRows>(
        r#"{"rows": [{"kind": "note", "id": "7", "ticketId": "ARC-0009", "field": "note",
                      "title": "Titel", "projectId": 1, "boardId": 1, "status": "taken",
                      "text": "Text", "textStart": 5, "textLength": 42,
                      "at": "2026-10-13T10:00:00.000Z"}],
            "truncated": true}"#,
    );
    roundtrip::<SearchHit>(
        r#"{"kind": "ticket", "id": "ARC-0001", "ticketId": "ARC-0001", "field": "title",
            "title": "Umlaute äöü", "project": {"id": 1, "key": "ARC", "name": "Arches"},
            "board": {"id": 1, "name": "Sprint"}, "status": "backlog", "snippet": "Umlaute äöü",
            "matches": [[0, 7], [8, 11]], "at": "2026-10-13T10:00:00.000Z"}"#,
    );
    roundtrip::<SearchResult>(
        r#"{"query": "umlaut", "terms": ["umlaut"],
            "hits": [{"kind": "ticket", "id": "ARC-0001", "ticketId": "ARC-0001",
                      "field": "description", "title": "Titel",
                      "project": {"id": 1, "key": "ARC", "name": "Arches"}, "board": null,
                      "status": "backlog", "snippet": "…", "matches": [], "at": null}],
            "truncated": false}"#,
    );
}

#[test]
fn arbeitsliste_fahrt_rund() {
    roundtrip::<InboxItem>(
        r#"{"key": "ticket:ARC-0001", "kind": "accept", "title": "Titel",
            "detail": "Human Interaction: ansehen", "boardId": 1, "boardName": "Sprint",
            "projectKey": "ARC", "ticketId": "ARC-0001", "questionId": null, "noteId": null,
            "waitingTickets": 0, "priority": "high", "since": "2026-10-13T10:00:00.000Z",
            "rank": 3}"#,
    );
    roundtrip::<InboxList>(
        r#"{"items": [{"key": "note:7", "kind": "note", "title": "Befund", "detail": null,
                       "boardId": 1, "boardName": "Sprint", "projectKey": "ARC",
                       "ticketId": null, "questionId": null, "noteId": 7, "waitingTickets": 0,
                       "priority": null, "since": "2026-10-13T10:00:00.000Z", "rank": 6}],
            "counts": {"merge": 0, "setup": 0, "question": 2, "accept": 1, "blocked": 0,
                       "planning": 0, "note": 1},
            "total": 4}"#,
    );
}

#[test]
fn enum_werte_stimmen_mit_den_typescript_literalen_uberein() {
    varianten(
        &TicketStatus::ALL,
        &[
            "backlog",
            "in_planning",
            "planned",
            "in_progress",
            "blocked",
            "done",
            "not_needed",
        ],
    );
    varianten(&Priority::ALL, &["low", "normal", "high", "urgent"]);
    varianten(&NoteState::ALL, &["open", "taken", "dropped"]);
    varianten(&AuthorKind::ALL, &["human", "session", "subagent"]);
    varianten(&QuestionState::ALL, &["open", "answered", "discarded"]);
    varianten(
        &BoardQuestionHistoryType::ALL,
        &[
            "created",
            "title",
            "description",
            "tags",
            "occasion",
            "tickets",
            "depends",
            "research",
            "decided",
            "discarded",
            "reopened",
            "ticket-deleted",
        ],
    );
    varianten(
        &BoardQuestionDecisionOutcome::ALL,
        &["answered", "discarded"],
    );
    varianten(&DispatchRunKind::ALL, &["work", "merge", "research"]);
    varianten(
        &DispatchRunOutcome::ALL,
        &[
            "done",
            "review",
            "human",
            "abort",
            "abort-final",
            "blocked",
            "question",
            "pause-limit",
            "pause-claude-missing",
            "setup-failed",
            "taken",
            "gone",
            "other",
            "merge-session",
            "research",
        ],
    );
    varianten(
        &DispatchProfileReason::ALL,
        &["default", "label", "plan", "rule", "setting"],
    );
    varianten(&DispatchRunSource::ALL, &["dispatcher", "comments"]);
    varianten(&DispatchSeriesUnit::ALL, &["day", "week"]);
    varianten(&SearchKind::ALL, &["ticket", "question", "note"]);
    varianten(
        &SearchField::ALL,
        &[
            "title",
            "description",
            "comment",
            "plan",
            "handbook",
            "acceptance",
            "decision",
            "question-comment",
            "note",
        ],
    );
    varianten(
        &InboxKind::ALL,
        &[
            "merge", "setup", "question", "accept", "blocked", "planning", "note",
        ],
    );
}

#[test]
fn as_str_und_json_sind_bei_jedem_enum_gleich() {
    for wert in DispatchRunOutcome::ALL {
        assert_eq!(
            serde_json::to_value(wert).unwrap(),
            Value::from(wert.as_str())
        );
    }
    for wert in SearchField::ALL {
        assert_eq!(
            serde_json::to_value(wert).unwrap(),
            Value::from(wert.as_str())
        );
    }
    for wert in BoardQuestionHistoryType::ALL {
        assert_eq!(
            serde_json::to_value(wert).unwrap(),
            Value::from(wert.as_str())
        );
    }
    for wert in InboxKind::ALL {
        assert_eq!(
            serde_json::to_value(wert).unwrap(),
            Value::from(wert.as_str())
        );
    }
}
