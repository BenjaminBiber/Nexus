//! Die Fragen-Beobachtung der Board-Datenbank; portiert aus
//! `packages/server/tests/unit/DbQuestionWatch.test.ts`.
//!
//! Abgefragt wird eine Attrappe statt einer Datenbank, gemeldet wird in eine Liste statt an
//! die Benachrichtigungen. Der Takt läuft über die angehaltene Uhr von tokio, damit die
//! Tests ohne echtes Warten durchlaufen.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use nexus_core::actors::AuthorKind;
use nexus_core::board::question_types::{
    NotifiableBoardQuestion, NotifiableOrigin, NotifiableResearch,
};
use nexus_core::registry::{ProjectState, RegisteredProject};
use nexus_core::{HarnessError, Result};
use nexus_server::notifications::db_question_watch::{DbQuestionSink, DbQuestionWatch};
use nexus_server::notifications::QuestionChange;

/// Eine Frage, wie sie die Board-Datenbank für Meldungen liefert
fn question(
    id: &str,
    kind: AuthorKind,
    research: usize,
    repo_paths: &[&str],
) -> NotifiableBoardQuestion {
    NotifiableBoardQuestion {
        id: id.to_string(),
        title: "Welcher Speicher?".to_string(),
        occasion: None,
        origin: NotifiableOrigin { kind },
        research: (0..research)
            .map(|_| NotifiableResearch {
                kind: AuthorKind::Session,
                options: Vec::new(),
            })
            .collect(),
        project_key: "ARC".to_string(),
        project_name: "Arches".to_string(),
        repo_paths: repo_paths.iter().map(|path| path.to_string()).collect(),
    }
}

/// Ein angemeldetes Harness-Projekt mit Pfad und Erreichbarkeit
fn project(id: &str, path: &str, reachable: bool) -> ProjectState {
    ProjectState {
        project: RegisteredProject {
            id: id.to_string(),
            name: id.to_string(),
            path: path.to_string(),
        },
        reachable,
    }
}

/// Die Attrappen eines Testlaufs: Bestand, Fehler und aufgezeichnete Meldungen
#[derive(Default)]
struct Fakes {
    current: Mutex<Vec<NotifiableBoardQuestion>>,
    /// Die Abfrage der Fragen schlägt fehl (keine Board-Datenbank)
    failing: AtomicBool,
    /// So viele Abfragen der Projektliste schlagen noch fehl
    list_failures: AtomicUsize,
    /// Die Meldung dieser Frage schlägt fehl
    fail_id: Mutex<Option<String>>,
    projects: Mutex<Vec<ProjectState>>,
    calls: Mutex<Vec<QuestionChange>>,
    question_calls: AtomicUsize,
    /// Hält die Abfrage der Fragen an, bis jemand benachrichtigt (Prüfung der Überlappung)
    gate: Mutex<Option<Arc<tokio::sync::Notify>>>,
}

impl Fakes {
    /// Der Bestand, den die nächste Abfrage liefert
    fn set(&self, questions: Vec<NotifiableBoardQuestion>) {
        *self.current.lock().expect("Sperre") = questions;
    }

    /// Die aufgezeichneten Meldungen als `(Art, Frage, Projekt-ID, Projektname, Reiter)`
    fn seen(&self) -> Vec<(String, String, String, String, Option<String>)> {
        self.calls
            .lock()
            .expect("Sperre")
            .iter()
            .map(|change| {
                (
                    format!("{:?}", change.kind),
                    change.question.id.clone(),
                    change.project_id.clone(),
                    change.project_name.clone(),
                    change.route.clone(),
                )
            })
            .collect()
    }

    /// Die Kennungen der gemeldeten Fragen
    fn ids(&self) -> Vec<String> {
        self.calls
            .lock()
            .expect("Sperre")
            .iter()
            .map(|change| change.question.id.clone())
            .collect()
    }
}

