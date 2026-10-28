//! Packt das Ergebnis von `claude -p --output-format json` aus; portiert aus
//! `packages/server/src/dispatch/ClaudeResultEnvelope.ts`.
//!
//! Gemeinsam genutzt von der Optimierungs-Analyse, dem Umschreiben eines Dokument-Bereichs
//! und den Ticket-Entwürfen.

use nexus_core::HarnessError;

/// So viele Zeichen einer Ausgabe stehen in einer Fehlermeldung
pub const EXCERPT_CHARS: usize = 300;

/// Antworttext von Claude und die Kosten des Laufs
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeResultText {
    pub text: String,
    pub cost_usd: Option<f64>,
}

/// Liest die letzte Zeile mit `type: "result"` aus der Ausgabe von Claude.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeResultEnvelope;

impl ClaudeResultEnvelope {
    /// Neuer Auswerter (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Antworttext und Kosten; fehlerhafte oder unlesbare Antworten werden zu `analysis-failed`
    pub fn parse(&self, output: &str) -> Result<ClaudeResultText, HarnessError> {
        for line in output.trim().lines().rev() {
            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if parsed.get("type").and_then(|t| t.as_str()) != Some("result") {
                continue;
            }
            let subtype = parsed.get("subtype").and_then(|s| s.as_str());
            let is_error = parsed
                .get("is_error")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let text = parsed.get("result").and_then(|r| r.as_str());
            match text {
                Some(text) if !is_error && subtype == Some("success") => {
                    return Ok(ClaudeResultText {
                        text: text.to_string(),
                        cost_usd: parsed.get("total_cost_usd").and_then(|c| c.as_f64()),
                    })
                }
                _ => {
                    return Err(HarnessError::new(
                        "analysis-failed",
                        format!(
                            "Claude meldet einen Fehler ({}).",
                            subtype.unwrap_or("unbekannt")
                        ),
                    ))
                }
            }
        }
        Err(HarnessError::new(
            "analysis-failed",
            format!("Antwort nicht lesbar: {}", excerpt(output)),
        ))
    }
}

/// Kürzt lange Ausgaben für Fehlermeldungen, damit der Client nicht ein ganzes Protokoll sieht
pub fn excerpt(text: &str) -> String {
    let shortened = if text.chars().count() > EXCERPT_CHARS {
        let end = text
            .char_indices()
            .nth(EXCERPT_CHARS)
            .map(|(index, _)| index)
            .unwrap_or(text.len());
        format!("{} …", &text[..end])
    } else {
        text.to_string()
    };
    shortened.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liest_die_letzte_ergebniszeile() {
        let output = "{\"type\":\"system\"}\n{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"fertig\",\"total_cost_usd\":0.25}\n";
        let parsed = ClaudeResultEnvelope::new().parse(output).expect("Ergebnis");
        assert_eq!(parsed.text, "fertig");
        assert_eq!(parsed.cost_usd, Some(0.25));
    }

    #[test]
    fn ohne_kosten_bleibt_der_preis_leer() {
        let output =
            "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"ok\"}";
        assert_eq!(
            ClaudeResultEnvelope::new()
                .parse(output)
                .expect("Ergebnis")
                .cost_usd,
            None
        );
    }

    #[test]
    fn ein_fehler_von_claude_wird_zu_analysis_failed() {
        let output = "{\"type\":\"result\",\"subtype\":\"error_max_turns\",\"is_error\":true,\"result\":\"\"}";
        let error = ClaudeResultEnvelope::new()
            .parse(output)
            .expect_err("Fehler");
        assert_eq!(error.code, "analysis-failed");
        assert_eq!(
            error.message,
            "Claude meldet einen Fehler (error_max_turns)."
        );
    }

    #[test]
    fn unlesbare_antworten_nennen_den_anfang_der_ausgabe() {
        let error = ClaudeResultEnvelope::new()
            .parse("kein JSON")
            .expect_err("Fehler");
        assert_eq!(error.code, "analysis-failed");
        assert_eq!(error.message, "Antwort nicht lesbar: kein JSON");
    }

    #[test]
    fn lange_ausgaben_werden_gekuerzt() {
        let long = "x".repeat(400);
        let error = ClaudeResultEnvelope::new()
            .parse(&long)
            .expect_err("Fehler");
        assert!(error.message.ends_with('…'));
        assert!(error.message.len() < long.len());
    }
}
