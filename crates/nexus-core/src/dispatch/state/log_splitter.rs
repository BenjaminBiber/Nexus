//! Zerlegt ein Lauf-Protokoll in Kopf, Arbeitsanweisung und Ausgabe, damit die Oberfläche die
//! Anweisung getrennt zeigen kann. Portiert aus
//! `packages/core/src/dispatch/state/RunLogSplitter.ts`.

use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Zeile im Lauf-Protokoll vor der Arbeitsanweisung
pub const RUN_LOG_PROMPT_MARKER: &str = "[dispatch] Arbeitsanweisung:";
/// Zeile im Lauf-Protokoll zwischen Arbeitsanweisung und Ausgabe von Claude
pub const RUN_LOG_OUTPUT_MARKER: &str = "[dispatch] Ausgabe:";

/// Teile eines Lauf-Protokolls
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct RunLogParts {
    /// Kopf des Dispatchers (Start, Profil, Aufruf)
    pub header: String,
    /// Arbeitsanweisung; `None`, wenn sie nicht (mehr) im Text steht, z. B. weil nur das Ende
    /// gelesen wurde
    pub prompt: Option<String>,
    /// Ausgabe von Claude und Ende des Laufs
    pub body: String,
}

/// Erste Ausgabe in Protokollen ohne Marker: Zeile mit Uhrzeit (stream-json), JSON (älteres
/// json-Format) oder Meldung des Dispatchers
fn first_output() -> Regex {
    Regex::new(r"^(\d\d:\d\d:\d\d {2}|\{|\[dispatch\] )").expect("festes Muster")
}

/// Zerlegt ein Lauf-Protokoll.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunLogSplitter;

impl RunLogSplitter {
    /// Neuer Zerleger (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Teile des Textes; ohne Anweisung ist alles Ausgabe
    pub fn split(&self, text: &str) -> RunLogParts {
        let lines: Vec<&str> = text.split('\n').collect();
        let clean: Vec<&str> = lines
            .iter()
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
            .collect();
        let Some(start) = clean.iter().position(|l| *l == RUN_LOG_PROMPT_MARKER) else {
            return RunLogParts {
                header: String::new(),
                prompt: None,
                body: text.to_string(),
            };
        };
        let (end, body_from) = match clean
            .iter()
            .skip(start + 1)
            .position(|l| *l == RUN_LOG_OUTPUT_MARKER)
            .map(|at| at + start + 1)
        {
            Some(end) => (end, end + 1),
            None => {
                let pattern = first_output();
                let end = clean
                    .iter()
                    .enumerate()
                    .find(|(i, l)| *i > start && pattern.is_match(l))
                    .map(|(i, _)| i)
                    .unwrap_or(clean.len());
                (end, end)
            }
        };
        RunLogParts {
            header: clean[..start].join("\n").trim_end().to_string(),
            prompt: Some(clean[start + 1..end].join("\n").trim_end().to_string()),
            body: lines
                .get(body_from..)
                .map(|rest| rest.join("\n"))
                .unwrap_or_default(),
        }
    }
}
