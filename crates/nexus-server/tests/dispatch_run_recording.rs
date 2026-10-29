//! Erfassung der Läufe für die Auswertung; portiert aus
//! `packages/server/tests/unit/dispatch/DispatchRunRecording.test.ts`.

mod common;
mod dispatch_support;

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::dispatch_run_types::DispatchProfileReason;
use nexus_core::board::ticket_service::{MoveOptions, TicketPatch};
use nexus_core::dispatch::DispatchConfigFingerprint;
use nexus_server::dispatch::ticket_starter::StartOutcome;
use serde_json::json;

use dispatch_support::{DispatchFixture, OK_OUTPUT};

/// Ergebnis-Ereignis wie von `claude -p --output-format stream-json` (gekürzt)
const COST_OUTPUT: &str = concat!(
    r#"{"type":"system","subtype":"init","model":"opus"}"#,
    "\n",
    r#"{"type":"result","subtype":"success","is_error":false,"result":"fertig","num_turns":17,"#,
    r#""duration_ms":240000,"total_cost_usd":0.42,"#,
    r#""usage":{"input_tokens":12,"output_tokens":3400,"cache_read_input_tokens":50000,"#,
    r#""cache_creation_input_tokens":9000},"#,
    r#""modelUsage":{"claude-opus-5-5":{"inputTokens":12,"outputTokens":3400,"costUSD":0.4},"#,
    r#""claude-haiku-5-5":{"inputTokens":1,"outputTokens":2,"costUSD":0.02}}}"#,
);

/// Ergebnis mit zwei abgelehnten Aufrufen desselben Werkzeugs und einem anderen
const DENIED_OUTPUT: &str = concat!(
    r#"{"type":"result","subtype":"success","is_error":false,"result":"fertig","#,
    r#""permission_denials":[{"tool_name":"Bash","tool_input":{"command":"git push"}},"#,
    r#"{"tool_name":"Bash","tool_input":{"command":"rm -rf /"}},{"tool_name":"Edit"}]}"#,
);

/// Versuch, Profil, Basisprofil, Ausgang, Abbruchkategorie und Kosten einer Lauf-Zeile
type Escalation<'a> = (
    Option<i32>,
    &'a str,
    Option<&'a str>,
    &'a str,
    Option<&'a str>,
    Option<f64>,
);

/// Der Lauf des Tickets, wenn er gestartet wurde
fn started(outcome: StartOutcome) -> nexus_core::dispatch::RunRecord {
    match outcome {
        StartOutcome::Started(record) => *record,
        other => panic!("Start erwartet, nicht {other:?}"),
    }
}

/// Startet das Ticket, meldet es erledigt und schreibt dieses Ergebnis
async fn done(
    f: &DispatchFixture,
    title: &str,
    output: &str,
) -> (String, nexus_core::dispatch::RunRecord) {
    let id = f.ready(title, TicketPatch::default()).await;
    let record = started(f.start(&id).await.expect("Start"));
    f.backend
        .services
        .tickets
        .move_to(
            &id,
            TicketStatus::Done,
            &f.agent(&id),
            &MoveOptions::default(),
        )
        .await
        .expect("erledigt melden");
    f.advance(4);
    f.end(&id, output, Some(0));
    (id, record)
}

