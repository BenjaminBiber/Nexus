//! Prozessangaben des laufenden Servers (`server.json`); nur ein Hinweis, maßgeblich ist
//! der Health-Aufruf.

use std::fs;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;

/// Prozessangaben des laufenden Servers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub pid: u32,
    pub port: u16,
    pub started_at: String,
}

/// Liest und schreibt `server.json`.
#[derive(Debug, Clone)]
pub struct ServerInfoFile {
    home: HarnessHome,
    writer: AtomicWriter,
}

impl ServerInfoFile {
    /// Datei im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
        }
    }

    /// Liest die Angaben (None, wenn es sie nicht gibt oder sie unlesbar sind)
    pub fn read(&self) -> Option<ServerInfo> {
        let text = fs::read_to_string(self.home.server_file()).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Schreibt die Angaben
    pub fn write(&self, info: &ServerInfo) -> Result<()> {
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        let json = serde_json::to_string(info).map_err(Error::unexpected)?;
        self.writer.write(&self.home.server_file(), &json)
    }

    /// Löscht die Angaben beim Beenden
    pub fn remove(&self) -> Result<()> {
        match fs::remove_file(self.home.server_file()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::unexpected(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schreibt_liest_und_loscht_die_prozessangaben() {
        let dir = tempfile::Builder::new()
            .prefix("harness-info-")
            .tempdir()
            .unwrap();
        let home = HarnessHome::at(dir.path());
        let file = ServerInfoFile::new(home.clone());
        let info = ServerInfo {
            pid: 42,
            port: 4710,
            started_at: "2026-10-09T10:00:00.000Z".into(),
        };

        assert_eq!(file.read(), None);
        file.write(&info).unwrap();
        assert_eq!(file.read(), Some(info));

        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(home.server_file()).unwrap()).unwrap();
        assert_eq!(json["port"], serde_json::json!(4710));
        assert_eq!(
            json["startedAt"],
            serde_json::json!("2026-10-09T10:00:00.000Z")
        );

        file.remove().unwrap();
        assert!(!home.server_file().exists());
        // Zweimal löschen ist kein Fehler
        file.remove().unwrap();
    }
}
