//! Der Schattenbetrieb: rechnet jede Entscheidung und jede Auswertung des Node-Dispatchers
//! nach und schreibt nur den Vergleich.
//!
//! Grundsatz: außerhalb von `<Benutzerordner>/dispatch/shadow/` wird nichts geschrieben.
//! Deshalb kommt hier weder [`crate::dispatch::ticket_starter::TicketStarter`] noch
//! [`crate::dispatch::result_handler::ResultHandler`] vor, der Kontext ist
//! [`DispatchContext::read_only`], und der Dispatch-Service dahinter dient allein
//! [`DispatchService::status`] für den Bereitschafts-Vergleich.
//!
//! Angestoßen wird zweifach: ein Beobachter auf `dispatch/runs` meldet neue Lauf- und
//! Ergebnisdateien sofort, und jeder Takt gleicht denselben Ordner noch einmal ab – so
//! fehlt auch dann nichts, wenn ein Ereignis verloren geht.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::board::repo_board_resolver::RepoBoardOwner;
use nexus_core::clock::Clock;
use nexus_core::dispatch::shadow::ShadowStore;
use nexus_core::dispatch::{
    DispatchConfig, DispatchConfigStore, DispatchLog, DispatchStateStore, ReviewStore, RunRecord,
    RunStore,
};
use nexus_core::{HarnessHome, Result};

use crate::board_runtime::{BoardBackend, BoardRuntimeHolder};
use crate::dispatch::context::{DispatchContext, DispatchStores};
use crate::dispatch::notifier::NoDispatchNotify;
use crate::dispatch::scheduler::{DispatchTick, LogLine};
use crate::dispatch::service::{DispatchService, DispatchServiceDeps};
use crate::dispatch::shadow::decision::ShadowDecisionMaker;
use crate::dispatch::shadow::evaluation::{EvaluationStatus, ShadowEvaluator};
use crate::dispatch::shadow::readiness::{ShadowReadinessCheck, SidecarStatus};
use crate::dispatch::worker_launcher::NoWorkerLauncher;

/// Was der Schattenbetrieb braucht
pub struct ShadowDeps {
    pub home: HarnessHome,
    pub boards: Arc<BoardRuntimeHolder>,
    pub clock: Clock,
    /// Umgebung, die ein Lauf erben würde (für die Argumente von `claude -p`)
    pub env: BTreeMap<String, String>,
    /// Name der Plattform wie in Node (`win32`, `darwin`, `linux`)
    pub platform: String,
    /// Zustand des Node-Sidecars für den Bereitschafts-Vergleich
    pub sidecar: Arc<dyn SidecarStatus>,
    /// Schreibt in die Protokolldatei des Servers (`server.log`)
    pub log: LogLine,
}

impl std::fmt::Debug for ShadowDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShadowDeps")
            .field("platform", &self.platform)
            .field("sidecar", &self.sidecar)
            .finish_non_exhaustive()
    }
}

/// Rechnet nach, vergleicht und schreibt nur nach `dispatch/shadow/`.
pub struct ShadowService {
    deps: ShadowDeps,
    store: ShadowStore,
    runs: RunStore,
    configs: DispatchConfigStore,
    /// Nur für [`DispatchService::status`]: derselbe Code, der im Rust-Betrieb über die
    /// Bereitschaft entscheiden würde
    status: Arc<DispatchService>,
    /// Nie zwei Vergleiche gleichzeitig (Takt und Beobachter stoßen beide an)
    lock: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for ShadowService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShadowService")
            .field("deps", &self.deps)
            .finish_non_exhaustive()
    }
}

impl ShadowService {
    /// Schattenbetrieb auf diesen Abhängigkeiten
    pub fn new(deps: ShadowDeps) -> Self {
        let status = Arc::new(DispatchService::new(DispatchServiceDeps {
            clock: deps.clock.clone(),
            env: deps.env.clone(),
            platform: deps.platform.clone(),
            read_only: true,
            ..DispatchServiceDeps::new(
                deps.home.clone(),
                deps.boards.clone(),
                Arc::new(NoWorkerLauncher),
            )
        }));
        Self {
            store: ShadowStore::new(deps.home.clone()),
            runs: RunStore::new(deps.home.clone()),
            configs: DispatchConfigStore::new(deps.home.clone()),
            status,
            lock: tokio::sync::Mutex::new(()),
            deps,
        }
    }

