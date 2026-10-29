//! Was die Oberfläche zum Bearbeiten einer Vorlage braucht; portiert aus
//! `packages/server/src/dispatch/PromptCatalog.ts`.
//!
//! Die Inhalte kommen vollständig aus `nexus_core` – hier wird kein Vorlagentext wiederholt,
//! sonst liefe die Anzeige dem Standard hinterher.

use nexus_core::dispatch::{prompt_defaults, PromptKind, PromptPlaceholder, PromptTemplateCheck};
use serde::{Deserialize, Serialize};

/// Standardvorlage, Platzhalter und Pflichtteile einer Laufart
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchPromptInfo {
    pub kind: PromptKind,
    /// Beschriftung der Laufart für die Oberfläche
    pub label: String,
    /// Standardvorlage, die ohne eigene Vorlage gilt
    pub default: String,
    pub placeholders: Vec<PromptPlaceholder>,
    /// Beschriftungen der Pflichtteile dieser Laufart
    pub required: Vec<String>,
}

/// Beschriftung einer Laufart für die Oberfläche
fn label(kind: PromptKind) -> &'static str {
    match kind {
        PromptKind::Work => "Umsetzung",
        PromptKind::Plan => "Planung",
        PromptKind::Research => "Recherche",
        PromptKind::Merge => "Merge",
    }
}

/// Liefert je Laufart Standardvorlage, Platzhalter und Pflichtteile.
#[derive(Debug, Clone, Copy, Default)]
pub struct PromptCatalog;

impl PromptCatalog {
    /// Neuer Katalog
    pub fn new() -> Self {
        Self
    }

    /// Alle Laufarten in der Reihenfolge von [`PromptKind::ALL`]
    pub fn list(&self) -> Vec<DispatchPromptInfo> {
        let check = PromptTemplateCheck::new();
        PromptKind::ALL
            .into_iter()
            .map(|kind| DispatchPromptInfo {
                kind,
                label: label(kind).to_string(),
                default: prompt_defaults::default_template(kind).to_string(),
                placeholders: prompt_defaults::placeholders(kind),
                required: check
                    .parts(kind)
                    .into_iter()
                    .map(|part| part.label)
                    .collect(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liefert_je_laufart_standardvorlage_platzhalter_und_pflichtteile() {
        let info = PromptCatalog::new().list();
        let kinds: Vec<PromptKind> = info.iter().map(|entry| entry.kind).collect();
        assert_eq!(kinds, PromptKind::ALL.to_vec());
        for entry in &info {
            assert!(!entry.label.is_empty(), "{}", entry.kind);
            assert_eq!(entry.default, prompt_defaults::default_template(entry.kind));
            let names: Vec<&str> = entry
                .placeholders
                .iter()
                .map(|placeholder| placeholder.name.as_str())
                .collect();
            let expected: Vec<String> = prompt_defaults::placeholders(entry.kind)
                .into_iter()
                .map(|placeholder| placeholder.name)
                .collect();
            assert_eq!(names, expected, "{}", entry.kind);
            assert!(!entry.required.is_empty(), "{}", entry.kind);
        }
        let work = info
            .iter()
            .find(|entry| entry.kind == PromptKind::Work)
            .expect("work");
        assert!(work.default.contains("{{ticketId}}"));
    }
}
