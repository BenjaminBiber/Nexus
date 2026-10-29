//! Der Schattenbetrieb (`NEXUS_DISPATCH=shadow`): Node dispatcht und schreibt, Rust rechnet
//! nach und schreibt nur nach `dispatch/shadow/`.
//!
//! Die Node-Seite spielt hier der Rust-Dispatcher der Testumgebung: er legt Lauf-Datei,
//! Protokoll, Zustand, Worktree und Lauf-Zeile genauso an, wie der TS-Code es tut, und der
//! Schattenbetrieb sieht davon nur die Dateien und die Datenbank. Der erste Test ist der
//! Nachweis, dass er dabei nichts anfasst: Prüfsummen über Benutzerordner, Board-Datenbank
//! und Repo vor und nach mehreren Durchläufen.

mod common;
mod dispatch_support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use nexus_core::board::basics::TicketStatus;
use nexus_core::board::ticket_service::TicketPatch;
use nexus_core::dispatch::shadow::{ShadowStore, DECISIONS, EVALUATIONS, READINESS};
use nexus_core::dispatch::{RunKind, RunRecord};
use nexus_core::Result;
use nexus_server::dispatch::research_runner::{ResearchRunner, StartMode};
use nexus_server::dispatch::shadow::{ShadowDeps, ShadowService, SidecarStatus};
use serde_json::{json, Value};

use dispatch_support::{DispatchFixture, OK_OUTPUT};

/// Ein Sidecar, der immer dieselbe Antwort gibt; `None` = der Node-Server antwortet nicht
#[derive(Debug, Default)]
struct FakeSidecar(Mutex<Option<Value>>);

impl FakeSidecar {
    /// Sidecar, der nicht erreichbar ist (der Bereitschafts-Vergleich fällt dann aus)
    fn silent() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Sidecar, der diese Bereitschaft meldet
    fn with(status: Value) -> Arc<Self> {
        Arc::new(Self(Mutex::new(Some(status))))
    }
}

#[async_trait]
impl SidecarStatus for FakeSidecar {
    async fn status(&self) -> Result<Value> {
        self.0
            .lock()
            .expect("Antwort")
            .clone()
            .ok_or_else(|| nexus_core::HarnessError::new("x", "Node antwortet nicht").into())
    }
}

/// Der Schattenbetrieb auf dem Benutzerordner der Testumgebung
fn shadow(f: &DispatchFixture, sidecar: Arc<dyn SidecarStatus>) -> Arc<ShadowService> {
    Arc::new(ShadowService::new(ShadowDeps {
        home: f.home.clone(),
        boards: f.holder.clone(),
        clock: f.clock(),
        env: BTreeMap::new(),
        platform: "linux".into(),
        sidecar,
        log: Arc::new(|_| {}),
    }))
}

/// Die Zeilen einer Tagesdatei des Schattenbetriebs, über alle Tage hinweg
fn lines(f: &DispatchFixture, prefix: &str) -> Vec<Value> {
    let dir = f.home.dispatch_shadow_dir();
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with(&format!("{prefix}-")))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
        .iter()
        .flat_map(|path| {
            std::fs::read_to_string(path)
                .expect("Tagesdatei")
                .lines()
                .map(|line| serde_json::from_str(line).expect("JSON-Zeile"))
                .collect::<Vec<Value>>()
        })
        .collect()
}

/// Ein Abdruck aller Dateien eines Ordners (Pfad und Inhalt), ohne `dispatch/shadow`
fn files_under(root: &Path, skip: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut found = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == skip {
            continue;
        }
        if path.is_dir() {
            found.extend(files_under(&path, skip));
        } else if let Ok(bytes) = std::fs::read(&path) {
            found.insert(path.to_string_lossy().into_owned(), bytes);
        }
    }
    found
}

