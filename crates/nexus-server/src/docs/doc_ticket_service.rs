//! Legt geprüfte Ticket-Entwürfe eines Dokuments der Reihe nach als Tickets auf dem Board des
//! Projekts an und nennt die Tickets, die aus einem Dokument entstanden sind; portiert aus
//! `packages/server/src/docs/DocTicketService.ts`.
//!
//! Ein einzelner Entwurf, der nicht durchkommt (Dublette, ungültige Felder), wird
//! übersprungen statt den Rest zu verwerfen.

use std::collections::HashMap;

use nexus_core::board::basics::{TicketStatus, MAX_SPEC_LENGTH};
use nexus_core::board::ticket_ordering::BoardColumn;
use nexus_core::board::ticket_service::{NewTicketInput, TicketListFilter, WriteOptions};
use nexus_core::board::types::TicketStep;
use nexus_core::docs::markdown_sections::split_lines;
use nexus_core::docs::ticket_draft_validator::DraftDoc;
use nexus_core::docs::{TicketDraft, TicketDraftValidator};
use nexus_core::text::utf16_len;
use nexus_core::{HarnessError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::board_runtime::BoardRuntimeHolder;
use crate::docs::doc_ticket_source::{format_source, parse_source};
use crate::docs::docs_root_resolver::DocsSource;
use crate::docs::docs_service::DocsService;

/// Der Körper der Anlege-Route
#[derive(Debug, Clone, PartialEq)]
pub struct DocTicketRequest {
    pub path: Option<String>,
    /// Die rohen Entwürfe aus dem Körper; jeder wird einzeln geprüft
    pub drafts: Vec<Value>,
}

/// Ticket eines Dokuments samt Stelle, aus der es entstanden ist; ohne Quellenzeile bleibt
/// `heading` leer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocTicketRef {
    pub id: String,
    pub title: String,
    pub status: TicketStatus,
    pub column: BoardColumn,
    pub heading: Option<String>,
    pub start_line: i64,
    pub end_line: i64,
}

/// Angelegtes Ticket samt Platz des Entwurfs in der Eingabe
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedDocTicket {
    pub index: usize,
    pub id: String,
}

/// Übersprungener Entwurf; bei einer Dublette zeigt `existingId` auf das vorhandene Ticket
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedDocTicket {
    pub index: usize,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_id: Option<String>,
}

/// Ergebnis des Anlegens
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocTicketResult {
    pub created: Vec<CreatedDocTicket>,
    pub skipped: Vec<SkippedDocTicket>,
}

/// Legt Tickets aus Dokument-Entwürfen an und liest sie wieder aus.
#[derive(Debug, Clone, Copy, Default)]
pub struct DocTicketService {
    validator: TicketDraftValidator,
}

impl DocTicketService {
    /// Neuer Dienst
    pub fn new() -> Self {
        Self::default()
    }

