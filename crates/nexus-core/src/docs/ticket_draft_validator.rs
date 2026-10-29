//! Prüft die Antwort von Claude auf Ticket-Entwürfe: Pflichtfelder, Zeilenbereiche, Indizes
//! und Kreise; portiert aus `packages/core/src/docs/TicketDraftValidator.ts`.

use std::collections::{BTreeSet, HashMap, HashSet};

use serde_json::Value;

use crate::docs::ticket_draft::{TicketDraft, TicketDraftCheck, TicketDraftSource};
use crate::error::{HarnessError, Result};

/// Mehr Entwürfe auf einmal sind nicht mehr überschaubar
pub const MAX_DRAFTS: usize = 15;
/// Ein ganzes Dokument (etwa ein Plan mit vielen Tasks) zerfällt in mehr, dafür kleinere Tickets
pub const MAX_DOC_DRAFTS: usize = 40;

/// Das Dokument, zu dem die Entwürfe gehören: seine Zeilenzahl und wie viele Entwürfe es
/// höchstens werden dürfen (ohne Angabe [`MAX_DRAFTS`])
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DraftDoc {
    pub lines: i64,
    pub max: Option<usize>,
}

impl DraftDoc {
    /// Dokument mit dieser Zeilenzahl und der Standardgrenze
    pub fn lines(lines: i64) -> Self {
        Self { lines, max: None }
    }

    /// Dokument mit dieser Zeilenzahl und einer eigenen Grenze
    pub fn with_max(lines: i64, max: usize) -> Self {
        Self {
            lines,
            max: Some(max),
        }
    }
}

/// Prüft die Antwort von Claude auf Ticket-Entwürfe.
#[derive(Debug, Clone, Copy, Default)]
pub struct TicketDraftValidator;

impl TicketDraftValidator {
    /// Neuer Prüfer (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Liefert die brauchbaren Entwürfe (höchstens `doc.max`) und Hinweise auf Verworfenes;
    /// ohne brauchbaren Entwurf ein Fehler
    pub fn check(&self, items: &Value, doc: DraftDoc) -> Result<TicketDraftCheck> {
        let Some(items) = items.as_array() else {
            return Err(failed("Antwort ist keine Liste von Entwürfen."));
        };
        let mut dropped: Vec<String> = Vec::new();
        let max = doc.max.unwrap_or(MAX_DRAFTS);
        let kept = &items[..items.len().min(max)];
        if items.len() > max {
            dropped.push(format!(
                "Nur die ersten {max} von {} Entwürfen übernommen.",
                items.len()
            ));
        }

        // Erst die Pflichtfelder prüfen und dabei die alte Position merken
        let mut positions: HashMap<i64, usize> = HashMap::new();
        let mut drafts: Vec<TicketDraft> = Vec::new();
        let mut raw: Vec<Value> = Vec::new();
        for (index, item) in kept.iter().enumerate() {
            let title = text(item.get("title"));
            let description = text(item.get("description"));
            let acceptance = text(item.get("acceptance"));
            if title.is_empty() || description.is_empty() || acceptance.is_empty() {
                dropped.push(format!(
                    "Entwurf {} verworfen: Titel, Beschreibung oder Abnahme fehlt.",
                    index + 1
                ));
                continue;
            }
            positions.insert(index as i64, drafts.len());
            raw.push(item.get("dependsOn").cloned().unwrap_or(Value::Null));
            drafts.push(TicketDraft {
                title,
                description,
                acceptance,
                steps: list(item.get("steps")),
                depends_on: Vec::new(),
                labels: list(item.get("labels")),
                source: source(item.get("source"), doc.lines),
            });
        }
        if drafts.is_empty() {
            return Err(failed("Die Antwort enthält keinen brauchbaren Entwurf."));
        }

        link(&mut drafts, &raw, &positions, &mut dropped);
        break_cycles(&mut drafts, &mut dropped);
        Ok(TicketDraftCheck { drafts, dropped })
    }
}

