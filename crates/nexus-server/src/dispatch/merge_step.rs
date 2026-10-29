//! Der Merge-Schritt eines Durchlaufs als Schnittstelle; die Umsetzung bringt P4-15b
//! (`packages/server/src/dispatch/MergeService.ts`).
//!
//! Bis dahin tut [`NoMergeStep`] nichts: der Durchlauf legt Merge-Aufträge an (das macht die
//! Auswertung der Läufe), arbeitet sie aber noch nicht ab.

use async_trait::async_trait;
use nexus_core::Result;

use crate::dispatch::context::DispatchContext;
use crate::dispatch::result_handler::MergeSessionEnd;

/// Was ein Merge-Schritt dem Bericht des Durchlaufs beisteuert
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeStepReport {
    /// Gemergte Tickets
    pub merged: Vec<String>,
    /// Fachliche Fehler je Merge-Auftrag; Verbindungsfehler brechen den Durchlauf ab
    pub errors: Vec<String>,
}

/// Arbeitet die Merge-Aufträge eines Durchlaufs ab und wertet beendete Merge-Sessions aus.
#[async_trait]
pub trait MergeStep: MergeSessionEnd {
    /// Alle offenen Merge-Aufträge; fachliche Fehler stehen im Bericht, Verbindungsfehler
    /// brechen den Durchlauf ab
    async fn process_all(&self, ctx: &DispatchContext) -> Result<MergeStepReport>;
}

/// Mergt nichts (Standard, bis P4-15b den Merge anbindet).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoMergeStep;

#[async_trait]
impl MergeSessionEnd for NoMergeStep {
    async fn session_ended(
        &self,
        _record: &nexus_core::dispatch::RunRecord,
        _end: &nexus_core::dispatch::RunEnd,
    ) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl MergeStep for NoMergeStep {
    async fn process_all(&self, _ctx: &DispatchContext) -> Result<MergeStepReport> {
        Ok(MergeStepReport::default())
    }
}
