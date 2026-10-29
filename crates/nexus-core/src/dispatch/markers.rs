//! Vorsilben der Kommentare, die der Dispatcher an Tickets und Fragen schreibt; portiert aus
//! `DISPATCH_MARKERS` in `packages/core/src/dispatch/DispatchConfig.ts`.
//!
//! Der Dispatcher zählt an diesen Kommentaren seine Versuche und die Auswertung erkennt sie
//! daran wieder – die Texte sind Vertrag und dürfen sich nicht ändern.

/// Ein Lauf wurde an einen Agenten übergeben
pub const START: &str = "[dispatch] Start";
/// Ein Lauf wurde abgebrochen
pub const ABORT: &str = "[dispatch] Abbruch";
/// Der Dispatcher pausiert
pub const PAUSE: &str = "[dispatch] Pause";
/// Der Lauf endete mit einer Frage an den Nutzer
pub const QUESTION: &str = "[dispatch] Frage";
/// Der Branch des Tickets wurde gemergt
pub const MERGED: &str = "[dispatch] Gemergt";
/// Der Merge scheiterte an einem Konflikt
pub const MERGE_CONFLICT: &str = "[dispatch] Merge-Konflikt";
/// Der Merge wartet noch
pub const MERGE_WAIT: &str = "[dispatch] Merge wartet";
/// Allgemeine Meldung rund um den Merge
pub const MERGE: &str = "[dispatch] Merge";
/// Einrichtung eines neuen Worktrees fehlgeschlagen (zählt nicht als Versuch)
pub const SETUP: &str = "[dispatch] Einrichtung";
/// Kommentar an der Frage, wenn ein Recherche-Lauf keine Recherche hinterlassen hat
pub const RESEARCH_EMPTY: &str = "[dispatch] Recherche ohne Ergebnis";
/// Kommentar am Ticket, wenn ein Planungslauf weder einen Plan noch eine Frage hinterlassen hat
pub const PLAN_EMPTY: &str = "[dispatch] Planung ohne Ergebnis";
/// Ticket wartet nach dem Lauf auf Freigabe vor dem Merge
pub const REVIEW: &str = "[dispatch] Review";
/// Mensch hat das Ticket aus dem Review zurück an den Agenten gegeben (zählt nicht als Versuch)
pub const REVIEW_RETURN: &str = "[dispatch] Zurück an den Agenten";

/// Alle Vorsilben in der Reihenfolge von TypeScript
pub const DISPATCH_MARKERS: [&str; 13] = [
    START,
    ABORT,
    PAUSE,
    QUESTION,
    MERGED,
    MERGE_CONFLICT,
    MERGE_WAIT,
    MERGE,
    SETUP,
    RESEARCH_EMPTY,
    PLAN_EMPTY,
    REVIEW,
    REVIEW_RETURN,
];

/// Gemeinsame Vorsilbe aller Dispatcher-Kommentare; die Auswertung sucht danach
pub const DISPATCH_PREFIX: &str = "[dispatch] ";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vorsilben_sind_wortgleich_zu_typescript() {
        assert_eq!(START, "[dispatch] Start");
        assert_eq!(ABORT, "[dispatch] Abbruch");
        assert_eq!(MERGE_CONFLICT, "[dispatch] Merge-Konflikt");
        assert_eq!(RESEARCH_EMPTY, "[dispatch] Recherche ohne Ergebnis");
        assert_eq!(REVIEW_RETURN, "[dispatch] Zurück an den Agenten");
    }

    #[test]
    fn jede_vorsilbe_tragt_die_gemeinsame_kennung() {
        assert_eq!(DISPATCH_MARKERS.len(), 13);
        for marker in DISPATCH_MARKERS {
            assert!(marker.starts_with(DISPATCH_PREFIX), "{marker}");
        }
    }

    #[test]
    fn merge_steht_nach_seinen_genaueren_fassungen() {
        // „[dispatch] Merge“ ist Vorsilbe von „Merge-Konflikt“ und „Merge wartet“: wer der Reihe
        // nach prüft, muss die genaueren zuerst sehen – deshalb diese Reihenfolge
        let merge = DISPATCH_MARKERS.iter().position(|m| *m == MERGE).unwrap();
        let konflikt = DISPATCH_MARKERS
            .iter()
            .position(|m| *m == MERGE_CONFLICT)
            .unwrap();
        let wartet = DISPATCH_MARKERS
            .iter()
            .position(|m| *m == MERGE_WAIT)
            .unwrap();
        assert!(merge > konflikt && merge > wartet);
    }
}