/// Alles, was der Schattenbetrieb nicht anfassen darf: Benutzerordner (ohne `dispatch/shadow`),
/// Repo und die Zeilen der Board-Datenbank samt Änderungsstand
async fn untouched(f: &DispatchFixture) -> Value {
    let rows: Vec<Value> = f
        .rows()
        .await
        .into_iter()
        .map(|row| serde_json::to_value(row).expect("Lauf-Zeile"))
        .collect();
    let tickets: Vec<Value> = f
        .backend
        .services
        .tickets
        .list(&nexus_core::board::ticket_service::TicketListFilter {
            board_ids: vec![f.board.id],
            ..Default::default()
        })
        .await
        .expect("Tickets")
        .into_iter()
        .map(|view| json!({ "id": view.ticket.id, "rowver": view.ticket.updated_at, "status": view.ticket.status, "assignee": view.ticket.assignee }))
        .collect();
    let home = files_under(f.home.dir(), &f.home.dispatch_shadow_dir());
    let repo = files_under(f.repo.path(), Path::new(""));
    json!({
        "rows": rows,
        "tickets": tickets,
        "home": home.keys().collect::<Vec<_>>(),
        "homeBytes": home.values().map(Vec::len).collect::<Vec<_>>(),
        "repo": repo.keys().collect::<Vec<_>>(),
    })
}

/// Ein Ticket, das Node gestartet und beendet hat: Lauf-Datei, Protokoll und Ergebnis stehen
/// im Benutzerordner, die Entscheidung ist noch nicht verglichen
async fn gestartet(f: &DispatchFixture, title: &str) -> (String, RunRecord) {
    let id = f.ready(title, TicketPatch::default()).await;
    f.tick().await;
    let record = f.run(&id);
    (id, record)
}

#[tokio::test]
async fn der_schattenbetrieb_schreibt_nur_in_seinen_eigenen_ordner() {
    let f = DispatchFixture::create(json!({ "wip": 2 })).await;
    let (first, _) = gestartet(&f, "Erstes").await;
    f.end(&first, OK_OUTPUT, Some(0));
    let (_, zweiter) = gestartet(&f, "Zweites").await;
    // Eine Frage und ein beendeter Lauf dazu: der Schatten sieht beide Laufarten
    f.question(&first, "Welches Format?").await;

    let service = shadow(
        &f,
        FakeSidecar::with(json!({ "tickets": [{ "id": first, "reason": "bereit" }] })),
    );
    let vorher = untouched(&f).await;
    for _ in 0..3 {
        service.run_tick().await;
    }
    let nachher = untouched(&f).await;
    assert_eq!(vorher, nachher, "der Schattenbetrieb hat etwas verändert");

    // Nur sein eigener Ordner ist dazugekommen – und dort steht zu jedem Lauf eine Zeile
    assert!(f.home.dispatch_shadow_dir().is_dir());
    let decisions = lines(&f, DECISIONS);
    assert_eq!(decisions.len(), 2, "{decisions:#?}");
    assert!(decisions.iter().any(|line| line["runId"] == zweiter.id));
    // Mehrere Durchläufe geben jedem Lauf trotzdem nur eine Zeile
    assert_eq!(
        decisions
            .iter()
            .filter(|line| line["runId"] == zweiter.id)
            .count(),
        1
    );
}

#[tokio::test]
async fn ein_von_node_gestarteter_lauf_wird_als_gleich_verglichen() {
    let f = DispatchFixture::create(json!({})).await;
    let (id, record) = gestartet(&f, "Gleich").await;

    shadow(&f, FakeSidecar::silent()).run_tick().await;

    let decisions = lines(&f, DECISIONS);
    assert_eq!(decisions.len(), 1, "{decisions:#?}");
    let line = &decisions[0];
    assert_eq!(line["runId"], record.id);
    assert_eq!(line["ticketId"], id);
    assert_eq!(line["kind"], "work");
    assert_eq!(line["equal"], true, "{line:#?}");
    assert_eq!(line["diffs"].as_array().expect("Abweichungen").len(), 0);
    assert!(line.get("promptDiff").is_none(), "{line:#?}");
    assert!(line.get("skipped").is_none(), "{line:#?}");
}

