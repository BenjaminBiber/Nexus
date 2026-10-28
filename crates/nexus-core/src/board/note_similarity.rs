//! Erkennt, ob ein neuer Befund schon als Notiz festgehalten ist (gemeinsame Begriffe, ohne
//! IDs und Füllwörter). Portiert aus `packages/core/src/board/NoteSimilarity.ts`.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::board::types::Note;

/// Ab diesem Anteil gemeinsamer Begriffe (bezogen auf den längeren Text) gilt eine Notiz als
/// derselbe Befund. Am kürzeren gemessen genügten drei Allerweltsbegriffe des Bereichs
/// (Worktree, Branch, Dispatcher), damit eine kurze Notiz an jeder langen hängen blieb und ohne
/// --force abgelehnt wurde.
const THRESHOLD: f64 = 0.25;
/// Weniger gemeinsame Begriffe sind Zufall, auch bei sehr kurzen Texten
const MIN_SHARED: usize = 3;

/// Ticket- und Fragen-IDs (AIH-0022, ARC-F0001) sagen nichts über den Befund
static TICKET_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[a-z][a-z0-9]{1,9}-f?[0-9]{3,}\b").unwrap());

/// Füllwörter und Floskeln, die in fast jeder Notiz stehen
const STOP_WORDS: [&str; 58] = [
    "der",
    "die",
    "das",
    "und",
    "oder",
    "nicht",
    "von",
    "mit",
    "bei",
    "aus",
    "auf",
    "für",
    "ist",
    "sind",
    "ein",
    "eine",
    "einer",
    "eines",
    "den",
    "dem",
    "des",
    "zu",
    "im",
    "in",
    "am",
    "an",
    "als",
    "auch",
    "wenn",
    "weil",
    "dass",
    "sich",
    "nur",
    "noch",
    "schon",
    "wird",
    "werden",
    "kein",
    "keine",
    "diese",
    "dieser",
    "dieses",
    "unabhängig",
    "unabhaengig",
    "verursacht",
    "vermutlich",
    "durch",
    "nach",
    "vor",
    "über",
    "unter",
    "sehr",
    "test",
    "tests",
    "npm",
    "run",
    "the",
    "and",
];

/// Titel einer Notiz zum Anzeigen; ältere Notizen ohne Titel zeigen die erste Zeile ihres
/// Inhalts
pub fn note_title(note: &Note) -> String {
    let title = note.title.trim();
    if !title.is_empty() {
        return title.to_string();
    }
    let first = note
        .body
        .split('\n')
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    if first.is_empty() {
        format!("Notiz {}", note.id)
    } else {
        first.to_string()
    }
}

/// Vergleicht Texte von Notizen.
pub struct NoteSimilarity;

impl NoteSimilarity {
    /// Notizen, die denselben Befund beschreiben, ähnlichste zuerst
    pub fn similar(text: &str, notes: &[Note]) -> Vec<Note> {
        let words = Self::words(text);
        let mut scored: Vec<(f64, &Note)> = notes
            .iter()
            .filter_map(|note| {
                let other = Self::words(&format!("{}\n{}", note.title, note.body));
                Self::score(&words, &other).map(|score| (score, note))
            })
            .collect();
        // Stabile Sortierung wie `Array.prototype.sort` in JavaScript: gleich gute Treffer
        // behalten die Reihenfolge der Datenbank
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.into_iter().map(|(_, note)| note.clone()).collect()
    }

    /// Anteil gemeinsamer Begriffe am längeren Text; `None` unter der Schwelle
    fn score(a: &HashSet<String>, b: &HashSet<String>) -> Option<f64> {
        let shared = a.iter().filter(|word| b.contains(*word)).count();
        let larger = a.len().max(b.len());
        if shared < MIN_SHARED {
            return None;
        }
        let score = shared as f64 / larger as f64;
        (score >= THRESHOLD).then_some(score)
    }

    /// Begriffe eines Texts: klein, ab drei Zeichen, ohne IDs und Füllwörter
    fn words(text: &str) -> HashSet<String> {
        let lower = text.to_lowercase();
        let without_ids = TICKET_ID.replace_all(&lower, " ");
        without_ids
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| word.chars().count() >= 3 && !STOP_WORDS.contains(word))
            .map(str::to_string)
            .collect()
    }
}
