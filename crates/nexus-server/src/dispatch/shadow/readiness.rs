//! Vergleicht je Takt, wie Node und Rust die Bereitschaft der Geplant-Tickets des aktiven
//! Boards beurteilen.
//!
//! Die Node-Seite kommt aus `GET /api/dispatch/status` des Sidecars, die Rust-Seite aus
//! [`DispatchService::status`] – also genau aus dem Code, der im Rust-Betrieb entscheiden
//! würde. Beide Seiten sehen die Lage zu leicht verschiedenen Zeitpunkten; hat sich das
//! Ticket kurz vorher geändert, wird die Abweichung als zeitnah markiert statt als echter
//! Unterschied gelesen.

use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::Duration;
use nexus_core::board::ticket_service::TicketListFilter;
use nexus_core::clock::parse_millis;
use nexus_core::dispatch::shadow::ShadowReadiness;
use nexus_core::{HarnessError, Result};
use serde_json::Value;

use crate::board_runtime::BoardRuntimeHolder;
use crate::dispatch::service::DispatchService;

/// So kurz vor dem Vergleich zählt eine Änderung des Tickets als Grund für eine Abweichung
pub const TIMING_MINUTES: i64 = 2;

/// Der Zustand des Node-Sidecars; im Betrieb über HTTP, in Tests eine feste Antwort
#[async_trait]
pub trait SidecarStatus: std::fmt::Debug + Send + Sync {
    /// `GET /api/dispatch/status` des Sidecars
    async fn status(&self) -> Result<Value>;
}

/// Holt den Zustand des Sidecars über HTTP.
#[derive(Debug, Clone)]
pub struct HttpSidecarStatus {
    base: String,
    client: reqwest::Client,
}

impl HttpSidecarStatus {
    /// Zustandsquelle auf der Adresse des Sidecars (`http://127.0.0.1:4711`)
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SidecarStatus for HttpSidecarStatus {
    async fn status(&self) -> Result<Value> {
        let url = format!("{}/api/dispatch/status", self.base.trim_end_matches('/'));
        let answer = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|error| failed(&url, &error.to_string()))?;
        if !answer.status().is_success() {
            return Err(failed(&url, &format!("HTTP {}", answer.status())));
        }
        answer
            .json()
            .await
            .map_err(|error| failed(&url, &error.to_string()))
    }
}

/// Der Sidecar hat nicht geantwortet
fn failed(url: &str, detail: &str) -> nexus_core::Error {
    HarnessError::new(
        "dispatch-sidecar-unreachable",
        format!("Der Node-Server hat {url} nicht beantwortet ({detail})."),
    )
    .into()
}

/// Vergleicht die Bereitschaft beider Seiten.
pub struct ShadowReadinessCheck<'a> {
    service: &'a DispatchService,
    boards: &'a BoardRuntimeHolder,
    sidecar: &'a dyn SidecarStatus,
    now: chrono::DateTime<chrono::Utc>,
}

impl<'a> ShadowReadinessCheck<'a> {
    /// Vergleich auf diesem (nur lesenden) Dispatch-Service
    pub fn new(
        service: &'a DispatchService,
        boards: &'a BoardRuntimeHolder,
        sidecar: &'a dyn SidecarStatus,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            service,
            boards,
            sidecar,
            now,
        }
    }

    /// Die Abweichungen dieses Takts; gleiche Urteile kommen nicht in den Bericht
    pub async fn compare(&self) -> Result<Vec<ShadowReadiness>> {
        let mine = self.service.status(None).await?;
        let theirs = self.sidecar.status().await?;
        let rust: BTreeMap<String, String> = mine
            .tickets
            .iter()
            .map(|row| (row.id.clone(), row.reason.clone()))
            .collect();
        let node = rows_of(&theirs);
        if rust == node {
            return Ok(Vec::new());
        }
        let changed = self.changed_recently(mine.board_id).await;
        let at = nexus_core::clock::iso(self.now);
        let mut lines = Vec::new();
        for id in node.keys().chain(rust.keys()).cloned().collect::<Vec<_>>() {
            let (left, right) = (node.get(&id), rust.get(&id));
            if left == right {
                continue;
            }
            if lines
                .iter()
                .any(|line: &ShadowReadiness| line.ticket_id == id)
            {
                continue;
            }
            lines.push(ShadowReadiness {
                at: at.clone(),
                ticket_id: id.clone(),
                node: left.cloned(),
                rust: right.cloned(),
                timing: changed.contains(&id),
            });
        }
        Ok(lines)
    }

    /// Tickets des Boards, die sich gerade erst geändert haben; ohne Board oder bei einem
    /// Lesefehler bleibt die Menge leer (dann gilt keine Abweichung als zeitnah)
    async fn changed_recently(&self, board_id: Option<i32>) -> Vec<String> {
        let Some(board_id) = board_id else {
            return Vec::new();
        };
        let since = self.now - Duration::minutes(TIMING_MINUTES);
        let views = self
            .boards
            .run(|backend| async move {
                backend
                    .services()
                    .tickets
                    .list(&TicketListFilter {
                        board_ids: vec![board_id],
                        without_texts: true,
                        ..TicketListFilter::default()
                    })
                    .await
            })
            .await;
        let Ok(views) = views else {
            return Vec::new();
        };
        views
            .into_iter()
            .filter(|view| {
                parse_millis(&view.ticket.updated_at)
                    .is_some_and(|millis| millis >= since.timestamp_millis())
            })
            .map(|view| view.ticket.id)
            .collect()
    }
}

/// Ticket-ID → Urteil aus der Antwort des Sidecars; unerwartete Formen ergeben nichts
fn rows_of(status: &Value) -> BTreeMap<String, String> {
    status
        .get("tickets")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let id = row.get("id")?.as_str()?.to_string();
                    let reason = row.get("reason")?.as_str().unwrap_or_default().to_string();
                    Some((id, reason))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn liest_die_urteile_aus_der_antwort_des_sidecars() {
        let status = json!({
            "tickets": [
                { "id": "ARC-0001", "title": "Eins", "reason": "bereit" },
                { "id": "ARC-0002", "title": "Zwei", "reason": "wartet auf ARC-0001" },
            ]
        });
        let rows = rows_of(&status);
        assert_eq!(rows.get("ARC-0001").map(String::as_str), Some("bereit"));
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn eine_antwort_ohne_tickets_ergibt_nichts() {
        assert!(rows_of(&json!({})).is_empty());
        assert!(rows_of(&json!({ "tickets": "kaputt" })).is_empty());
    }
}
