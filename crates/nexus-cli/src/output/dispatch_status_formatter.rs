//! Lesbare Zeilen für `harness board dispatch status`, `board merge`, `board plan-start` und
//! `board question research-start`.
//! Portiert aus `packages/cli/src/board/DispatchStatusFormatter.ts`; die Sichten stammen aus
//! `DispatchClient.ts`.

use serde::Deserialize;
use serde_json::Value;

/// Ein laufender Worker; bei einer Recherche steht die Frage in `question_id`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchRunView {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub ticket_id: String,
    #[serde(default)]
    pub question_id: Option<String>,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub log: String,
}

/// Zustand einer Automatik (Recherchen bzw. Planungen); ältere Server liefern das Feld nicht
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoView {
    #[serde(default)]
    pub auto: bool,
    #[serde(default)]
    pub wip: i64,
    #[serde(default)]
    pub active: i64,
    #[serde(default)]
    pub max_per_day: Option<i64>,
    #[serde(default)]
    pub today: Option<i64>,
    #[serde(default)]
    pub retry_at: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Ein anstehender Merge
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeView {
    #[serde(default)]
    pub ticket_id: String,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub target: Option<String>,
}

/// Ein Ticket auf Geplant samt Grund seiner Bereitschaft
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketView {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub reason: String,
}

/// Antwort von `/api/dispatch/status` (wie `DispatchStatus` im Server)
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchStatusView {
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub runs: Vec<DispatchRunView>,
    #[serde(default)]
    pub research: Option<AutoView>,
    #[serde(default)]
    pub planning_auto: Option<AutoView>,
    #[serde(default)]
    pub merges: Vec<MergeView>,
    #[serde(default)]
    pub board_id: Option<i64>,
    #[serde(default)]
    pub board_in_scope: bool,
    #[serde(default)]
    pub tickets: Vec<TicketView>,
    #[serde(default)]
    pub tickets_error: Option<String>,
    #[serde(default)]
    pub last_errors: Vec<String>,
    #[serde(default)]
    pub config_file: String,
    #[serde(default)]
    pub config_error: Option<String>,
    /// Ergebnis von „Erneut mergen“ bzw. „Abnehmen“ (ältere Server liefern es nicht)
    #[serde(default)]
    pub result: Option<MergeResult>,
    /// Der gestartete Lauf von „Recherche starten“ bzw. „Planen“
    #[serde(default)]
    pub started: Option<DispatchRunView>,
}

/// Ergebnis für das Ticket bei „Erneut mergen“ und „Abnehmen“
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeResult {
    #[serde(default)]
    pub text: String,
}

/// Text der Phase eines Merges
fn phase_text(phase: &str) -> &str {
    match phase {
        "pending" => "Merge steht an",
        "waiting" => "wartet auf weitere Tickets",
        "session" => "Merge-Session läuft",
        other => other,
    }
}

/// „Standard“, wenn der Server kein Modell bzw. keinen Effort nennt
fn oder_standard(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("Standard")
}

/// Lesbare Zeilen für die Dispatcher-Befehle.
#[derive(Debug, Clone, Copy, Default)]
pub struct DispatchStatusFormatter;

impl DispatchStatusFormatter {
    /// Liest die Antwort des Servers; unbekannte Felder bleiben unberührt (sie gehen nur in `--json`)
    pub fn view(json: &Value) -> DispatchStatusView {
        serde_json::from_value(json.clone()).unwrap_or_default()
    }

    /// Ergebnis von `harness board merge <id>` für dieses Ticket; ohne Ergebnis (älterer Server)
    /// die Übersicht
    pub fn merge_lines(&self, view: &DispatchStatusView) -> Vec<String> {
        match &view.result {
            Some(result) => vec![result.text.clone()],
            None => self.lines(view),
        }
    }

    /// Ergebnis von `harness board question research-start <id>`
    pub fn research_lines(&self, view: &DispatchStatusView) -> Vec<String> {
        let Some(run) = &view.started else {
            return self.lines(view);
        };
        vec![
            format!(
                "Recherche {} gestartet – Profil {} ({}/{}), Protokoll {}.",
                run.question_id.as_deref().unwrap_or(&run.ticket_id),
                run.profile,
                oder_standard(&run.model),
                oder_standard(&run.effort),
                run.log
            ),
            "Das Ergebnis erscheint als Recherche an der Frage; ohne Ergebnis schreibt der Dispatcher einen Kommentar.".to_string(),
        ]
    }

    /// Ausgabe von `harness board plan-start`
    pub fn plan_lines(&self, view: &DispatchStatusView) -> Vec<String> {
        let Some(run) = &view.started else {
            return self.lines(view);
        };
        vec![
            format!(
                "Planung {} gestartet – Profil {} ({}/{}), Protokoll {}.",
                run.ticket_id,
                run.profile,
                oder_standard(&run.model),
                oder_standard(&run.effort),
                run.log
            ),
            "Der Plan erscheint am Ticket (danach Geplant); fehlt eine Entscheidung, legt der Agent eine Frage an; ohne Ergebnis schreibt der Dispatcher einen Kommentar.".to_string(),
        ]
    }

    /// Art eines Laufs für die Übersicht, z. B. „Recherche ARC-F0007“
    pub fn run_label(&self, run: &DispatchRunView) -> String {
        match run.kind.as_str() {
            "research" => format!(
                "Recherche {}",
                run.question_id.as_deref().unwrap_or(&run.ticket_id)
            ),
            "plan" => format!("{}  Planung", run.ticket_id),
            "merge" => format!("{}  Merge-Session", run.ticket_id),
            _ => format!("{}  Umsetzung", run.ticket_id),
        }
    }

    /// Zustand, laufende Worker, Merges, Bereitschaft, Fehler, Einstellungsdatei
    pub fn lines(&self, status: &DispatchStatusView) -> Vec<String> {
        let mut out = vec![format!("Dispatcher: {}", status.message)];
        if let Some(error) = &status.config_error {
            out.push(format!("Einstellung fehlerhaft: {error}"));
        }
        out.push("Läufe:".to_string());
        if status.runs.is_empty() {
            out.push("  keine".to_string());
        }
        for run in &status.runs {
            out.push(format!(
                "  {}, Profil {} ({}/{}), Protokoll {}",
                self.run_label(run),
                run.profile,
                oder_standard(&run.model),
                oder_standard(&run.effort),
                run.log
            ));
        }
        if let Some(research) = &status.research {
            let day = match research.max_per_day {
                Some(max) => format!(
                    ", heute {}/{max} automatisch",
                    research.today.unwrap_or_default()
                ),
                None => String::new(),
            };
            out.push(format!(
                "Recherchen: {}/{} laufen, Automatik {}{day}",
                research.active,
                research.wip,
                an_oder_aus(research.auto)
            ));
            Self::push_auto_error(&mut out, research);
        }
        if let Some(planning) = &status.planning_auto {
            out.push(format!(
                "Planungen: {}/{} laufen, Automatik {}",
                planning.active,
                planning.wip,
                an_oder_aus(planning.auto)
            ));
            Self::push_auto_error(&mut out, planning);
        }
        if !status.merges.is_empty() {
            out.push("Merges:".to_string());
            for merge in &status.merges {
                let target = match &merge.target {
                    Some(target) => format!(" → {target}"),
                    None => String::new(),
                };
                out.push(format!(
                    "  {}  {} ({}{target})",
                    merge.ticket_id,
                    phase_text(&merge.phase),
                    merge.branch
                ));
            }
        }
        if let Some(board_id) = status.board_id {
            if !status.board_in_scope {
                out.push(format!(
                    "Board {board_id} gehört nicht zu den Boards des Dispatchers."
                ));
            }
            if let Some(error) = &status.tickets_error {
                out.push(format!("Tickets nicht lesbar: {error}"));
            } else if status.tickets.is_empty() {
                out.push("Keine Tickets auf Geplant.".to_string());
            } else {
                out.push(format!("Geplant (Board {board_id}):"));
                for ticket in &status.tickets {
                    out.push(format!("  {}  {}", ticket.id, ticket.reason));
                }
            }
        }
        if !status.last_errors.is_empty() {
            out.push("Fehler beim letzten Durchlauf:".to_string());
            for error in &status.last_errors {
                out.push(format!("  {error}"));
            }
        }
        out.push(format!("Einstellung: {}", status.config_file));
        out
    }

    /// Fehler einer Automatik samt Neuversuch
    fn push_auto_error(out: &mut Vec<String>, auto: &AutoView) {
        let Some(error) = &auto.error else { return };
        let retry = match &auto.retry_at {
            Some(at) => format!(" (Neuversuch ab {at})"),
            None => String::new(),
        };
        out.push(format!("  Automatik-Fehler{retry}: {error}"));
    }
}

/// „an“ oder „aus“
fn an_oder_aus(on: bool) -> &'static str {
    if on {
        "an"
    } else {
        "aus"
    }
}

#[cfg(test)]
mod tests;