#[async_trait]
impl DbQuestionSink for Fakes {
    async fn on_question_change(&self, change: &QuestionChange) -> Result<()> {
        let failing = self.fail_id.lock().expect("Sperre").clone();
        if failing.as_deref() == Some(change.question.id.as_str()) {
            return Err(HarnessError::new("toast-kaputt", "Toast kaputt").into());
        }
        self.calls.lock().expect("Sperre").push(change.clone());
        Ok(())
    }
}

/// Fehler, der für eine fehlende Board-Datenbank steht
fn missing() -> nexus_core::Error {
    HarnessError::new("board-config-missing", "Board-Datenbank fehlt").into()
}

/// Beobachtung samt Attrappen; `interval_ms` 0 schaltet den Takt ab
fn setup(projects: Vec<ProjectState>, interval_ms: u64) -> (Arc<DbQuestionWatch>, Arc<Fakes>) {
    let fakes = Arc::new(Fakes::default());
    *fakes.projects.lock().expect("Sperre") = projects;
    let questions = {
        let fakes = fakes.clone();
        Arc::new(move || {
            let fakes = fakes.clone();
            Box::pin(async move {
                fakes.question_calls.fetch_add(1, Ordering::SeqCst);
                if fakes.failing.load(Ordering::SeqCst) {
                    return Err(missing());
                }
                let gate = fakes.gate.lock().expect("Sperre").clone();
                if let Some(gate) = gate {
                    gate.notified().await;
                }
                Ok(fakes.current.lock().expect("Sperre").clone())
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = _> + Send>>
        })
    };
    let list = {
        let fakes = fakes.clone();
        Arc::new(move || {
            let fakes = fakes.clone();
            Box::pin(async move {
                if fakes
                    .list_failures
                    .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                        left.checked_sub(1)
                    })
                    .is_ok()
                {
                    return Err(HarnessError::new("liste", "Projektliste fehlt").into());
                }
                Ok(fakes.projects.lock().expect("Sperre").clone())
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = _> + Send>>
        })
    };
    let watch = DbQuestionWatch::new(questions, list, fakes.clone(), interval_ms);
    (watch, fakes)
}

/// Die beiden Projekte der TypeScript-Tests
fn default_projects() -> Vec<ProjectState> {
    vec![
        project("p1", "C:/A", true),
        project("p2", "C:\\Projects\\Arches", true),
    ]
}

/// Beobachtung mit den Standard-Projekten und ohne Takt
fn standard() -> (Arc<DbQuestionWatch>, Arc<Fakes>) {
    setup(default_projects(), 0)
}

#[tokio::test]
async fn meldet_beim_ersten_durchlauf_nichts_danach_neue_fragen_und_recherchen_mit_passender_adresse(
) {
    let (watch, fakes) = standard();
    fakes.set(vec![question("ARC-F0001", AuthorKind::Session, 0, &[])]);
    watch.tick().await;
    assert!(fakes.seen().is_empty());

    fakes.set(vec![
        question("ARC-F0001", AuthorKind::Session, 1, &[]),
        question(
            "ARC-F0002",
            AuthorKind::Session,
            0,
            &["c:/projects/arches/"],
        ),
        question("ARC-F0003", AuthorKind::Human, 0, &[]),
    ]);
    watch.tick().await;
    assert_eq!(
        fakes.seen(),
        vec![
            (
                "Recherche".to_string(),
                "ARC-F0001".to_string(),
                "p1".to_string(),
                "ARC · Arches".to_string(),
                Some("questions".to_string())
            ),
            (
                "FrageNeu".to_string(),
                "ARC-F0002".to_string(),
                "p2".to_string(),
                "ARC · Arches".to_string(),
                Some("questions".to_string())
            ),
        ]
    );

    watch.tick().await;
    assert_eq!(fakes.ids().len(), 2);
}

#[tokio::test]
async fn bleibt_still_wenn_die_board_datenbank_fehlt_und_meldet_danach_nichts_doppelt() {
    let (watch, fakes) = standard();
    fakes.failing.store(true, Ordering::SeqCst);
    watch.tick().await;
    fakes.failing.store(false, Ordering::SeqCst);
    watch.tick().await;
    fakes.set(vec![question("ARC-F0001", AuthorKind::Session, 0, &[])]);
    watch.tick().await;
    watch.tick().await;
    assert_eq!(fakes.ids(), ["ARC-F0001"]);
}

#[tokio::test]
async fn nutzt_ohne_erreichbares_harness_projekt_eine_leere_projekt_id() {
    let (watch, fakes) = setup(vec![project("p1", "C:/A", false)], 0);
    watch.tick().await;
    fakes.set(vec![question("ARC-F0001", AuthorKind::Session, 0, &[])]);
    watch.tick().await;
    assert_eq!(
        fakes.calls.lock().expect("Sperre")[0].project_id,
        String::new()
    );
}

#[tokio::test]
async fn startet_nicht_mit_abstand_null() {
    let (watch, fakes) = standard();
    watch.start();
    watch.stop();
    tokio::task::yield_now().await;
    assert_eq!(fakes.question_calls.load(Ordering::SeqCst), 0);
    assert!(fakes.seen().is_empty());
}

#[tokio::test]
async fn verliert_bei_einer_fehlschlagenden_meldung_die_uebrigen_aenderungen_nicht() {
    let (watch, fakes) = standard();
    watch.tick().await;
    *fakes.fail_id.lock().expect("Sperre") = Some("ARC-F0001".to_string());
    fakes.set(vec![
        question("ARC-F0001", AuthorKind::Session, 0, &[]),
        question("ARC-F0002", AuthorKind::Session, 0, &[]),
        question("ARC-F0003", AuthorKind::Session, 0, &[]),
    ]);
    watch.tick().await;
    assert_eq!(fakes.ids(), ["ARC-F0002", "ARC-F0003"]);
}

#[tokio::test]
async fn meldet_bei_fehlgeschlagener_projektliste_im_naechsten_durchlauf_danach_nichts_doppelt() {
    let (watch, fakes) = standard();
    watch.tick().await;
    fakes.set(vec![question("ARC-F0001", AuthorKind::Session, 0, &[])]);
    fakes.list_failures.store(1, Ordering::SeqCst);
    watch.tick().await;
    assert!(fakes.seen().is_empty());
    watch.tick().await;
    assert_eq!(fakes.ids(), ["ARC-F0001"]);
    watch.tick().await;
    assert_eq!(fakes.ids().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn pollt_mit_positivem_abstand_bis_stop() {
    let (watch, fakes) = setup(default_projects(), 1_000);
    watch.start();
    // Der erste Takt läuft sofort, danach jede Sekunde
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    assert_eq!(fakes.question_calls.load(Ordering::SeqCst), 1);
    tokio::time::sleep(std::time::Duration::from_millis(1_000)).await;
    assert_eq!(fakes.question_calls.load(Ordering::SeqCst), 2);
    tokio::time::sleep(std::time::Duration::from_millis(1_000)).await;
    assert_eq!(fakes.question_calls.load(Ordering::SeqCst), 3);

    watch.stop();
    tokio::time::sleep(std::time::Duration::from_millis(5_000)).await;
    assert_eq!(fakes.question_calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn laesst_zwei_durchlaeufe_nicht_ueberlappen() {
    let (watch, fakes) = standard();
    // Der erste Durchlauf hängt in der Abfrage; der zweite sieht ihn laufen und kehrt um
    let gate = Arc::new(tokio::sync::Notify::new());
    *fakes.gate.lock().expect("Sperre") = Some(gate.clone());
    let erster = {
        let watch = watch.clone();
        tokio::spawn(async move { watch.tick().await })
    };
    tokio::task::yield_now().await;
    assert_eq!(fakes.question_calls.load(Ordering::SeqCst), 1);
    watch.tick().await;
    assert_eq!(fakes.question_calls.load(Ordering::SeqCst), 1);

    gate.notify_waiters();
    erster.await.expect("erster Durchlauf");
    // Danach läuft wieder einer
    *fakes.gate.lock().expect("Sperre") = None;
    watch.tick().await;
    assert_eq!(fakes.question_calls.load(Ordering::SeqCst), 2);
}
