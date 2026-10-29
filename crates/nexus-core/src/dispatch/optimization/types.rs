//! Formen der Optimierungs-Vorschläge; portiert aus
//! `packages/core/src/dispatch/optimization/SuggestionTypes.ts`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::board::DispatchProfileReason;

/// Mindestzahl passender Läufe, bevor eine Regel urteilt
pub const MIN_RUNS: i64 = 5;

/// Herkunft eines Vorschlags: feste Regel des Advisors oder Claude-Analyse
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "dispatch.ts")]
pub enum SuggestionSource {
    Rule,
    Claude,
}

/// Regel, aus der ein Vorschlag stammt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "dispatch.ts")]
pub enum SuggestionKind {
    TurnsTooLow,
    TimeoutTooLow,
    ProfileOvertaken,
    EscalationTargetDominates,
    RuleFails,
    ProfileOversized,
    ContextFull,
    ProfileUnused,
    Claude,
}

/// Eine Änderung an der Dispatcher-Einstellung; `from` ist der wirksame Wert, auf dem der
/// Vorschlag beruht.
///
/// `from` fehlt (`None`), wenn es den Wert nicht gibt – in TypeScript steht dort `undefined`,
/// und genau dieser Unterschied zu `null` entscheidet beim Prüfen des Ausgangswerts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ConfigPatchOp {
    /// profiles.&lt;name&gt;.maxTurns|timeoutMinutes|model|effort, defaultProfile,
    /// rules.&lt;index ab 0&gt;.profile oder escalation
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "unknown")]
    pub from: Option<Value>,
    /// Neuer Wert; fehlt er, wird `null` geschrieben (die Prüfung lehnt das in der Regel ab)
    #[serde(default)]
    #[ts(type = "unknown")]
    pub to: Value,
}

/// Ziel eines Vorschlags: Profil und – bei Regeln – deren 1-basierte Nummer
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct SuggestionTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rule_index: Option<i64>,
}

/// Belege eines Vorschlags: betroffene Läufe, Treffer und Quote
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct SuggestionEvidence {
    pub runs: i64,
    pub hits: i64,
    pub rate: f64,
}

/// Ein Vorschlag zur Dispatcher-Einstellung
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct Suggestion {
    /// Kurzer Hash des Fingerabdrucks; Schlüssel für Übernehmen und Ausblenden
    pub id: String,
    /// Regel, Ziel und betroffene Werte – ändert sich mit dem Profil, damit Ausgeblendetes
    /// nach einer Änderung wiederkommt
    pub fingerprint: String,
    pub source: SuggestionSource,
    pub kind: SuggestionKind,
    pub title: String,
    pub reason: String,
    pub target: SuggestionTarget,
    /// null = nur Hinweis
    pub patch: Option<Vec<ConfigPatchOp>>,
    /// Sortierung: betroffene Läufe × Quote
    pub weight: f64,
    pub evidence: Option<SuggestionEvidence>,
}

/// Wie die Starts eines Profils zustande kamen (Grund und Regel), häufigste zuerst
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct StartSource {
    pub reason: Option<DispatchProfileReason>,
    pub rule_index: Option<i64>,
    pub count: i64,
}

/// Kennzahlen eines Profils über die Läufe mit seinen aktuellen Limits
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ProfileFacts {
    pub profile: String,
    pub max_turns: i64,
    pub timeout_minutes: i64,
    /// Läufe mit den aktuellen Limits
    pub runs: i64,
    /// davon Starts eines Tickets (ohne Eskalation, erster Versuch)
    pub starts: i64,
    /// Starts, deren Ticket danach von diesem Profil weg eskaliert wurde
    pub escalated_starts: i64,
    /// Läufe, die per Eskalation auf dieses Profil kamen
    pub via_escalation: i64,
    /// Häufigstes Basisprofil dieser Eskalationen
    pub top_origin: Option<String>,
    /// Abbruch-Kategorie → Anzahl
    #[ts(type = "Record<string, number>")]
    pub aborts: BTreeMap<String, i64>,
    /// Starts mit Ergebnis done oder review
    pub first_try_success: i64,
    /// 90. Perzentil der Schritte; null ohne Angaben
    pub turns_p90: Option<i64>,
    pub start_sources: Vec<StartSource>,
    /// Letzter Lauf mit diesem Profil (egal welche Limits); null ohne Lauf im Zeitraum
    pub last_run_at: Option<String>,
}

/// Kennzahlen einer Regel über die Starts, die sie in ihrer aktuellen Fassung ausgelöst hat
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct RuleFacts {
    /// 1-basiert wie in der Oberfläche
    pub index: i64,
    pub profile: String,
    pub runs: i64,
    /// davon eskaliert oder endgültig gescheitert
    pub failed: i64,
}

/// Ein Eskalationsweg Basisprofil → Zielprofil
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct EscalationPath {
    pub from: String,
    pub to: String,
    pub count: i64,
}

/// Grundlage der Auswertung: Zahl der Läufe, Projekte und Beginn des Zeitraums
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct OptimizationBasis {
    /// ausgewertete Arbeitsläufe
    pub runs: i64,
    pub projects: i64,
    /// Beginn des Zeitraums (ISO)
    pub since: String,
}

/// Die verdichteten Kennzahlen aller Läufe
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct OptimizationFacts {
    pub basis: OptimizationBasis,
    pub profiles: Vec<ProfileFacts>,
    pub rules: Vec<RuleFacts>,
    pub paths: Vec<EscalationPath>,
}

/// Ein genutztes Profil mit zu wenig Läufen für ein Urteil
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct InsufficientData {
    pub profile: String,
    pub runs: i64,
    pub needed: i64,
}

/// Ergebnis des Regelkatalogs: Vorschläge und Profile mit zu wenig Daten
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct AdvisorResult {
    pub suggestions: Vec<Suggestion>,
    pub insufficient: Vec<InsufficientData>,
}
