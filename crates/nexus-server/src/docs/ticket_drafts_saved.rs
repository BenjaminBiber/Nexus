//! Die gespeicherten Ticket-Entwürfe eines Dokuments: lesen, bearbeitet speichern, in der
//! Docs-Liste markieren und verwerfen. Portiert aus den Claude-freien Teilen von
//! `packages/server/src/docs/TicketDraftService.ts` (`saved`, `update`, `flag`, `discard`);
//! das Erzeugen über Claude gehört [`crate::docs::TicketDraftService`].

use std::path::{Path, PathBuf};

use nexus_core::clock::Clock;
use nexus_core::docs::markdown_sections::split_lines;
use nexus_core::docs::ticket_draft_validator::DraftDoc;
use nexus_core::docs::{TicketDraftValidator, MAX_DOC_DRAFTS};
use nexus_core::{HarnessError, HarnessHome, Result};
use serde_json::Value;

use crate::docs::doc_draft_store::{DocDraftStore, TicketDraftResult};
use crate::docs::docs_root_resolver::DocsSource;
use crate::docs::docs_service::{DocsListDto, DocsService};

/// Liest, speichert und verwirft die Ticket-Entwürfe eines Dokuments.
#[derive(Debug)]
pub struct SavedTicketDrafts {
    store: DocDraftStore,
    validator: TicketDraftValidator,
    clock: Clock,
}

impl SavedTicketDrafts {
    /// Dienst über dem Entwurfs-Speicher des Benutzerordners
    pub fn new(home: HarnessHome) -> Self {
        Self::with_clock(home, Clock::system())
    }

    /// Wie [`SavedTicketDrafts::new`], mit eigener Uhr (Tests)
    pub fn with_clock(home: HarnessHome, clock: Clock) -> Self {
        Self {
            store: DocDraftStore::new(home),
            validator: TicketDraftValidator::new(),
            clock,
        }
    }

    /// Zuletzt erzeugte Entwürfe zum Dokument; `None`, wenn keine gespeichert sind
    pub fn saved(
        &self,
        docs: &DocsService,
        source: &DocsSource,
        path: Option<&str>,
    ) -> Result<Option<TicketDraftResult>> {
        Ok(self.store.get(&docs.full_path(source, path)?))
    }

    /// Merkt sich die eben erzeugten Entwürfe zum Dokument (ruft
    /// [`crate::docs::TicketDraftService`] nach dem Lauf von Claude)
    pub async fn put(
        &self,
        docs: &DocsService,
        source: &DocsSource,
        path: Option<&str>,
        result: &TicketDraftResult,
    ) -> Result<()> {
        self.store.put(&docs.full_path(source, path)?, result).await
    }

    /// Speichert die im Dialog bearbeiteten Entwürfe (geprüft wie Claudes Antwort); Kosten und
    /// Zeitpunkt der Erzeugung bleiben
    pub async fn update(
        &self,
        docs: &DocsService,
        source: &DocsSource,
        path: Option<&str>,
        drafts: &Value,
    ) -> Result<TicketDraftResult> {
        if !drafts.is_array() {
            return Err(HarnessError::new("invalid-input", "Entwürfe fehlen.").into());
        }
        let doc = docs.read(source, path)?;
        let full = docs.full_path(source, path)?;
        let before = self.store.get(&full);
        let lines = split_lines(&doc.content).len() as i64;
        let check = self
            .validator
            .check(drafts, DraftDoc::with_max(lines, MAX_DOC_DRAFTS))?;
        let saved = TicketDraftResult {
            drafts: check.drafts,
            dropped: before
                .as_ref()
                .map(|before| before.dropped.clone())
                .unwrap_or_default(),
            cost_usd: before.as_ref().and_then(|before| before.cost_usd),
            at: before
                .as_ref()
                .map(|before| before.at.clone())
                .unwrap_or_else(|| self.clock.iso()),
        };
        self.store.put(&full, &saved).await?;
        Ok(saved)
    }

    /// Markiert in der Docs-Liste die Dateien, zu denen Entwürfe gespeichert sind
    /// (Filter „Entwürfe vorhanden“)
    pub fn flag(&self, list: DocsListDto) -> DocsListDto {
        let Some(root) = list.source.root.clone() else {
            return list;
        };
        if list.docs.is_empty() {
            return list;
        }
        let full: Vec<PathBuf> = list
            .docs
            .iter()
            .map(|doc| Path::new(&root).join(&doc.path))
            .collect();
        let saved = self.store.saved_among(&full);
        if saved.is_empty() {
            return list;
        }
        let mut list = list;
        for (doc, full) in list.docs.iter_mut().zip(full) {
            if saved.contains(&full) {
                doc.has_drafts = Some(true);
            }
        }
        list
    }

    /// Verwirft die gespeicherten Entwürfe zum Dokument
    pub async fn discard(
        &self,
        docs: &DocsService,
        source: &DocsSource,
        path: Option<&str>,
    ) -> Result<()> {
        self.store.remove(&docs.full_path(source, path)?).await
    }
}