/// Zeilenbereich auf das Dokument begrenzen; unbrauchbare Angaben werden zur ersten Zeile
fn source(value: Option<&Value>, lines: i64) -> TicketDraftSource {
    let heading = text(value.and_then(|found| found.get("heading")));
    let start = integer(value.and_then(|found| found.get("startLine")));
    let end = integer(value.and_then(|found| found.get("endLine")));
    match (start, end) {
        (Some(start), Some(end)) if start >= 1 && end >= start && start <= lines => {
            TicketDraftSource {
                heading,
                start_line: start,
                end_line: end.min(lines),
            }
        }
        _ => TicketDraftSource {
            heading,
            start_line: 1,
            end_line: 1,
        },
    }
}

/// `dependsOn` über die alten auf die neuen Positionen umrechnen; Unbrauchbares fällt weg
fn link(
    drafts: &mut [TicketDraft],
    raw: &[Value],
    positions: &HashMap<i64, usize>,
    dropped: &mut Vec<String>,
) {
    for (index, draft) in drafts.iter_mut().enumerate() {
        let values = raw
            .get(index)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        // Reihenfolge der Aufnahme wie ein `Set` in JavaScript: aufsteigend ist sie hier nicht,
        // deshalb merkt sich `seen` die Reihenfolge und `known` den Inhalt
        let mut seen: Vec<usize> = Vec::new();
        let mut known: HashSet<usize> = HashSet::new();
        for value in &values {
            let next = integer(Some(value)).and_then(|old| positions.get(&old).copied());
            let fresh = next.is_some_and(|next| next != index && !known.contains(&next));
            match (next, fresh) {
                (Some(next), true) => {
                    known.insert(next);
                    seen.push(next);
                }
                _ => dropped.push(format!(
                    "Abhängigkeit „{}“ bei „{}“ entfernt.",
                    js_string(value),
                    draft.title
                )),
            }
        }
        draft.depends_on = seen.into_iter().map(|next| next as i64).collect();
    }
}

/// Kanten der Reihe nach aufnehmen; jede Kante, die einen Kreis schließt, wird entfernt
fn break_cycles(drafts: &mut [TicketDraft], dropped: &mut Vec<String>) {
    let titles: Vec<String> = drafts.iter().map(|draft| draft.title.clone()).collect();
    let mut edges: HashMap<usize, BTreeSet<usize>> = (0..drafts.len())
        .map(|index| (index, BTreeSet::new()))
        .collect();
    for to in 0..drafts.len() {
        let mut keep: Vec<i64> = Vec::new();
        for from in drafts[to].depends_on.clone() {
            let from = from as usize;
            if reaches(&edges, to, from) {
                dropped.push(format!(
                    "Abhängigkeit von „{}“ zu „{}“ entfernt: schließt einen Kreis.",
                    titles[from], titles[to]
                ));
                continue;
            }
            edges.entry(from).or_default().insert(to);
            keep.push(from as i64);
        }
        drafts[to].depends_on = keep;
    }
}

/// Erreicht `from` den Knoten `to` über die schon aufgenommenen Kanten?
fn reaches(edges: &HashMap<usize, BTreeSet<usize>>, from: usize, to: usize) -> bool {
    let mut stack = vec![from];
    let mut seen: HashSet<usize> = HashSet::new();
    while let Some(node) = stack.pop() {
        if node == to {
            return true;
        }
        if !seen.insert(node) {
            continue;
        }
        if let Some(next) = edges.get(&node) {
            stack.extend(next.iter().copied());
        }
    }
    false
}

/// Ein Textfeld ohne Rand; alles, was kein Text ist, gilt als leer
fn text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .map(|text| text.trim().to_string())
        .unwrap_or_default()
}