    /// Anlegen in Eingabereihenfolge: die IDs der Vorgänger werden für `dependsOn` gebraucht
    pub async fn create(
        &self,
        holder: &BoardRuntimeHolder,
        docs: &DocsService,
        source: &DocsSource,
        input: &DocTicketRequest,
    ) -> Result<DocTicketResult> {
        if input.drafts.is_empty() {
            return Err(HarnessError::new(
                "invalid-input",
                "Bitte mindestens einen Entwurf übergeben.",
            )
            .into());
        }
        let board_id = board_id(source)?;
        // wirft doc-not-found, wenn es den Pfad nicht gibt
        let doc = docs.read(source, input.path.as_deref())?;
        let doc_path = doc.path.clone();
        let lines = split_lines(&doc.content).len() as i64;

        let mut created: Vec<CreatedDocTicket> = Vec::new();
        let mut skipped: Vec<SkippedDocTicket> = Vec::new();
        // 0-basierter Index des Entwurfs → ID des angelegten oder vorhandenen Tickets
        let mut ids: HashMap<i64, String> = HashMap::new();
        for (index, raw) in input.drafts.iter().enumerate() {
            let Some(draft) = self.valid(raw, lines) else {
                skipped.push(SkippedDocTicket {
                    index,
                    reason: "draft-invalid".into(),
                    existing_id: None,
                });
                continue;
            };
            let new = ticket_input(&draft, &doc_path, &ids);
            let result = holder
                .run(move |b| async move {
                    b.services()
                        .tickets
                        .create(board_id, &new, &b.actor(None), &WriteOptions::default())
                        .await
                })
                .await;
            match result {
                Ok(ticket) => {
                    ids.insert(index as i64, ticket.ticket.id.clone());
                    created.push(CreatedDocTicket {
                        index,
                        id: ticket.ticket.id,
                    });
                }
                // Nur Fachfehler des einzelnen Entwurfs werden übersprungen; ohne
                // Board-Verbindung oder bei einem Programmfehler scheiterten auch alle
                // weiteren, das soll als Fehler ankommen statt als Antwort voller
                // übersprungener Entwürfe
                Err(error)
                    if error.harness().is_some()
                        && !BoardRuntimeHolder::is_connection_error(&error) =>
                {
                    let harness = error.harness().expect("erwartbarer Fehler");
                    let existing_id = existing_id(harness);
                    if let Some(existing) = &existing_id {
                        ids.insert(index as i64, existing.clone());
                    }
                    skipped.push(SkippedDocTicket {
                        index,
                        reason: harness.code.clone(),
                        existing_id,
                    });
                }
                Err(error) => return Err(error),
            }
        }
        Ok(DocTicketResult { created, skipped })
    }

