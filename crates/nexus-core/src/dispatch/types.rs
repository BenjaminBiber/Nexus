//! Gemeinsame Typen der Lauf-Dateien: das Ergebnis eines Worker-Laufs (`RunResult` aus
//! `packages/core/src/dispatch/RunOutputReader.ts`) und ein Testlauf des Agenten (`TestRun`
//! aus `packages/core/src/dispatch/TestRunExtractor.ts`). `RunEnd` und `ReviewEntry`
//! brauchen sie, deshalb stehen sie hier und nicht beim Auswerten der Ausgabe.

use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

/// Liest ein Feld, das es in TypeScript sowohl gar nicht als auch mit dem Wert `null` gibt
/// (`feld?: T | null`). Ohne diese Hilfe würde serde ein geschriebenes `null` wie ein fehlendes
/// Feld lesen – beim Zurückschreiben wäre die Datei dann eine andere als die von Node.
pub(crate) fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// Gescheiterter Einrichtungsbefehl eines neuen Worktrees
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct SetupFailure {
    pub cwd: String,
    pub run: String,
    pub code: Option<i64>,
    pub timed_out: bool,
    pub output: String,
}

/// Ergebnis eines Worker-Laufs
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "dispatch.ts")]
pub struct RunResult {
    /// Exit-Code von Claude; null, wenn der Prozess beendet wurde
    pub code: Option<i64>,
    pub timed_out: bool,
    /// stdout (Ereignisse von `--output-format stream-json`, zuletzt das Ergebnis), notfalls samt stderr
    pub output: String,
    /// Einrichtung des Worktrees: true = alle Befehle gelaufen (fehlt ohne Einrichtung)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub setup_ok: Option<bool>,
    /// Einrichtung gescheitert: dieser Befehl; Claude wurde dann nicht gestartet
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub setup_failed: Option<SetupFailure>,
}

/// Ein Testlauf des Agenten, wie das Review ihn zeigt; `ok: null` = Ausgang unklar
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "dispatch.ts")]
pub struct TestRun {
    pub command: String,
    pub ok: Option<bool>,
    pub output: String,
}