#[tokio::test]
async fn erledigter_lauf_wird_mit_kosten_tokens_modell_profil_und_versuch_erfasst() {
    let f = DispatchFixture::create(json!({})).await;
    let (id, record) = done(&f, "Fertig", COST_OUTPUT).await;
    f.evaluate(&id).await.expect("Auswertung");
    f.flush().await.expect("Übertragen");
    let rows = f.rows().await;
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert!(
        row.id.starts_with(&format!("{}-", record.id))
            && row.id.len() == record.id.len() + 9
            && row.id[record.id.len() + 1..]
                .chars()
                .all(|c| c.is_ascii_hexdigit()),
        "{} trägt einen zufälligen Anhang",
        row.id
    );
    assert_eq!(row.kind.as_str(), "work");
    assert_eq!(row.subject_id, id);
    assert_eq!(row.board_id, Some(f.board.id));
    assert_eq!(row.agent, format!("junior-{id}"));
    assert_eq!(row.attempt, Some(1));
    assert_eq!(row.profile, "standard");
    assert_eq!(row.escalated_from, None);
    assert_eq!(row.requested_model.as_deref(), Some("opus"));
    assert_eq!(row.model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(row.effort.as_deref(), Some("high"));
    assert_eq!(row.outcome.as_str(), "done");
    assert_eq!(row.abort_category, None);
    assert_eq!(row.cost_usd, Some(0.42));
    assert_eq!(row.input_tokens, Some(12));
    assert_eq!(row.output_tokens, Some(3400));
    assert_eq!(row.cache_read_tokens, Some(50_000));
    assert_eq!(row.cache_write_tokens, Some(9_000));
    assert_eq!(row.num_turns, Some(17));
    assert_eq!(row.exit_code, Some(0));
    assert!(!row.timed_out);
    assert_eq!(row.source.as_str(), "dispatcher");
    assert_eq!(row.started_at, record.started_at);
    // Limits und Grund der Profilwahl, wie sie für den Lauf galten
    assert_eq!(row.max_turns, Some(record.run.max_turns as i32));
    assert_eq!(row.timeout_minutes, Some(record.run.timeout_minutes as i32));
    assert_eq!(row.profile_reason, Some(DispatchProfileReason::Default));
    assert_eq!(row.rule_index, None);
    assert!(
        row.model_usage
            .as_deref()
            .expect("Verbrauch je Modell")
            .contains("claude-haiku-5-5"),
        "{:?}",
        row.model_usage
    );
    assert!(f.kept().is_empty(), "nichts bleibt vorgemerkt");
}

#[tokio::test]
async fn abbruch_und_eskalation_werden_mit_kategorie_und_basisprofil_erfasst() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Schwierig", TicketPatch::default()).await;
    started(f.start(&id).await.expect("Start"));
    f.end(&id, "Fehlerausgabe", Some(2));
    f.evaluate(&id).await.expect("Auswertung");
    f.advance(1);
    started(f.start(&id).await.expect("Start"));
    f.end(&id, OK_OUTPUT, Some(0));
    f.evaluate(&id).await.expect("Auswertung");
    f.flush().await.expect("Übertragen");
    let rows = f.rows().await;
    let seen: Vec<Escalation<'_>> = rows
        .iter()
        .map(|row| {
            (
                row.attempt,
                row.profile.as_str(),
                row.escalated_from.as_deref(),
                row.outcome.as_str(),
                row.abort_category.as_deref(),
                row.cost_usd,
            )
        })
        .collect();
    assert_eq!(
        seen,
        vec![
            (Some(1), "standard", None, "abort", Some("exit-code"), None),
            (
                Some(2),
                "schwer",
                Some("standard"),
                "abort-final",
                Some("no-outcome"),
                None
            ),
        ]
    );
}

#[tokio::test]
async fn ein_nutzungslimit_wird_als_pause_erfasst_nicht_als_abbruch() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Limit", TicketPatch::default()).await;
    started(f.start(&id).await.expect("Start"));
    f.end(&id, "Claude usage limit reached", Some(1));
    f.evaluate(&id).await.expect("Auswertung");
    f.flush().await.expect("Übertragen");
    assert_eq!(
        f.rows()
            .await
            .iter()
            .map(|row| row.outcome.as_str())
            .collect::<Vec<_>>(),
        vec!["pause-limit"]
    );
}

#[tokio::test]
async fn ein_gescheiterter_start_wird_als_abbruch_mit_kategorie_start_failed_erfasst() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Kein Claude", TicketPatch::default()).await;
    f.launcher.fail_with("claude fehlt");
    f.start(&id).await.expect("Start");
    f.advance(1);
    f.start(&id).await.expect("Start");
    f.flush().await.expect("Übertragen");
    let rows = f.rows().await;
    assert_eq!(
        rows.iter()
            .map(|row| (
                row.subject_id.as_str(),
                row.outcome.as_str(),
                row.abort_category.as_deref(),
                row.attempt
            ))
            .collect::<Vec<_>>(),
        vec![
            (id.as_str(), "abort", Some("start-failed"), Some(1)),
            (id.as_str(), "abort-final", Some("start-failed"), Some(2)),
        ]
    );
    // Auch ohne Lauf stehen Limits und Grund der Profilwahl in der Zeile – der zweite Versuch
    // mit den Limits des eskalierten Profils
    assert_eq!(
        rows.iter()
            .map(|row| (
                row.max_turns,
                row.timeout_minutes,
                row.profile_reason,
                row.rule_index
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                Some(80),
                Some(90),
                Some(DispatchProfileReason::Default),
                None
            ),
            (
                Some(150),
                Some(120),
                Some(DispatchProfileReason::Default),
                None
            ),
        ]
    );
}