#[tokio::test]
async fn ein_abweichender_lauf_nennt_feld_node_wert_und_rust_wert() {
    let f = DispatchFixture::create(json!({})).await;
    let (_, record) = gestartet(&f, "Abweichend").await;
    // Wie eine von Hand angelegte Lauf-Datei aus dem TS-Code: anderes Profil, anderer
    // Versuch und eine andere Arbeitsanweisung
    let mut geaendert = record.clone();
    geaendert.run.profile = "schwer".into();
    geaendert.run.max_turns = 999;
    geaendert.attempt = Some(7);
    geaendert.prompt = format!("{}\nEine Zeile zu viel", record.prompt);
    f.ctx
        .stores
        .runs
        .create(&geaendert)
        .expect("Lauf-Datei neu schreiben");

    shadow(&f, FakeSidecar::silent()).run_tick().await;

    let line = lines(&f, DECISIONS).remove(0);
    assert_eq!(line["equal"], false, "{line:#?}");
    let diffs: BTreeMap<String, (String, String)> = line["diffs"]
        .as_array()
        .expect("Abweichungen")
        .iter()
        .map(|diff| {
            (
                diff["field"].as_str().expect("Feld").to_string(),
                (
                    diff["node"].as_str().unwrap_or_default().to_string(),
                    diff["rust"].as_str().unwrap_or_default().to_string(),
                ),
            )
        })
        .collect();
    assert_eq!(
        diffs.get("run.profile"),
        Some(&("schwer".into(), record.run.profile.clone())),
        "{line:#?}"
    );
    assert_eq!(
        diffs.get("run.maxTurns"),
        Some(&("999".into(), record.run.max_turns.to_string())),
        "{line:#?}"
    );
    assert_eq!(
        diffs.get("attempt"),
        Some(&("7".into(), "1".into())),
        "{line:#?}"
    );
    assert!(diffs.contains_key("prompt"), "{line:#?}");
    let diff = line["promptDiff"].as_str().expect("Prompt-Diff");
    assert!(diff.contains("-Eine Zeile zu viel"), "{diff}");
}

#[tokio::test]
async fn ohne_ticket_bleibt_die_entscheidung_ohne_urteil() {
    let f = DispatchFixture::create(json!({})).await;
    let (_, record) = gestartet(&f, "Verschwunden").await;
    let mut verwaist = record.clone();
    verwaist.ticket_id = "ARC-9999".into();
    f.ctx.stores.runs.create(&verwaist).expect("Lauf-Datei");

    shadow(&f, FakeSidecar::silent()).run_tick().await;

    let line = lines(&f, DECISIONS).remove(0);
    assert_eq!(line["equal"], false, "{line:#?}");
    assert!(
        line["skipped"]
            .as_str()
            .expect("Grund")
            .contains("ARC-9999"),
        "{line:#?}"
    );
}

#[tokio::test]
async fn eine_auswertung_wird_mit_der_zeile_in_dispatch_runs_verglichen() {
    let f = DispatchFixture::create(json!({})).await;
    let (id, record) = gestartet(&f, "Ausgewertet").await;
    f.end(&id, OK_OUTPUT, Some(0));
    // Der Schatten kopiert Lauf und Ergebnis, bevor Node sie nach der Auswertung entfernt
    let service = shadow(&f, FakeSidecar::silent());
    service.run_tick().await;
    assert!(f
        .home
        .dispatch_shadow_runs_dir()
        .join(format!("{}.json", record.id))
        .is_file());

    // Jetzt wertet Node aus: Lauf-Zeile in die Datenbank, Lauf-Datei weg
    f.evaluate(&id).await.expect("Auswertung");
    f.flush().await.expect("Zeilen übertragen");
    service.run_tick().await;

    let evaluations = lines(&f, EVALUATIONS);
    assert_eq!(evaluations.len(), 1, "{evaluations:#?}");
    let line = &evaluations[0];
    assert_eq!(line["runId"], record.id);
    assert_eq!(line["ticketId"], id);
    assert_eq!(line["missingRow"], false);
    assert_eq!(line["equal"], true, "{line:#?}");
    assert!(line["rowId"]
        .as_str()
        .expect("Zeilen-ID")
        .starts_with(&format!("{}-", record.id)));
    // Die Kopien sind weg: verglichen wird jeder Lauf nur einmal
    assert!(!f
        .home
        .dispatch_shadow_runs_dir()
        .join(format!("{}.result.json", record.id))
        .is_file());
    service.run_tick().await;
    assert_eq!(lines(&f, EVALUATIONS).len(), 1);
}

#[tokio::test]
async fn ohne_zeile_in_dispatch_runs_wird_nach_der_wartefrist_missing_row_vermerkt() {
    let f = DispatchFixture::create(json!({})).await;
    let (id, record) = gestartet(&f, "Ohne Zeile").await;
    f.end(&id, OK_OUTPUT, Some(0));
    let service = shadow(&f, FakeSidecar::silent());
    service.run_tick().await;
    // Node hat den Lauf entfernt, aber nie eine Zeile geschrieben
    f.drop_run(&record.id);

    // Innerhalb der Wartefrist wird weiter gewartet
    service.run_tick().await;
    assert!(lines(&f, EVALUATIONS).is_empty());

    f.advance(nexus_server::dispatch::shadow::WAIT_MINUTES + 1);
    service.run_tick().await;
    let line = lines(&f, EVALUATIONS).remove(0);
    assert_eq!(line["missingRow"], true, "{line:#?}");
    assert_eq!(line["equal"], false, "{line:#?}");
    assert!(line.get("rowId").is_none(), "{line:#?}");
}

