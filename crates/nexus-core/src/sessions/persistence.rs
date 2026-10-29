//! Speichert die Sessions aller Projekte in `~/.nexus/sessions.json` (atomar).
//! Portiert aus `packages/core/src/sessions/SessionPersistence.ts`.

use std::fs;

use serde::{Deserialize, Serialize};

use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;

use super::session::Session;

/// Form der Datei: ein Objekt mit der Liste der Sessions
#[derive(Debug, Default, Serialize, Deserialize)]
struct SessionsFile {
    #[serde(default)]
    sessions: Vec<Session>,
}

/// Liest und schreibt `sessions.json`
#[derive(Debug)]
pub struct SessionPersistence {
    home: HarnessHome,
    writer: AtomicWriter,
}

impl SessionPersistence {
    /// Speicher im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
        }
    }

    /// Liest die gespeicherten Sessions (leer, wenn es die Datei nicht gibt)
    pub fn read(&self) -> Result<Vec<Session>> {
        let file = self.home.sessions_file();
        if !file.exists() {
            return Ok(Vec::new());
        }
        let corrupt = || {
            HarnessError::new(
                "sessions-corrupt",
                format!("Die Sessiondatei ist beschädigt: {}", file.display()),
            )
        };
        let text = fs::read_to_string(&file).map_err(|_| corrupt())?;
        let data: SessionsFile = serde_json::from_str(&text).map_err(|_| corrupt())?;
        Ok(data.sessions)
    }

    /// Schreibt die Sessions; der Ordner wird bei Bedarf angelegt
    pub fn write(&self, sessions: &[Session]) -> Result<()> {
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        let data = SessionsFile {
            sessions: sessions.to_vec(),
        };
        self.writer.write(
            &self.home.sessions_file(),
            &serde_json::to_string(&data).map_err(Error::unexpected)?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::session::SessionState;
    use super::*;

    /// Eine Session als Vorlage
    fn sample() -> Session {
        Session {
            id: "s1".into(),
            project_id: "p".into(),
            tickets: vec!["T-0001".into()],
            cwd: "D:/wt".into(),
            state: SessionState::Idle,
            started_at: "2026-10-10T10:00:00.000Z".into(),
            last_event_at: "2026-10-10T10:00:00.000Z".into(),
            ended_at: None,
            title: None,
            model: None,
            subagents: Vec::new(),
        }
    }

    /// Ein Speicher in einem noch nicht vorhandenen Unterordner
    fn persistence(dir: &std::path::Path) -> SessionPersistence {
        SessionPersistence::new(HarnessHome::at(dir.join("neu")))
    }

    #[test]
    fn liefert_ohne_datei_eine_leere_liste() {
        let dir = tempfile::tempdir().unwrap();
        assert!(persistence(dir.path()).read().unwrap().is_empty());
    }

    #[test]
    fn schreibt_und_liest_sessions_auch_bei_noch_nicht_vorhandenem_ordner() {
        let dir = tempfile::tempdir().unwrap();
        let p = persistence(dir.path());
        p.write(&[sample()]).unwrap();
        assert_eq!(p.read().unwrap(), [sample()]);
        let text = fs::read_to_string(dir.path().join("neu").join("sessions.json")).unwrap();
        let data: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(data["sessions"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn meldet_eine_beschadigte_datei_verstandlich() {
        let dir = tempfile::tempdir().unwrap();
        let p = persistence(dir.path());
        p.write(&[sample()]).unwrap();
        fs::write(dir.path().join("neu").join("sessions.json"), "{kaputt").unwrap();
        let error = p.read().unwrap_err();
        assert_eq!(error.code(), Some("sessions-corrupt"));
        assert!(error.to_string().contains("sessions.json"));
    }

    #[test]
    fn ubersteht_viele_gleichzeitige_schreibzugriffe_mit_gultigem_ergebnis() {
        let dir = tempfile::tempdir().unwrap();
        let p = std::sync::Arc::new(persistence(dir.path()));
        let threads: Vec<_> = (0..20)
            .map(|i| {
                let p = std::sync::Arc::clone(&p);
                std::thread::spawn(move || {
                    p.write(&[Session {
                        id: format!("s{i}"),
                        ..sample()
                    }])
                    .unwrap()
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(p.read().unwrap().len(), 1);
    }
}
