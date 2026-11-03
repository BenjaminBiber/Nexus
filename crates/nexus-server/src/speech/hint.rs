//! Erkennungshilfe: Begriffe, die in einer Sprachnachricht zu erwarten sind.
//!
//! Dienste mit der Schnittstelle von OpenAI nehmen einen `prompt` an und schreiben die darin
//! genannten Namen danach richtig – ohne ihn wird aus „NXS-0123“ gern „N X S null eins zwei
//! drei“. Die Hilfe sammelt deshalb, was auf dem Board vorkommt: Projekt-Keys, Boardnamen, die
//! letzten Ticketnummern und die Begriffe aus `GLOSSARY.md` im Repo des aktiven Boards.
//!
//! Lang darf die Hilfe nicht werden: der Dienst zählt sie zum Kontext, und ein überlanger
//! Prompt verdrängt das Gehörte. [`MAX_HINT_CHARS`] ist die Grenze.

use std::collections::BTreeSet;
use std::path::Path;

use nexus_core::board::board_service::BoardListFilter;
use nexus_core::board::ticket_service::TicketListFilter;

use crate::board_db::ServerBoardServices;

/// Höchstlänge der Erkennungshilfe in Zeichen
pub const MAX_HINT_CHARS: usize = 800;

/// So viele Ticketnummern kommen in die Hilfe (die jüngsten)
pub const MAX_TICKETS: usize = 30;

/// Dateiname des Glossars im Repo
pub const GLOSSARY_FILE: &str = "GLOSSARY.md";

/// Was dem Dienst vor der Erkennung mitgegeben wird
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpeechHint {
    /// Begriffe, durch Komma getrennt; leer = keine Hilfe
    pub prompt: String,
    /// Sprache der Aufnahme als Kürzel; `None` = der Dienst erkennt sie selbst
    pub language: Option<String>,
}

impl SpeechHint {
    /// Hilfe nur mit der Sprache, ohne Begriffe
    pub fn language_only(language: Option<String>) -> Self {
        Self {
            prompt: String::new(),
            language,
        }
    }

    /// Hilfe aus Begriffen: Leeres und Doppeltes fällt weg, der Rest wird mit Komma verbunden
    /// und bei [`MAX_HINT_CHARS`] an der letzten ganzen Angabe abgeschnitten
    pub fn from_terms<I, S>(language: Option<String>, terms: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut seen = BTreeSet::new();
        let mut prompt = String::new();
        for term in terms {
            let term = term.as_ref().trim();
            if term.is_empty() || !seen.insert(term.to_string()) {
                continue;
            }
            let addition = match prompt.is_empty() {
                true => term.to_string(),
                false => format!(", {term}"),
            };
            if prompt.chars().count() + addition.chars().count() > MAX_HINT_CHARS {
                break;
            }
            prompt.push_str(&addition);
        }
        Self { prompt, language }
    }

    /// Sammelt die Hilfe aus dem Board: Projekt-Keys, Boardnamen, die letzten
    /// [`MAX_TICKETS`] Ticketnummern und die Begriffe aus `GLOSSARY.md` im Repo des aktiven
    /// Boards. Scheitert eine Abfrage, fehlt dieser Teil – die Erkennung soll daran nicht
    /// hängen.
    pub async fn collect(
        services: &ServerBoardServices,
        active_board: Option<i32>,
        language: Option<String>,
    ) -> Self {
        let mut terms: Vec<String> = Vec::new();
        if let Ok(projects) = services.projects.list().await {
            terms.extend(projects.into_iter().map(|project| project.key));
        }
        let boards = services
            .boards
            .list(&BoardListFilter::default())
            .await
            .unwrap_or_default();
        terms.extend(boards.iter().map(|board| board.board.name.clone()));
        let board_ids: Vec<i32> = boards.iter().map(|board| board.board.id).collect();
        if !board_ids.is_empty() {
            let filter = TicketListFilter {
                board_ids,
                without_texts: true,
                ..TicketListFilter::default()
            };
            if let Ok(tickets) = services.tickets.list(&filter).await {
                terms.extend(latest_ticket_ids(
                    tickets.iter().map(|view| view.ticket.id.as_str()),
                ));
            }
        }
        // Das Glossar gehört zum Repo des aktiven Boards; ohne aktives Board gibt es keines
        let repo = active_board.and_then(|id| {
            boards
                .iter()
                .find(|board| board.board.id == id)
                .and_then(|board| board.board.repo_path.clone())
        });
        if let Some(repo) = repo {
            terms.extend(glossary_terms(Path::new(&repo)));
        }
        Self::from_terms(language, terms)
    }
}