/// Eine Liste von Texten ohne leere Einträge; alles andere ergibt eine leere Liste
fn list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| text(Some(item)))
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Ganze Zahl wie `Number.isInteger(Number(value))` in JavaScript; `None`, wenn der Wert keine
/// ganze Zahl ergibt (`undefined` wird dort zu `NaN`, `null` zu `0`)
fn integer(value: Option<&Value>) -> Option<i64> {
    let number = match value? {
        Value::Null => 0.0,
        Value::Bool(flag) => f64::from(u8::from(*flag)),
        Value::Number(number) => number.as_f64()?,
        Value::String(text) if text.trim().is_empty() => 0.0,
        Value::String(text) => text.trim().parse::<f64>().ok()?,
        _ => return None,
    };
    (number.fract() == 0.0 && number.is_finite()).then_some(number as i64)
}

/// Der Wert als Text, wie `String(value)` ihn in JavaScript schreibt (für die Hinweise)
fn js_string(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// „Die Antwort ist unbrauchbar“ – wortgleich zu TypeScript
fn failed(message: &str) -> crate::error::Error {
    HarnessError::new("analysis-failed", message).into()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const DOC: DraftDoc = DraftDoc {
        lines: 100,
        max: None,
    };

    /// Ein vollständiger Entwurf mit überschreibbaren Feldern (wie in
    /// `TicketDraftValidator.test.ts`)
    fn draft(title: &str, extra: Value) -> Value {
        let mut item = json!({
            "title": title,
            "description": format!("Beschreibung {title}"),
            "acceptance": format!("Abnahme {title}"),
            "steps": ["Schritt"],
            "dependsOn": [],
            "labels": ["server"],
            "source": { "heading": "Ziel", "startLine": 3, "endLine": 7 },
        });
        if let (Some(target), Some(extra)) = (item.as_object_mut(), extra.as_object()) {
            for (key, value) in extra {
                target.insert(key.clone(), value.clone());
            }
        }
        item
    }

    /// Die Titel der geprüften Entwürfe
    fn titles(check: &TicketDraftCheck) -> Vec<&str> {
        check
            .drafts
            .iter()
            .map(|draft| draft.title.as_str())
            .collect()
    }

    #[test]
    fn ubernimmt_eine_gultige_liste_unverandert() {
        let items = json!([
            draft("A", json!({})),
            draft("B", json!({ "dependsOn": [0] }))
        ]);

        let result = TicketDraftValidator::new().check(&items, DOC).unwrap();

        assert!(result.dropped.is_empty(), "{:?}", result.dropped);
        assert_eq!(titles(&result), ["A", "B"]);
        assert_eq!(result.drafts[1].depends_on, [0]);
        assert_eq!(
            result.drafts[0].source,
            TicketDraftSource {
                heading: "Ziel".into(),
                start_line: 3,
                end_line: 7
            }
        );
    }

    #[test]
    fn verwirft_entwurfe_ohne_pflichtfelder_und_rechnet_dependson_um() {
        let items = json!([
            draft("A", json!({})),
            draft("B", json!({ "title": "  " })),
            draft("C", json!({ "acceptance": "" })),
            draft("D", json!({ "dependsOn": [0, 3] })),
        ]);

        let result = TicketDraftValidator::new().check(&items, DOC).unwrap();

        assert_eq!(titles(&result), ["A", "D"]);
        // 0 bleibt A, der Verweis auf den verworfenen Eintrag 3 (D selbst) fällt weg
        assert_eq!(result.drafts[1].depends_on, [0]);
        assert!(result.dropped.len() >= 2, "{:?}", result.dropped);
    }

    #[test]
    fn kappt_die_liste_bei_15_entwurfen() {
        let items = Value::Array(
            (0..20)
                .map(|index| draft(&format!("T{index}"), json!({})))
                .collect(),
        );

        let result = TicketDraftValidator::new().check(&items, DOC).unwrap();

        assert_eq!(result.drafts.len(), 15);
        assert!(
            result.dropped.join(" ").contains("15"),
            "{:?}",
            result.dropped
        );
    }

    #[test]
    fn entfernt_selbstbezug_indizes_ausserhalb_und_dubletten() {
        let items = json!([
            draft("A", json!({ "dependsOn": [0, 5, -1, 1, 1] })),
            draft("B", json!({})),
        ]);

        let result = TicketDraftValidator::new().check(&items, DOC).unwrap();

        assert_eq!(result.drafts[0].depends_on, [1]);
        assert!(!result.dropped.is_empty());
    }

    #[test]
    fn bereinigt_kreise_und_behalt_alle_entwurfe() {
        let items = json!([
            draft("A", json!({ "dependsOn": [2] })),
            draft("B", json!({ "dependsOn": [0] })),
            draft("C", json!({ "dependsOn": [1] })),
        ]);

        let result = TicketDraftValidator::new().check(&items, DOC).unwrap();

        assert_eq!(titles(&result), ["A", "B", "C"]);
        let edges: usize = result
            .drafts
            .iter()
            .map(|draft| draft.depends_on.len())
            .sum();
        assert_eq!(edges, 2);
        assert!(
            result.dropped.join(" ").contains("Kreis"),
            "{:?}",
            result.dropped
        );
    }

    #[test]
    fn begrenzt_source_auf_das_dokument_und_ersetzt_unbrauchbare_angaben() {
        let items = json!([
            draft(
                "A",
                json!({ "source": { "heading": 42, "startLine": 0, "endLine": 9000 } })
            ),
            draft(
                "B",
                json!({ "source": { "heading": "Plan", "startLine": 90, "endLine": 9000 } })
            ),
        ]);

        let result = TicketDraftValidator::new().check(&items, DOC).unwrap();

        assert_eq!(
            result.drafts[0].source,
            TicketDraftSource {
                heading: String::new(),
                start_line: 1,
                end_line: 1
            }
        );
        assert_eq!(
            result.drafts[1].source,
            TicketDraftSource {
                heading: "Plan".into(),
                start_line: 90,
                end_line: 100
            }
        );
    }

    #[test]
    fn raumt_steps_und_labels_auf() {
        let items = json!([draft(
            "A",
            json!({ "steps": ["x", "", 7], "labels": "server" })
        )]);

        let result = TicketDraftValidator::new().check(&items, DOC).unwrap();

        assert_eq!(result.drafts[0].steps, ["x"]);
        assert!(result.drafts[0].labels.is_empty());
    }

    #[test]
    fn meldet_analysis_failed_ohne_liste_oder_ohne_brauchbaren_entwurf() {
        let validator = TicketDraftValidator::new();
        assert_eq!(
            validator
                .check(&json!({ "drafts": [] }), DOC)
                .unwrap_err()
                .code(),
            Some("analysis-failed")
        );
        assert_eq!(
            validator.check(&json!([]), DOC).unwrap_err().code(),
            Some("analysis-failed")
        );
        assert_eq!(
            validator
                .check(&json!([draft("A", json!({ "description": "" }))]), DOC)
                .unwrap_err()
                .code(),
            Some("analysis-failed")
        );
    }

    #[test]
    fn ubergeht_eintrage_die_keine_objekte_sind() {
        let items = json!([draft("A", json!({})), "kein Objekt", Value::Null]);

        let result = TicketDraftValidator::new().check(&items, DOC).unwrap();

        assert_eq!(titles(&result), ["A"]);
        assert_eq!(result.dropped.len(), 2, "{:?}", result.dropped);
    }

    #[test]
    fn nimmt_eine_eigene_grenze_entgegen() {
        let items = Value::Array(
            (0..20)
                .map(|index| draft(&format!("T{index}"), json!({})))
                .collect(),
        );

        let result = TicketDraftValidator::new()
            .check(&items, DraftDoc::with_max(100, MAX_DOC_DRAFTS))
            .unwrap();

        assert_eq!(result.drafts.len(), 20);
        assert!(result.dropped.is_empty());
    }
}