    /// Der Ordner, in den der Schattenbetrieb schreibt
    pub fn shadow_dir(&self) -> std::path::PathBuf {
        self.store.dir()
    }

    /// Der Ordner, den der Beobachter im Auge behält (`dispatch/runs` des Node-Dispatchers)
    pub fn runs_dir(&self) -> std::path::PathBuf {
        self.deps.home.dispatch_runs_dir()
    }

    /// Ein voller Durchlauf: neue Läufe vergleichen, wartende Auswertungen nachsehen und die
    /// Bereitschaft abgleichen. Ein Fehler eines Schritts hält die anderen nicht auf
    pub async fn run_tick(&self) {
        let _guard = self.lock.lock().await;
        if let Err(error) = self.scan_runs_locked().await {
            self.log(&format!("Schattenbetrieb: Läufe – {error}"));
        }
        if let Err(error) = self.evaluate_pending().await {
            self.log(&format!("Schattenbetrieb: Auswertungen – {error}"));
        }
        if let Err(error) = self.compare_readiness().await {
            self.log(&format!("Schattenbetrieb: Bereitschaft – {error}"));
        }
    }

    /// Der Ordner `dispatch/runs` hat sich geändert: neue Läufe und Ergebnisse aufnehmen.
    /// Der Beobachter ruft nur das auf, der Rest wartet auf den Takt
    pub async fn scan_runs(&self) {
        let _guard = self.lock.lock().await;
        if let Err(error) = self.scan_runs_locked().await {
            self.log(&format!("Schattenbetrieb: Läufe – {error}"));
        }
    }

    /// Jeden Lauf des Ordners kopieren und – einmal je Lauf – seine Entscheidung vergleichen
    async fn scan_runs_locked(&self) -> Result<()> {
        let records = self.runs.list()?;
        if records.is_empty() {
            return Ok(());
        }
        let decided = self.store.decided()?;
        let mut fresh: Vec<RunRecord> = Vec::new();
        for record in records {
            // Erst kopieren: der Node-Dispatcher entfernt Lauf und Ergebnis, sobald er
            // ausgewertet hat
            self.store.keep_run(&record)?;
            if let Some(end) = self.runs.result(&record.id)? {
                self.store.keep_result(&record.id, &end)?;
            }
            if !decided.contains(&record.id) {
                fresh.push(record);
            }
        }
        if fresh.is_empty() {
            return Ok(());
        }
        let cfg = self.configs.read()?.config;
        self.with_context(cfg, |ctx| async move {
            let maker = ShadowDecisionMaker::new(&ctx);
            for record in &fresh {
                let line = maker.compare(record).await;
                self.store.append_decision(&line)?;
                self.store.mark_decided(&record.id)?;
                self.log(&format!(
                    "Schattenbetrieb: Entscheidung {} ({}) {}",
                    record.ticket_id,
                    record.kind.as_str(),
                    verdict(line.equal, line.skipped.as_deref(), line.diffs.len()),
                ));
            }
            Ok(())
        })
        .await
    }

    /// Für jeden kopierten Lauf mit Ergebnis nachsehen, ob seine Zeile in
    /// `board.DispatchRuns` steht, und sie vergleichen
    async fn evaluate_pending(&self) -> Result<()> {
        let pending = self.store.kept_results()?;
        if pending.is_empty() {
            return Ok(());
        }
        let cfg = self.configs.read()?.config;
        self.with_context(cfg, |ctx| async move {
            let evaluator = ShadowEvaluator::new(&ctx);
            for run_id in &pending {
                let (Some(record), Some(end)) = (
                    self.store.kept_run(run_id)?,
                    self.store.kept_result(run_id)?,
                ) else {
                    // Ohne Lauf-Kopie gibt es nichts nachzurechnen
                    self.store.forget_run(run_id)?;
                    continue;
                };
                let status = match evaluator.compare(&record, &end).await {
                    Ok(status) => status,
                    Err(error) => {
                        self.log(&format!(
                            "Schattenbetrieb: Auswertung {} – {error}",
                            record.ticket_id
                        ));
                        continue;
                    }
                };
                let line = match status {
                    EvaluationStatus::Waiting => continue,
                    EvaluationStatus::Compared(line) | EvaluationStatus::Missing(line) => line,
                };
                self.store.append_evaluation(&line)?;
                self.store.forget_run(run_id)?;
                self.log(&format!(
                    "Schattenbetrieb: Auswertung {} {}",
                    record.ticket_id,
                    if line.missing_row {
                        "ohne Zeile".to_string()
                    } else {
                        verdict(line.equal, None, line.diffs.len())
                    },
                ));
            }
            Ok(())
        })
        .await
    }

