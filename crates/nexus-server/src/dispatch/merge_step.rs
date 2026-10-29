//! Die Merge-Seite des Dispatchers als Schnittstelle; die Umsetzung ist
//! [`crate::dispatch::merge_service::MergeService`] (portiert aus
//! `packages/server/src/dispatch/MergeService.ts`).
//!
//! Sie steht als Trait im Kontext, damit Tests sie umhüllen können (in TypeScript tun das
//! `vi.spyOn(MergeService.prototype, …)`); [`NoMergeStep`] mergt nichts.

use async_trait::async_trait;
use nexus_core::board::basics::BoardActor;
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

/// Ergebnis von „Abnehmen“: ob danach gemergt wird und, wenn nicht, warum nicht
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptOutcome {
    pub ticket_id: String,
    /// true = der Merge ist angestoßen (eigenes Board mit Repo und Branch)
    pub merge: bool,
    /// Eine Zeile, warum nichts gemergt wird (leer, wenn gemergt wird)
    pub text: String,
}

/// Arbeitet die Merge-Aufträge eines Durchlaufs ab, wertet beendete Merge-Sessions aus und
/// führt „Erneut mergen“ und „Abnehmen“ von Hand aus.
#[async_trait]
pub trait MergeStep: MergeSessionEnd {
    /// Alle offenen Merge-Aufträge; fachliche Fehler stehen im Bericht, Verbindungsfehler
    /// brechen den Durchlauf ab
    async fn process_all(&self, ctx: &DispatchContext) -> Result<MergeStepReport>;

    /// „Erneut mergen“: setzt die Aufträge des Branches zurück; liefert die Ticket-ID
    async fn retry(&self, ctx: &DispatchContext, id: &str, actor: &BoardActor) -> Result<String>;

    /// „Abnehmen“ eines Human-Interaction-Tickets; stößt auf eigenem Board den Merge an
    async fn accept(
        &self,
        ctx: &DispatchContext,
        id: &str,
        actor: &BoardActor,
    ) -> Result<AcceptOutcome>;
}

/// Mergt nichts (für Tests, die den Merge nicht brauchen).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoMergeStep;

#[async_trait]
impl MergeSessionEnd for NoMergeStep {
    async fn session_ended(
        &self,
        _ctx: &DispatchContext,
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

    async fn retry(&self, _ctx: &DispatchContext, id: &str, _actor: &BoardActor) -> Result<String> {
        Ok(id.to_string())
    }

    async fn accept(
        &self,
        _ctx: &DispatchContext,
        id: &str,
        _actor: &BoardActor,
    ) -> Result<AcceptOutcome> {
        Ok(AcceptOutcome {
            ticket_id: id.to_string(),
            merge: false,
            text: String::new(),
        })
    }
}