    /// Tickets, die aus diesem Dokument entstanden sind, mit der Stelle aus ihrer Quellenzeile.
    ///
    /// Ohne Board oder Repo-Pfad bleibt die Liste leer statt zu werfen: die Chips sind
    /// Beiwerk, das Dokument bleibt lesbar.
    pub async fn list_for(
        &self,
        holder: &BoardRuntimeHolder,
        docs: &DocsService,
        source: &DocsSource,
        path: Option<&str>,
    ) -> Result<Vec<DocTicketRef>> {
        let (Some(board_id), Some(_)) = (source.board_id, source.root.as_deref()) else {
            return Ok(Vec::new());
        };
        // wirft doc-not-found, wenn es den Pfad nicht gibt
        let doc_path = docs.assert_document(source, path)?;
        let spec = short_spec(&doc_path);
        let tickets = holder
            .run(move |b| async move {
                b.services()
                    .tickets
                    .list(&TicketListFilter {
                        board_ids: vec![board_id],
                        ..TicketListFilter::default()
                    })
                    .await
            })
            .await?;
        let mut refs: Vec<DocTicketRef> = tickets
            .into_iter()
            .filter(|view| view.ticket.spec.as_deref() == Some(spec.as_str()))
            .map(|view| {
                let found = parse_source(&view.ticket.description);
                DocTicketRef {
                    id: view.ticket.id,
                    title: view.ticket.title,
                    status: view.ticket.status,
                    column: view.column,
                    heading: found.as_ref().map(|found| found.heading.clone()),
                    start_line: found.as_ref().map(|found| found.start_line).unwrap_or(0),
                    end_line: found.as_ref().map(|found| found.end_line).unwrap_or(0),
                }
            })
            .collect();
        refs.sort_by(|a, b| {
            a.start_line
                .cmp(&b.start_line)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(refs)
    }

    /// Prüft einen Entwurf wie die Antwort von Claude (Pflichtfelder, Zeilenbereich); `None`,
    /// wenn er unbrauchbar ist.
    ///
    /// `depends_on` bleibt als Liste von Eingabe-Indizes stehen, Verweise nach vorn oder ins
    /// Leere fallen später in [`depends_on`] weg.
    fn valid(&self, raw: &Value, lines: i64) -> Option<TicketDraft> {
        if !raw.is_object() {
            return None;
        }
        let check = self
            .validator
            .check(&Value::Array(vec![raw.clone()]), DraftDoc::lines(lines))
            .ok()?;
        let draft = check.drafts.into_iter().next()?;
        let depends_on = raw
            .get("dependsOn")
            .and_then(Value::as_array)
            // Nur echte ganze Zahlen zählen (wie `Number.isInteger` in TypeScript)
            .map(|items| items.iter().filter_map(Value::as_i64).collect())
            .unwrap_or_default();
        Some(TicketDraft {
            depends_on,
            ..draft
        })
    }
}

/// Board des Projektordners; ohne Board oder ohne dessen Repo-Pfad lässt sich nichts anlegen
fn board_id(source: &DocsSource) -> Result<i32> {
    let Some(board_id) = source.board_id else {
        return Err(HarnessError::new(
            "no-board",
            "Zu diesem Ordner gibt es kein Board; Tickets lassen sich erst nach dem Verknüpfen anlegen.",
        )
        .into());
    };
    if source.root.is_none() {
        return Err(HarnessError::new(
            "no-repo-path",
            format!(
                "Das Board „{}“ hat keinen erreichbaren Repo-Pfad; ohne ihn gibt es keine Dokumente, aus denen Tickets entstehen.",
                source.board_name.as_deref().unwrap_or_default()
            ),
        )
        .into());
    }
    Ok(board_id)
}

/// Entwurf in die Ticket-Form: Backlog, Planungsbedarf, Spec = Dokumentpfad, Quelle am Ende
/// der Beschreibung
fn ticket_input(draft: &TicketDraft, doc_path: &str, ids: &HashMap<i64, String>) -> NewTicketInput {
    NewTicketInput {
        title: draft.title.clone(),
        description: Some(description(draft, doc_path)),
        status: Some(TicketStatus::Backlog),
        needs_planning: Some(true),
        labels: Some(draft.labels.clone()),
        acceptance: Some(draft.acceptance.clone()),
        steps: Some(
            draft
                .steps
                .iter()
                .map(|text| TicketStep {
                    text: text.clone(),
                    done: false,
                    detail: None,
                })
                .collect(),
        ),
        depends_on: Some(depends_on(draft, ids)),
        // Ticket.spec ist auf 200 Zeichen begrenzt; ein längerer Pfad wird hinten
        // abgeschnitten, damit der Dateiname bleibt
        spec: Some(short_spec(doc_path)),
        ..NewTicketInput::default()
    }
}

/// Beschreibung mit Quellenzeile; sie nennt Dokument, Überschrift und Zeilenbereich der
/// markierten Stelle
fn description(draft: &TicketDraft, doc_path: &str) -> String {
    let source = &draft.source;
    format!(
        "{}\n\n{}",
        draft.description.trim_end(),
        format_source(
            doc_path,
            &source.heading,
            source.start_line,
            source.end_line
        )
    )
}

/// Indizes in die bis hierher bekannten IDs übersetzen; Verweise auf Unbekanntes (etwa nach
/// vorn) fallen weg
fn depends_on(draft: &TicketDraft, ids: &HashMap<i64, String>) -> Vec<String> {
    draft
        .depends_on
        .iter()
        .filter_map(|index| ids.get(index).cloned())
        .collect()
}

/// Der Dokumentpfad als Spec; zu lange Pfade werden hinten abgeschnitten (wie in TypeScript
/// nach UTF-16-Einheiten gezählt)
fn short_spec(doc_path: &str) -> String {
    if utf16_len(doc_path) <= MAX_SPEC_LENGTH {
        return doc_path.to_string();
    }
    let units: Vec<u16> = doc_path.encode_utf16().collect();
    String::from_utf16_lossy(&units[units.len() - MAX_SPEC_LENGTH..])
}

/// ID des vorhandenen Tickets aus einer Dublette
fn existing_id(error: &HarnessError) -> Option<String> {
    if error.code != "ticket-duplicate" {
        return None;
    }
    error
        .details
        .as_ref()?
        .get("existingId")?
        .as_str()
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use nexus_core::docs::TicketDraftSource;
    use serde_json::json;

    use super::*;

    const SPEC: &str = "docs/superpowers/specs/2026-10-22-docs-rework-design.md";

    /// Ein Entwurf mit Quelle
    fn draft() -> TicketDraft {
        TicketDraft {
            title: "A".into(),
            description: "Beschreibung A\n".into(),
            acceptance: "Abnahme A".into(),
            steps: vec!["Test schreiben".into(), "Umsetzen".into()],
            depends_on: vec![0, 7],
            labels: vec!["server".into()],
            source: TicketDraftSource {
                heading: "Ziel".into(),
                start_line: 3,
                end_line: 5,
            },
        }
    }

    #[test]
    fn hangt_die_quelle_an_die_beschreibung() {
        assert_eq!(
            description(&draft(), SPEC),
            format!("Beschreibung A\n\nQuelle: {SPEC} › Ziel (Z. 3–5)")
        );
    }

    #[test]
    fn ubersetzt_nur_bekannte_indizes_in_ids() {
        let ids = HashMap::from([(0, "ARC-0001".to_string())]);
        assert_eq!(depends_on(&draft(), &ids), ["ARC-0001"]);
        assert!(depends_on(&draft(), &HashMap::new()).is_empty());
    }

    #[test]
    fn schneidet_einen_zu_langen_pfad_hinten_ab() {
        let long = format!("docs/superpowers/specs/{}.md", "a".repeat(300));
        let spec = short_spec(&long);
        assert_eq!(utf16_len(&spec), MAX_SPEC_LENGTH);
        assert!(long.ends_with(&spec));
        assert_eq!(short_spec(SPEC), SPEC);
    }

    #[test]
    fn setzt_status_planungsbedarf_labels_schritte_abnahme_und_spec() {
        let input = ticket_input(&draft(), SPEC, &HashMap::new());

        assert_eq!(input.status, Some(TicketStatus::Backlog));
        assert_eq!(input.needs_planning, Some(true));
        assert_eq!(
            input.labels.as_deref(),
            Some(["server".to_string()].as_slice())
        );
        assert_eq!(input.acceptance.as_deref(), Some("Abnahme A"));
        assert_eq!(input.spec.as_deref(), Some(SPEC));
        assert_eq!(
            input
                .steps
                .unwrap_or_default()
                .iter()
                .map(|step| step.text.clone())
                .collect::<Vec<_>>(),
            ["Test schreiben", "Umsetzen"]
        );
    }

    #[test]
    fn pruft_jeden_entwurf_einzeln_und_behalt_die_eingabe_indizes() {
        let service = DocTicketService::new();
        let raw = json!({
            "title": "A", "description": "B", "acceptance": "C",
            "steps": [], "dependsOn": [3, "x", 1.5], "labels": [],
            "source": { "heading": "Ziel", "startLine": 3, "endLine": 5 },
        });

        let draft = service.valid(&raw, 100).expect("gültiger Entwurf");

        assert_eq!(draft.depends_on, [3]);
        assert_eq!(draft.source.start_line, 3);
        assert_eq!(service.valid(&json!("kein Objekt"), 100), None);
        assert_eq!(service.valid(&json!({ "title": "" }), 100), None);
    }

    #[test]
    fn meldet_ein_board_ohne_repo_pfad_mit_einem_hinweis() {
        let without_board = DocsSource::no_board();
        assert_eq!(
            board_id(&without_board).unwrap_err().code(),
            Some("no-board")
        );

        let without_path = DocsSource {
            root: None,
            board_id: Some(7),
            board_name: Some("Sprint".into()),
            reason: None,
        };
        let error = board_id(&without_path).unwrap_err();
        assert_eq!(error.code(), Some("no-repo-path"));
        assert!(error.to_string().contains("Sprint"), "{error}");

        let full = DocsSource {
            root: Some("C:/repo".into()),
            board_id: Some(7),
            board_name: Some("Sprint".into()),
            reason: None,
        };
        assert_eq!(board_id(&full).unwrap(), 7);
    }

    #[test]
    fn liest_die_id_eines_vorhandenen_tickets_aus_der_dublette() {
        let duplicate = HarnessError::new("ticket-duplicate", "schon da")
            .with_details(json!({ "existingId": "ARC-0001" }));
        assert_eq!(existing_id(&duplicate).as_deref(), Some("ARC-0001"));
        assert_eq!(
            existing_id(&HarnessError::new("ticket-duplicate", "schon da")),
            None
        );
        assert_eq!(
            existing_id(&HarnessError::new("title-missing", "fehlt")),
            None
        );
    }
}