    /// Die Bereitschaft beider Seiten abgleichen; nur Abweichungen werden geschrieben
    async fn compare_readiness(&self) -> Result<()> {
        let lines = ShadowReadinessCheck::new(
            &self.status,
            &self.deps.boards,
            self.deps.sidecar.as_ref(),
            self.deps.clock.now(),
        )
        .compare()
        .await?;
        for line in &lines {
            self.store.append_readiness(line)?;
        }
        if !lines.is_empty() {
            self.log(&format!(
                "Schattenbetrieb: Bereitschaft – {} Abweichung(en)",
                lines.len()
            ));
        }
        Ok(())
    }

    /// Ein nur lesender Kontext des eigenen Benutzers auf dieser Einstellung
    async fn with_context<T, F, Fut>(&self, cfg: DispatchConfig, work: F) -> Result<T>
    where
        F: FnOnce(DispatchContext) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let board = BoardConfigStore::new(self.deps.home.clone()).require()?;
        let me = RepoBoardOwner {
            user_id: board.user_id.clone(),
            active_board: board.active_board,
        };
        self.deps
            .boards
            .run(|backend| async move { work(self.context(cfg, me, backend)).await })
            .await
    }

    /// Der Kontext des Vergleichs: nur lesend, ohne Starter und ohne Meldungen
    fn context(
        &self,
        cfg: DispatchConfig,
        me: RepoBoardOwner,
        backend: Arc<dyn BoardBackend>,
    ) -> DispatchContext {
        let stores = DispatchStores {
            runs: RunStore::new(self.deps.home.clone()),
            state: DispatchStateStore::with_clock(self.deps.home.clone(), self.deps.clock.clone()),
            log: DispatchLog::with_clock(self.deps.home.clone(), self.deps.clock.clone()),
            reviews: ReviewStore::new(self.deps.home.clone()),
        };
        DispatchContext::new(
            cfg,
            me,
            self.deps.home.clone(),
            backend,
            stores,
            Arc::new(NoWorkerLauncher),
            self.deps.clock.clone(),
            Arc::new(NoDispatchNotify),
            self.deps.env.clone(),
            self.deps.platform.clone(),
        )
        .read_only()
    }

    /// Zeile in der Protokolldatei des Servers
    fn log(&self, text: &str) {
        (self.deps.log)(text);
    }
}

/// Der Takt stößt den Schattenbetrieb genauso an wie sonst den Dispatcher
#[async_trait]
impl DispatchTick for ShadowService {
    async fn tick(&self) -> Result<()> {
        ShadowService::run_tick(self).await;
        Ok(())
    }
}

/// Das Urteil eines Vergleichs für die Protokollzeile
fn verdict(equal: bool, skipped: Option<&str>, diffs: usize) -> String {
    match (skipped, equal) {
        (Some(reason), _) => format!("ohne Urteil ({reason})"),
        (None, true) => "gleich".to_string(),
        (None, false) => format!("{diffs} Abweichung(en)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn das_urteil_nennt_gleich_abweichend_oder_ohne_urteil() {
        assert_eq!(verdict(true, None, 0), "gleich");
        assert_eq!(verdict(false, None, 2), "2 Abweichung(en)");
        assert_eq!(
            verdict(false, Some("Ticket gibt es nicht"), 0),
            "ohne Urteil (Ticket gibt es nicht)"
        );
    }
}
