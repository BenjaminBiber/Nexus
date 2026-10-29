//! Beobachtet die offenen eigenen Fragen der Board-Datenbank; portiert aus
//! `packages/server/src/notifications/DbQuestionWatch.ts`.
//!
//! Der Start liest sofort den Bestand ein (ohne Meldung) und vergleicht danach im Takt.
//! Ohne eingerichtete oder erreichbare Board-Datenbank bleibt die Beobachtung still.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use nexus_core::board::question_types::NotifiableBoardQuestion;
use nexus_core::registry::ProjectState;
use nexus_core::Result;
use tokio::task::JoinHandle;

use crate::notifications::question_changes::QuestionChangeDetector;
use crate::notifications::service::QuestionChange;

/// Eine Abfrage, die eine Zukunft liefert
pub type Query<T> = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<T>> + Send>> + Send + Sync>;

/// Empfänger der Meldungen (die [`crate::notifications::NotificationService`])
#[async_trait]
pub trait DbQuestionSink: Send + Sync {
    async fn on_question_change(&self, change: &QuestionChange) -> Result<()>;
}

/// Fragt regelmäßig die offenen eigenen Fragen ab und meldet neue Fragen und Recherchen von
/// Agenten.
pub struct DbQuestionWatch {
    questions: Query<Vec<NotifiableBoardQuestion>>,
    projects: Query<Vec<ProjectState>>,
    sink: Arc<dyn DbQuestionSink>,
    interval: Duration,
    detector: Mutex<QuestionChangeDetector<NotifiableBoardQuestion>>,
    running: AtomicBool,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl DbQuestionWatch {
    /// Beobachtung mit den Abfragen der Fragen und der Projekte; Abstand 0 schaltet sie ab
    pub fn new(
        questions: Query<Vec<NotifiableBoardQuestion>>,
        projects: Query<Vec<ProjectState>>,
        sink: Arc<dyn DbQuestionSink>,
        interval_ms: u64,
    ) -> Arc<Self> {
        Arc::new(Self {
            questions,
            projects,
            sink,
            interval: Duration::from_millis(interval_ms),
            detector: Mutex::new(QuestionChangeDetector::new()),
            running: AtomicBool::new(false),
            task: Mutex::new(None),
        })
    }

    /// Liest sofort den Bestand ein (ohne Meldung) und vergleicht danach im Takt; Abstand 0
    /// schaltet ab
    pub fn start(self: &Arc<Self>) {
        let mut slot = self.task.lock().expect("Sperre des Takts");
        if slot.is_some() || self.interval.is_zero() {
            return;
        }
        let watch = self.clone();
        *slot = Some(tokio::spawn(async move {
            let mut takt = tokio::time::interval(watch.interval);
            // Nach einem langen Durchlauf wird nicht aufgeholt, sondern der nächste Takt
            // abgewartet – wie `setInterval` in Node
            takt.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                takt.tick().await;
                watch.tick().await;
            }
        }));
    }

    /// Hält den Takt an
    pub fn stop(&self) {
        if let Some(task) = self.task.lock().expect("Sperre des Takts").take() {
            task.abort();
        }
    }

    /// Ein Durchlauf (nie zwei gleichzeitig); Fehler wie fehlende Konfig oder verlorene
    /// Verbindung werden übergangen
    pub async fn tick(&self) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let _ = self.run().await;
        self.running.store(false, Ordering::SeqCst);
    }

    /// Der Durchlauf selbst; sein Fehler endet still im Aufrufer
    async fn run(&self) -> Result<()> {
        let questions = (self.questions)().await?;
        // Erst einlesen (ohne Meldung); danach das Ziel auflösen, bevor der gemerkte Stand
        // fortgeschrieben wird: schlägt die Projektliste fehl, bleiben die Änderungen für den
        // nächsten Durchlauf erhalten
        let primed = self.detector.lock().expect("Sperre der Fragen").primed();
        let projects = match primed {
            true => (self.projects)().await?,
            false => Vec::new(),
        };
        let changes = self
            .detector
            .lock()
            .expect("Sperre der Fragen")
            .compare(questions);
        for change in changes {
            let question = change.question;
            // Eine fehlgeschlagene Meldung hält die übrigen nicht auf
            let _ = self
                .sink
                .on_question_change(&QuestionChange {
                    project_id: pid_for(&question.repo_paths, &projects),
                    project_name: format!("{} · {}", question.project_key, question.project_name),
                    kind: change.kind,
                    question,
                    route: Some("questions".to_string()),
                })
                .await;
        }
        Ok(())
    }
}

/// Harness-Projekt für die Adresse: eines, dessen Pfad einem Repo-Pfad der Boards entspricht,
/// sonst das erste erreichbare
pub fn pid_for(paths: &[String], projects: &[ProjectState]) -> String {
    let wanted: Vec<String> = paths.iter().map(|path| normalize(path)).collect();
    let reachable: Vec<&ProjectState> = projects
        .iter()
        .filter(|project| project.reachable)
        .collect();
    reachable
        .iter()
        .find(|project| wanted.contains(&normalize(&project.project.path)))
        .or(reachable.first())
        .map(|project| project.project.id.clone())
        .unwrap_or_default()
}

/// Pfad zum Vergleichen: Schrägstriche vereinheitlichen, Endtrenner weg, kleingeschrieben
fn normalize(path: &str) -> String {
    path.trim_end_matches(['\\', '/'])
        .replace('\\', "/")
        .to_lowercase()
}
