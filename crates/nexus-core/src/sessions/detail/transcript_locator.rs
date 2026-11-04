//! Findet das Transcript einer Session: der Pfad aus dem Hook, sonst
//! `<Config-Ordner>/projects/<kodierter cwd>/<id>.jsonl`, sonst die erste Datei `<id>.jsonl`
//! in einem Projektordner von Claude Code.

use std::path::{Path, PathBuf};

/// Kodiert den Arbeitsordner wie Claude Code: alles außer `[A-Za-z0-9]` wird zu `-`
pub fn encode_cwd(cwd: &str) -> String {
    cwd.chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() {
                char
            } else {
                '-'
            }
        })
        .collect()
}

/// Sucht Transcripts in den Config-Ordnern von Claude Code
#[derive(Debug, Clone, Default)]
pub struct TranscriptLocator {
    config_dirs: Vec<PathBuf>,
}

impl TranscriptLocator {
    /// Sucht in diesen Config-Ordnern (`~/.claude` bzw. `CLAUDE_CONFIG_DIR` der Konten)
    pub fn new(config_dirs: Vec<PathBuf>) -> Self {
        Self { config_dirs }
    }

    /// Pfad des Transcripts; nichts, wenn keines existiert
    pub fn locate(&self, known: Option<&str>, cwd: &str, session_id: &str) -> Option<PathBuf> {
        if let Some(path) = known.map(PathBuf::from).filter(|path| path.is_file()) {
            return Some(path);
        }
        if !valid_id(session_id) {
            return None;
        }
        let name = format!("{session_id}.jsonl");
        let encoded = encode_cwd(cwd);
        for dir in &self.config_dirs {
            let path = dir.join("projects").join(&encoded).join(&name);
            if path.is_file() {
                return Some(path);
            }
        }
        self.config_dirs
            .iter()
            .find_map(|dir| scan(&dir.join("projects"), &name))
    }
}

/// Die Datei `name` in einem Unterordner von `projects`
fn scan(projects: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(projects)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().join(name))
        .find(|path| path.is_file())
}

/// Nur Kennungen, die kein Pfad sein können
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '-' || char == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::make_temp_dir;

    #[test]
    fn kodiert_den_arbeitsordner_wie_claude_code() {
        assert_eq!(encode_cwd("C:\\Users\\x\\Nexus"), "C--Users-x-Nexus");
        assert_eq!(encode_cwd("D:/wt/ä b.c"), "D--wt---b-c");
    }

    #[test]
    fn nimmt_den_bekannten_pfad_sonst_den_kodierten_ordner_sonst_die_suche() {
        let dir = make_temp_dir("claude-config-");
        let config = dir.path().to_path_buf();
        let locator = TranscriptLocator::new(vec![config.join("fehlt"), config.clone()]);
        let encoded = config.join("projects").join("D--wt");
        std::fs::create_dir_all(&encoded).unwrap();
        std::fs::write(encoded.join("s1.jsonl"), "{}\n").unwrap();
        assert_eq!(
            locator.locate(None, "D:/wt", "s1"),
            Some(encoded.join("s1.jsonl"))
        );
        // Bekannter Pfad gewinnt, ein nicht vorhandener wird übergangen
        let known = config.join("anders.jsonl");
        std::fs::write(&known, "{}\n").unwrap();
        assert_eq!(
            locator.locate(Some(&known.to_string_lossy()), "D:/wt", "s1"),
            Some(known)
        );
        assert_eq!(
            locator.locate(Some("C:/gibt/es/nicht.jsonl"), "D:/wt", "s1"),
            Some(encoded.join("s1.jsonl"))
        );
        // Anderer Arbeitsordner (z. B. nach cd): Suche über alle Projektordner
        assert_eq!(
            locator.locate(None, "E:/sonst", "s1"),
            Some(encoded.join("s1.jsonl"))
        );
        assert_eq!(locator.locate(None, "D:/wt", "s2"), None);
        assert_eq!(locator.locate(None, "D:/wt", "../s1"), None);
    }
}
