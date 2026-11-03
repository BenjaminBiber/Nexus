//! Die Texte des Bot-Profils: angezeigter Name, Kurzbeschreibung und Beschreibung.
//!
//! Sie stehen hier und nicht im Abgleich ([`super::profile`]), damit ein Test sie prüfen kann,
//! ohne einen Bot zu brauchen. Die Beschreibung entsteht aus der Befehlsliste des Routers,
//! damit im Chatfenster dasselbe steht wie in `/hilfe`.

/// Angezeigter Name des Bots
pub const NAME: &str = "Nexus";

/// Kurzbeschreibung: steht im Profil des Bots
pub const SHORT_DESCRIPTION: &str = "Nexus-Board unterwegs: Ideen per Sprache, Freigaben, Status";

/// Einleitung der Beschreibung; danach folgen die Befehle
const INTRO: &str = "Dein Board unterwegs: Ideen per Sprache erfassen, Läufe freigeben und den Stand abfragen.\n\nBefehle:";

/// Höchstlänge des Namens in Zeichen (Bot-API)
pub const NAME_MAX: usize = 64;

/// Höchstlänge der Kurzbeschreibung in Zeichen (Bot-API)
pub const SHORT_DESCRIPTION_MAX: usize = 120;

/// Höchstlänge der Beschreibung in Zeichen (Bot-API)
pub const DESCRIPTION_MAX: usize = 512;

/// Die Beschreibung aus Einleitung und Befehlsliste.
///
/// Telegram lehnt eine zu lange Beschreibung ab; melden spätere Tickets so viele Befehle an,
/// dass es nicht mehr passt, bleiben die letzten weg – lieber eine Zeile weniger als eine
/// abgeschnittene.
pub fn description(commands: &[(String, String)]) -> String {
    let mut text = String::from(INTRO);
    let mut laenge = text.chars().count();
    for (name, description) in commands {
        let line = format!("\n/{name} – {description}");
        let zeichen = line.chars().count();
        if laenge + zeichen > DESCRIPTION_MAX {
            break;
        }
        text.push_str(&line);
        laenge += zeichen;
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eine Befehlsliste wie die des Routers
    fn commands(count: usize) -> Vec<(String, String)> {
        (0..count)
            .map(|nummer| {
                (
                    format!("befehl{nummer}"),
                    "eine Beschreibung mittlerer Länge".to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn nennt_in_der_beschreibung_die_befehle() {
        let text = description(&[("status".into(), "Dispatcher und Läufe".into())]);

        assert!(text.starts_with("Dein Board unterwegs"), "{text}");
        assert!(text.contains("/status – Dispatcher und Läufe"), "{text}");
    }

    #[test]
    fn halt_die_hochstlangen_der_bot_api_ein() {
        assert!(NAME.chars().count() <= NAME_MAX);
        assert!(SHORT_DESCRIPTION.chars().count() <= SHORT_DESCRIPTION_MAX);
        assert!(description(&commands(100)).chars().count() <= DESCRIPTION_MAX);
    }

    #[test]
    fn lasst_zu_viele_befehle_ganz_weg_statt_sie_abzuschneiden() {
        let text = description(&commands(100));

        assert!(text.lines().last().unwrap().ends_with("Länge"), "{text}");
    }
}