#[tokio::test]
async fn ein_lauf_eines_aelteren_stands_ohne_grund_bleibt_ohne_profile_reason() {
    let f = DispatchFixture::create(json!({})).await;
    let (id, record) = done(&f, "Alt", COST_OUTPUT).await;
    // Lauf-Datei zurückschreiben, wie ein Stand ohne reason/ruleIndex sie geschrieben hätte
    f.ctx
        .stores
        .runs
        .create(&nexus_core::dispatch::RunRecord {
            run: nexus_core::dispatch::RunSettings {
                reason: None,
                rule_index: None,
                ..record.run.clone()
            },
            ..record.clone()
        })
        .expect("Lauf zurückschreiben");
    f.evaluate(&id).await.expect("Auswertung");
    f.flush().await.expect("Übertragen");
    assert_eq!(
        f.rows()
            .await
            .iter()
            .map(|row| (row.profile_reason, row.rule_index, row.max_turns))
            .collect::<Vec<_>>(),
        vec![(None, None, Some(80))]
    );
}

#[tokio::test]
async fn die_konfig_version_wird_genau_einmal_angelegt() {
    let f = DispatchFixture::create(json!({})).await;
    let (id, _) = done(&f, "Konfig", COST_OUTPUT).await;
    f.evaluate(&id).await.expect("Auswertung");
    f.flush().await.expect("Übertragen");
    let hash = DispatchConfigFingerprint::new().hash(&f.cfg);
    assert_eq!(
        f.rows()
            .await
            .iter()
            .map(|row| row.config_hash.clone())
            .collect::<Vec<_>>(),
        vec![Some(hash.clone())]
    );
    let versions = f
        .backend
        .services
        .dispatch_runs
        .config_versions("2000-01-01T00:00:00.000Z", "2100-01-01T00:00:00.000Z")
        .await
        .expect("Fassungen");
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].hash, hash);
    assert_eq!(versions[0].dispatcher, f.cfg.dispatcher_name);
    let content: serde_json::Value =
        serde_json::from_str(&versions[0].content).expect("Inhalt als JSON");
    assert_eq!(
        content["defaultProfile"].as_str(),
        Some(f.cfg.default_profile.as_str())
    );

    // Zweiter Lauf derselben Einstellung: keine zweite Fassung
    let (next, _) = done(&f, "Noch einer", COST_OUTPUT).await;
    f.evaluate(&next).await.expect("Auswertung");
    f.flush().await.expect("Übertragen");
    let again = f
        .backend
        .services
        .dispatch_runs
        .config_versions("2000-01-01T00:00:00.000Z", "2100-01-01T00:00:00.000Z")
        .await
        .expect("Fassungen");
    assert_eq!(again, versions);
}

#[tokio::test]
async fn ein_lauf_eines_aelteren_stands_ohne_hash_bleibt_ohne_config_hash() {
    let f = DispatchFixture::create(json!({})).await;
    let (id, record) = done(&f, "Ohne Hash", COST_OUTPUT).await;
    f.ctx
        .stores
        .runs
        .create(&nexus_core::dispatch::RunRecord {
            run: nexus_core::dispatch::RunSettings {
                config_hash: None,
                ..record.run.clone()
            },
            ..record.clone()
        })
        .expect("Lauf zurückschreiben");
    f.evaluate(&id).await.expect("Auswertung");
    f.flush().await.expect("Übertragen");
    assert_eq!(
        f.rows()
            .await
            .iter()
            .map(|row| row.config_hash.clone())
            .collect::<Vec<_>>(),
        vec![None]
    );
}

