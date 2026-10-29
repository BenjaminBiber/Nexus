//! Optimierungs-Vorschläge zur Dispatcher-Einstellung: berechnen (Läufe aller Projekte),
//! übernehmen, ausblenden und das Claude-Ergebnis merken. Portiert aus
//! `packages/server/src/dispatch/DispatchSuggestionService.ts`.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use nexus_core::clock::Clock;
use nexus_core::dispatch::optimization::{
    AdvisorResult, ConfigPatcher, InsufficientData, OptimizationAdvisor, OptimizationBasis,
    OptimizationFacts, OptimizationFactsCollector, StoredAnalysis, Suggestion,
    SuggestionStateStore,
};
use nexus_core::dispatch::{
    DispatchConfigFingerprint, DispatchConfigRead, DispatchConfigStore, FactsInput,
};
use nexus_core::{HarnessError, HarnessHome, Result};
use serde::Serialize;
use serde_json::Value;

use crate::board_runtime::BoardRuntimeHolder;
use crate::log::ServerLog;

/// Ausgewertet werden die Läufe der letzten 180 Tage
const RUN_WINDOW_DAYS: i64 = 180;

/// Antwort von `GET /api/dispatch/suggestions`
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionListDto {
    /// Hash der Einstellung, auf der die Liste beruht – Pflicht beim Übernehmen
    pub config_hash: String,
    pub basis: OptimizationBasis,
    pub suggestions: Vec<Suggestion>,
    pub dismissed: Vec<Suggestion>,
    pub insufficient: Vec<InsufficientData>,
    pub analysis: Option<AnalysisDto>,
}

/// Kurzangaben zur letzten Claude-Analyse
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisDto {
    pub at: String,
    pub cost_usd: Option<f64>,
    /// true = die Analyse gilt einer älteren Einstellung
    pub stale: bool,
    pub count: i64,
    pub skipped: i64,
}

/// Grundlage einer Auswertung: Einstellung, Kennzahlen und Regel-Vorschläge
#[derive(Debug, Clone)]
pub struct SuggestionContext {
    pub read: DispatchConfigRead,
    pub config_hash: String,
    pub facts: OptimizationFacts,
    pub advice: AdvisorResult,
}

/// Die Optimierungs-Vorschläge zur Dispatcher-Einstellung.
pub struct DispatchSuggestionService {
    board: Arc<BoardRuntimeHolder>,
    store: DispatchConfigStore,
    state: SuggestionStateStore,
    patcher: ConfigPatcher,
    clock: Clock,
}

impl DispatchSuggestionService {
    /// Dienst zum Benutzerordner; Warnungen zur Datei gehen ins Protokoll des Servers
    pub fn new(board: Arc<BoardRuntimeHolder>, home: HarnessHome, clock: Clock) -> Self {
        let log = ServerLog::new(&home);
        Self {
            board,
            store: DispatchConfigStore::new(home.clone()),
            state: SuggestionStateStore::with_warn(home, move |message| log.write(message)),
            patcher: ConfigPatcher::new(),
            clock,
        }
    }

    /// Pfad der Einstellung (für die Sicht der Oberfläche)
    pub fn config_file(&self) -> String {
        self.store.file().display().to_string()
    }

    /// Die Liste der Vorschläge
    pub async fn list(&self) -> Result<SuggestionListDto> {
        let context = self.context().await?;
        self.compose(&context)
    }

    /// Liest Einstellung, Läufe und Konfig-Fassungen und wendet den Regelkatalog an; eine
    /// kaputte Einstellung meldet `dispatch-config-invalid`
    pub async fn context(&self) -> Result<SuggestionContext> {
        let read = self.store.read()?;
        let now = self.clock.now();
        let since = now - Duration::days(RUN_WINDOW_DAYS);
        let until = now + Duration::minutes(1);
        let (runs, versions) = self
            .board
            .run(|backend| async move {
                let runs = backend
                    .services()
                    .dispatch_runs
                    .list_all(since, until)
                    .await?;
                let versions = backend
                    .services()
                    .dispatch_runs
                    .config_versions(&iso(DateTime::UNIX_EPOCH), &iso(until))
                    .await?;
                Ok((runs, versions))
            })
            .await?;
        let facts = OptimizationFactsCollector::new().collect(&FactsInput {
            runs: &runs,
            config: &read.config,
            versions: &versions,
            since: iso(since),
        });
        let advice = OptimizationAdvisor::new().advise(&facts, &read.config, now);
        Ok(SuggestionContext {
            config_hash: DispatchConfigFingerprint::new().hash(&read.config),
            read,
            facts,
            advice,
        })
    }

