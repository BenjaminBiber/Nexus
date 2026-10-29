//! Baut den kopierbaren Befehl zum Fortsetzen einer Session (der Harness startet kein Terminal).
//! Portiert aus `packages/core/src/favorites/ResumeCommand.ts`.

/// Der Befehl `cd "<Verzeichnis>" && claude --resume <Kennung>`
pub struct ResumeCommand;

impl ResumeCommand {
    /// Anführungszeichen im Pfad werden maskiert; eine ungewöhnliche Kennung kommt in
    /// einfache Anführungszeichen
    pub fn build(cwd: &str, session_id: &str) -> String {
        let dir = format!("\"{}\"", cwd.replace('"', "\\\""));
        let id = if is_plain(session_id) {
            session_id.to_string()
        } else {
            format!("'{}'", session_id.replace('\'', "'\\''"))
        };
        format!("cd {dir} && claude --resume {id}")
    }
}

/// Besteht die Kennung nur aus Buchstaben, Ziffern, Punkt, Unterstrich und Bindestrich?
fn is_plain(session_id: &str) -> bool {
    !session_id.is_empty()
        && session_id
            .chars()
            .all(|z| z.is_ascii_alphanumeric() || z == '.' || z == '_' || z == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baut_den_befehl_mit_quotiertem_pfad() {
        assert_eq!(
            ResumeCommand::build("D:/Mein Projekt ä/wt", "abc-123"),
            "cd \"D:/Mein Projekt ä/wt\" && claude --resume abc-123"
        );
    }

    #[test]
    fn maskiert_anfuhrungszeichen_im_pfad_und_quotiert_ungewohnliche_kennungen() {
        assert_eq!(
            ResumeCommand::build("D:/a\"b", "x"),
            "cd \"D:/a\\\"b\" && claude --resume x"
        );
        assert_eq!(
            ResumeCommand::build("/tmp", "a b;c"),
            "cd \"/tmp\" && claude --resume 'a b;c'"
        );
    }
}