#[tokio::test]
async fn abweichende_bereitschaft_steht_als_zeile_da_gleiche_nicht() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Bereit", TicketPatch::default()).await;

    // Node sieht dasselbe wie Rust: keine Zeile
    let gleich = f.service.status(Some(f.board.id)).await.expect("Zustand");
    let rows: Vec<Value> = gleich
        .tickets
        .iter()
        .map(|row| json!({ "id": row.id, "reason": row.reason }))
        .collect();
    shadow(&f, FakeSidecar::with(json!({ "tickets": rows })))
        .run_tick()
        .await;
    assert!(lines(&f, READINESS).is_empty());

    // Node hält dasselbe Ticket für blockiert
    shadow(
        &f,
        FakeSidecar::with(json!({
            "tickets": [{ "id": id, "reason": "wartet auf ARC-0001" }]
        })),
    )
    .run_tick()
    .await;
    let line = lines(&f, READINESS).remove(0);
    assert_eq!(line["ticketId"], id);
    assert_eq!(line["node"], "wartet auf ARC-0001");
    assert_eq!(line["rust"], "bereit");
    assert_eq!(line["timing"], true, "das Ticket ist gerade entstanden");
}

#[tokio::test]
async fn auch_eine_recherche_wird_nachgerechnet() {
    let f = DispatchFixture::create(json!({})).await;
    let id = f.ready("Mit Frage", TicketPatch::default()).await;
    let question = f.question(&id, "Welches Format?").await;
    // Die Recherche startet Node; der Schatten rechnet ihr Profil und ihre Anweisung nach
    let record = ResearchRunner::new(&f.ctx)
        .start(&question, StartMode::Manual)
        .await
        .expect("Recherche starten");
    assert_eq!(record.kind, RunKind::Research);

    shadow(&f, FakeSidecar::silent()).run_tick().await;

    let decisions = lines(&f, DECISIONS);
    let research = decisions
        .iter()
        .find(|line| line["runId"] == record.id)
        .unwrap_or_else(|| panic!("keine Recherche: {decisions:#?}"));
    assert_eq!(research["kind"], RunKind::Research.as_str());
    assert_eq!(research["equal"], true, "{research:#?}");
}

#[tokio::test]
async fn ein_vom_menschen_uebernommenes_ticket_aendert_nichts_am_schatten() {
    let f = DispatchFixture::create(json!({})).await;
    let (id, _) = gestartet(&f, "Übernommen").await;
    let actor = f.actor();
    f.backend
        .services
        .tickets
        .update(
            &id,
            &TicketPatch {
                status: Some(TicketStatus::Blocked),
                ..TicketPatch::default()
            },
            &actor,
            &nexus_core::board::ticket_service::WriteOptions::default(),
        )
        .await
        .expect("Ticket blockieren");

    let vorher = untouched(&f).await;
    shadow(&f, FakeSidecar::silent()).run_tick().await;
    assert_eq!(vorher, untouched(&f).await);
    // Der Vergleich läuft trotzdem; der Zustand des Tickets geht ihn nichts an
    assert_eq!(lines(&f, DECISIONS).len(), 1);
}

#[tokio::test]
async fn der_bericht_fasst_zusammen_was_der_schattenbetrieb_geschrieben_hat() {
    let f = DispatchFixture::create(json!({})).await;
    let (_, record) = gestartet(&f, "Für den Bericht").await;
    let mut geaendert = record.clone();
    geaendert.run.profile = "schwer".into();
    f.ctx.stores.runs.create(&geaendert).expect("Lauf-Datei");
    shadow(&f, FakeSidecar::silent()).run_tick().await;

    let report = ShadowStore::new(f.home.clone())
        .report(None, None)
        .expect("Bericht");
    assert_eq!(report.summary.runs, 1);
    assert_eq!(report.summary.decisions_different, 1);
    let markdown = report.markdown();
    assert!(markdown.contains("| run.profile | schwer |"), "{markdown}");
}