/// Die jüngsten [`MAX_TICKETS`] Ticketnummern: nach Nummer absteigend, Reihenfolge der Ausgabe
/// wieder aufsteigend (so liest der Prompt sich wie das Board)
fn latest_ticket_ids<'a>(ids: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut ids: Vec<&str> = ids.collect();
    // Gleiche Länge und Ziffern am Ende: die Textordnung ist hier die Nummernordnung
    ids.sort_unstable();
    ids.reverse();
    ids.truncate(MAX_TICKETS);
    ids.reverse();
    ids.into_iter().map(ToString::to_string).collect()
}

/// Die Begriffe aus `GLOSSARY.md` eines Repos: alles, was in einer Zeile als `**Begriff**:`
/// steht. Fehlt die Datei oder ist sie nicht lesbar, gibt es keine Begriffe.
pub fn glossary_terms(repo: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(repo.join(GLOSSARY_FILE)) else {
        return Vec::new();
    };
    text.lines().filter_map(glossary_term).collect()
}

/// Der Begriff einer Glossarzeile: `- **Dispatcher**: …` ergibt `Dispatcher`
fn glossary_term(line: &str) -> Option<String> {
    let (_, rest) = line.split_once("**")?;
    let (term, rest) = rest.split_once("**")?;
    // Nur die Form `**Begriff**:` zählt; fett gesetzte Wörter mitten im Text nicht
    if !rest.trim_start().starts_with(':') {
        return None;
    }
    let term = term.trim();
    (!term.is_empty()).then(|| term.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbindet_begriffe_mit_komma_und_wirft_doppeltes_weg() {
        let hint = SpeechHint::from_terms(
            Some("de".to_string()),
            ["NXS", " Nexus ", "NXS", "", "Dispatcher"],
        );
        assert_eq!(hint.prompt, "NXS, Nexus, Dispatcher");
        assert_eq!(hint.language.as_deref(), Some("de"));
    }

    #[test]
    fn schneidet_bei_800_zeichen_an_der_letzten_ganzen_angabe_ab() {
        // 100 Begriffe mit je 9 Zeichen plus Trenner: die Grenze liegt mitten in der Liste
        let terms: Vec<String> = (0..100).map(|n| format!("Begriff{n:02}")).collect();
        let hint = SpeechHint::from_terms(None, &terms);
        assert!(hint.prompt.chars().count() <= MAX_HINT_CHARS);
        // Abgeschnitten wird zwischen zwei Begriffen, nie in einem
        assert!(!hint.prompt.ends_with(','), "{}", hint.prompt);
        assert!(
            hint.prompt.starts_with("Begriff00, Begriff01"),
            "{}",
            hint.prompt
        );
        for teil in hint.prompt.split(", ") {
            assert!(terms.contains(&teil.to_string()), "halber Begriff: {teil}");
        }
    }

    #[test]
    fn hilfe_nur_mit_sprache_hat_keinen_prompt() {
        let hint = SpeechHint::language_only(Some("de".to_string()));
        assert!(hint.prompt.is_empty());
        assert_eq!(hint.language.as_deref(), Some("de"));
    }

    #[test]
    fn nimmt_die_jungsten_dreissig_ticketnummern_in_aufsteigender_folge() {
        let ids: Vec<String> = (1..=40).map(|n| format!("NXS-{n:04}")).collect();
        let latest = latest_ticket_ids(ids.iter().map(String::as_str));
        assert_eq!(latest.len(), MAX_TICKETS);
        assert_eq!(latest.first().map(String::as_str), Some("NXS-0011"));
        assert_eq!(latest.last().map(String::as_str), Some("NXS-0040"));
    }

    #[test]
    fn liest_die_fett_gesetzten_begriffe_des_glossars() {
        let dir = tempfile::tempdir().expect("Ordner");
        std::fs::write(
            dir.path().join(GLOSSARY_FILE),
            "# Glossar\n\n- **Dispatcher**: verteilt Tickets\n- **Worktree**: eigener Ordner\n\nDer **Dispatcher** steht oben.\n- ohne Begriff\n",
        )
        .expect("Glossar schreiben");

        let terms = glossary_terms(dir.path());

        assert_eq!(terms, vec!["Dispatcher", "Worktree"]);
    }

    #[test]
    fn ohne_glossar_gibt_es_keine_begriffe() {
        let dir = tempfile::tempdir().expect("Ordner");
        assert!(glossary_terms(dir.path()).is_empty());
    }
}