#[tokio::test]
async fn abgelehnte_werkzeugaufrufe_werden_mit_anzahl_und_namen_erfasst() {
    let f = DispatchFixture::create(json!({})).await;
    let (id, _) = done(&f, "Abgelehnt", DENIED_OUTPUT).await;
    f.evaluate(&id).await.expect("Auswertung");
    f.flush().await.expect("Übertragen");
    assert_eq!(
        f.rows()
            .await
            .iter()
            .map(|row| (
                row.outcome.as_str(),
                row.denied_count,
                row.denied_tools.clone()
            ))
            .collect::<Vec<_>>(),
        vec![("done", Some(3), Some("Bash, Edit".to_string()))]
    );
}

#[tokio::test]
async fn ein_ergebnis_ohne_ablehnungen_zaehlt_null() {
    let f = DispatchFixture::create(json!({})).await;
    let (id, _) = done(&f, "Ohne Ablehnung", OK_OUTPUT).await;
    f.evaluate(&id).await.expect("Auswertung");
    f.flush().await.expect("Übertragen");
    assert_eq!(
        f.rows()
            .await
            .iter()
            .map(|row| (row.denied_count, row.denied_tools.clone()))
            .collect::<Vec<_>>(),
        vec![(Some(0), None)]
    );
}

#[tokio::test]
async fn eine_ausgabe_ohne_ergebnis_objekt_laesst_die_ablehnungen_leer() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Hart beendet", TicketPatch::default()).await;
    started(f.start(&id).await.expect("Start"));
    f.end(&id, "Fehlerausgabe ohne JSON", Some(1));
    f.evaluate(&id).await.expect("Auswertung");
    f.flush().await.expect("Übertragen");
    assert_eq!(
        f.rows()
            .await
            .iter()
            .map(|row| (row.denied_count, row.denied_tools.clone()))
            .collect::<Vec<_>>(),
        vec![(None, None)]
    );
}

#[tokio::test]
async fn ein_verbindungsfehler_laesst_den_lauf_vorgemerkt_und_schreibt_ihn_spaeter() {
    let f = DispatchFixture::create(json!({})).await;
    let (id, _) = done(&f, "Netz", COST_OUTPUT).await;
    f.evaluate(&id).await.expect("Auswertung");
    // Die Datenbank ist für den Schreibversuch weg
    f.backend.break_connection();
    let error = f.flush().await.expect_err("Verbindungsfehler");
    f.backend.mend_connection();
    assert_eq!(error.code(), Some("board-db-unreachable"));
    // Der Lauf ist erledigt (wird nicht erneut ausgewertet), die Zeile bleibt vorgemerkt
    assert!(f.ctx.stores.runs.list().expect("Läufe").is_empty());
    assert_eq!(f.kept().len(), 1);
    assert!(f.rows().await.is_empty());

    assert_eq!(f.flush().await.expect("Übertragen"), 1);
    assert_eq!(
        f.rows()
            .await
            .iter()
            .map(|row| row.outcome.as_str())
            .collect::<Vec<_>>(),
        vec!["done"]
    );
    assert!(f.kept().is_empty());
}

#[tokio::test]
async fn eine_fehlende_erweiterung_laesst_den_lauf_vorgemerkt_und_schreibt_ihn_danach() {
    let f = DispatchFixture::create(json!({})).await;
    let (id, _) = done(&f, "Schema", COST_OUTPUT).await;
    f.evaluate(&id).await.expect("Auswertung");
    f.backend.outdate_schema();
    // Kein Fehler: die Zeilen gehen nach dem Einspielen hinein
    let written = f.flush().await.expect("Übertragen");
    f.backend.mend_connection();
    assert_eq!(written, 0);
    assert_eq!(f.kept().len(), 1);
    assert!(f.rows().await.is_empty());

    assert_eq!(f.flush().await.expect("Übertragen"), 1);
    assert_eq!(
        f.rows()
            .await
            .iter()
            .map(|row| row.outcome.as_str())
            .collect::<Vec<_>>(),
        vec!["done"]
    );
    assert!(f.kept().is_empty());
}
