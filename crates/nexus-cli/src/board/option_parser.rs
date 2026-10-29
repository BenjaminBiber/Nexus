//! Übersetzt CLI-Texte in Werte für die Board-Dienste.
//! Portiert aus `packages/cli/src/board/BoardOptionParser.ts`.

use nexus_core::board::basics::TicketStatus;
use nexus_core::board::status_parser::StatusParser;
use nexus_core::board::ticket_ordering::BoardColumn;
use nexus_core::{HarnessError, Result};

/// Schreibweisen, mit denen die abgeleitete Spalte „Wartet“ gemeint ist
const WAITING: [&str; 2] = ["wartet", "waiting"];

/// Übersetzt CLI-Texte in Werte für die Board-Dienste.
#[derive(Debug, Clone, Copy, Default)]
pub struct BoardOptionParser;

impl BoardOptionParser {
    /// Ein Status (deutsche Aliase erlaubt)
    pub fn status(&self, value: &str) -> Result<TicketStatus> {
        StatusParser::parse(value)
    }

    /// Kommagetrennte Spalten; „wartet“/„waiting“ ist hier erlaubt
    pub fn columns(&self, value: &str) -> Result<Vec<BoardColumn>> {
        value
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| {
                if WAITING.contains(&part.to_lowercase().as_str()) {
                    Ok(BoardColumn::Waiting)
                } else {
                    StatusParser::parse(part).map(BoardColumn::from)
                }
            })
            .collect()
    }

    /// Kommagetrennte Liste oder `None`
    pub fn list(&self, value: Option<&str>) -> Option<Vec<String>> {
        value.map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(ToString::to_string)
                .collect()
        })
    }

    /// Voraussetzungen; „none“/„keine“ leert die Liste
    pub fn depends_on(&self, value: &str) -> Vec<String> {
        if ["none", "keine"].contains(&value.trim().to_lowercase().as_str()) {
            return Vec::new();
        }
        self.list(Some(value)).unwrap_or_default()
    }

    /// true/false (alles außer „false“ gilt als true, wie im alten Board)
    pub fn bool(&self, value: &str) -> bool {
        value.trim().to_lowercase() != "false"
    }

    /// Erwartete Version
    pub fn rev(&self, value: Option<&str>) -> Result<Option<i32>> {
        let Some(value) = value else {
            return Ok(None);
        };
        match value.parse::<i32>() {
            Ok(rev) if rev >= 1 => Ok(Some(rev)),
            _ => Err(HarnessError::new(
                "rev-invalid",
                format!("„{value}“ ist keine Versionsnummer."),
            )
            .into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liest_einen_status_mit_deutschem_alias() {
        assert_eq!(
            BoardOptionParser.status("in Arbeit").unwrap(),
            TicketStatus::InProgress
        );
    }

    #[test]
    fn liest_spalten_kommagetrennt_und_kennt_wartet() {
        assert_eq!(
            BoardOptionParser
                .columns("geplant, wartet ,WAITING")
                .unwrap(),
            [
                BoardColumn::Planned,
                BoardColumn::Waiting,
                BoardColumn::Waiting
            ]
        );
    }

    #[test]
    fn eine_unbekannte_spalte_wird_mit_status_invalid_abgelehnt() {
        let error = BoardOptionParser.columns("geplant,quatsch").unwrap_err();
        assert_eq!(error.code(), Some("status-invalid"));
    }

    #[test]
    fn eine_liste_ohne_wert_bleibt_leer() {
        assert_eq!(BoardOptionParser.list(None), None);
        assert_eq!(
            BoardOptionParser.list(Some("a, b ,,c")),
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn none_und_keine_leeren_die_voraussetzungen() {
        assert!(BoardOptionParser.depends_on(" None ").is_empty());
        assert!(BoardOptionParser.depends_on("keine").is_empty());
        assert_eq!(
            BoardOptionParser.depends_on("ARC-0001, ARC-0002"),
            ["ARC-0001", "ARC-0002"]
        );
    }

    #[test]
    fn alles_ausser_false_gilt_als_true() {
        assert!(BoardOptionParser.bool("true"));
        assert!(BoardOptionParser.bool("ja"));
        assert!(!BoardOptionParser.bool(" FALSE "));
    }

    #[test]
    fn eine_versionsnummer_muss_eine_ganze_zahl_ab_eins_sein() {
        assert_eq!(BoardOptionParser.rev(None).unwrap(), None);
        assert_eq!(BoardOptionParser.rev(Some("7")).unwrap(), Some(7));
        for raw in ["0", "-1", "1.5", "x", ""] {
            let error = BoardOptionParser.rev(Some(raw)).unwrap_err();
            assert_eq!(error.code(), Some("rev-invalid"), "{raw}");
            assert_eq!(
                error.to_string(),
                format!("„{raw}“ ist keine Versionsnummer.")
            );
        }
    }
}