    /// Übernimmt die Änderung eines Vorschlags und liefert die neue rohe Einstellung samt
    /// Liste; geänderte Einstellung → `conflict` (`config-changed`), unbekannte ID →
    /// `suggestion-not-found`
    pub async fn apply(&self, id: &str, config_hash: Option<&Value>) -> Result<ApplyResult> {
        require_id(id)?;
        let config_hash = match config_hash.and_then(Value::as_str) {
            Some(hash) if !hash.is_empty() => hash.to_string(),
            _ => {
                return Err(HarnessError::new("input-invalid", "„configHash“ fehlt.").into());
            }
        };
        let context = self.context().await?;
        if context.config_hash != config_hash {
            return Err(HarnessError::new(
                "conflict",
                "Die Dispatcher-Einstellung wurde inzwischen geändert – die Vorschläge wurden neu berechnet.",
            )
            .with_details(serde_json::json!({ "reason": "config-changed" }))
            .into());
        }
        let list = self.compose(&context)?;
        let found = list
            .suggestions
            .iter()
            .chain(list.dismissed.iter())
            .find(|suggestion| suggestion.id == id)
            .ok_or_else(|| {
                HarnessError::new(
                    "suggestion-not-found",
                    "Diesen Vorschlag gibt es nicht mehr.",
                )
            })?;
        let patch = found.patch.as_ref().ok_or_else(|| {
            HarnessError::new(
                "input-invalid",
                "Dieser Vorschlag ist nur ein Hinweis und ändert nichts.",
            )
        })?;
        let next = self
            .patcher
            .apply(&context.read.raw, &context.read.config, patch)?;
        self.store.write(&next)?;
        Ok(ApplyResult {
            read: self.store.read()?,
            list: self.list().await?,
        })
    }

    /// Blendet einen Vorschlag aus (`hidden`) oder wieder ein
    pub async fn dismiss(&self, id: &str, hidden: bool) -> Result<SuggestionListDto> {
        require_id(id)?;
        let id = id.to_string();
        self.state.update(|mut state| {
            match hidden {
                // Eine schon ausgeblendete ID behält ihren Platz (wie `new Set` in TypeScript)
                true if !state.dismissed.contains(&id) => {
                    state.dismissed.push(id.clone());
                }
                true => {}
                false => state.dismissed.retain(|other| *other != id),
            }
            state
        })?;
        self.list().await
    }

    /// Merkt das Ergebnis einer Claude-Analyse
    pub fn save_analysis(&self, analysis: StoredAnalysis) -> Result<()> {
        self.state.update(|mut state| {
            state.analysis = Some(analysis.clone());
            state
        })?;
        Ok(())
    }

    /// Regel- und Claude-Vorschläge zusammen; Claude-Vorschläge zu einer älteren Einstellung
    /// bleiben nur als Hinweis
    fn compose(&self, context: &SuggestionContext) -> Result<SuggestionListDto> {
        let state = self.state.read();
        let analysis = state.analysis.clone();
        let stale = analysis
            .as_ref()
            .is_some_and(|analysis| analysis.config_hash != context.config_hash);
        let claude: Vec<Suggestion> = analysis
            .as_ref()
            .map(|analysis| analysis.suggestions.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|mut suggestion| {
                if stale {
                    suggestion.patch = None;
                }
                suggestion
            })
            .collect();
        let mut all = context.advice.suggestions.clone();
        all.extend(claude);
        let all = OptimizationAdvisor::sort(all);
        let hidden = |suggestion: &Suggestion| state.dismissed.contains(&suggestion.id);
        Ok(SuggestionListDto {
            config_hash: context.config_hash.clone(),
            basis: context.facts.basis.clone(),
            suggestions: all.iter().filter(|s| !hidden(s)).cloned().collect(),
            dismissed: all.iter().filter(|s| hidden(s)).cloned().collect(),
            insufficient: context.advice.insufficient.clone(),
            analysis: analysis.map(|analysis| AnalysisDto {
                at: analysis.at,
                cost_usd: analysis.cost_usd,
                stale,
                count: analysis.suggestions.len() as i64,
                skipped: analysis.skipped.unwrap_or(0),
            }),
        })
    }
}

/// Ergebnis des Übernehmens: die neue Einstellung und die neu berechnete Liste
#[derive(Debug, Clone)]
pub struct ApplyResult {
    pub read: DispatchConfigRead,
    pub list: SuggestionListDto,
}

/// Eine Vorschlags-ID sind 16 Hex-Zeichen
fn require_id(id: &str) -> Result<()> {
    let valid = id.len() == 16
        && id
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
    match valid {
        true => Ok(()),
        false => Err(HarnessError::new(
            "input-invalid",
            format!("{id} ist keine gültige Vorschlags-ID."),
        )
        .into()),
    }
}

/// Zeitpunkt in ISO-Form mit Millisekunden, wie `new Date().toISOString()`
fn iso(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lehnt_ids_ab_die_keine_16_hex_zeichen_sind() {
        assert!(require_id("abcdef0123456789").is_ok());
        for id in ["kaputt!", "ABCDEF0123456789", "abc", ""] {
            let error = require_id(id).unwrap_err();
            assert_eq!(error.code(), Some("input-invalid"));
        }
    }
}
