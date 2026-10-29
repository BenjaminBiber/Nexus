//! Prozessangaben des laufenden Servers (`server.json`); nur ein Hinweis, maßgeblich ist
//! der Health-Aufruf.

use std::fs;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::locking::AtomicWriter;
use crate::process::is_alive;
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

    /// Löscht die Angaben (`stop`, nachdem es den eingetragenen Server beendet hat)
    pub fn remove(&self) -> Result<()> {
        match fs::remove_file(self.home.server_file()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::unexpected(error)),
        }
    }

    /// Trägt den startenden Server ein – außer ein anderer, noch laufender Server steht schon
    /// darin (etwa wenn ein Testserver versehentlich mit dem echten Benutzerordner startet).
    /// `false`: nicht eingetragen
    pub fn claim(&self, info: &ServerInfo) -> Result<bool> {
        if let Some(current) = self.read() {
            if current.pid != info.pid && is_alive(current.pid) {
                return Ok(false);
            }
        }
        self.write(info)?;
        Ok(true)
    }

    /// Löscht die Angaben beim Beenden, aber nur, wenn sie zu diesem Server (`pid`) gehören
    pub fn release(&self, pid: u32) -> Result<()> {
        match self.read() {
            Some(current) if current.pid == pid => self.remove(),
            _ => Ok(()),
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

    /// Ein Benutzerordner mit `server.json`
    fn datei() -> (tempfile::TempDir, ServerInfoFile) {
        let dir = tempfile::tempdir().unwrap();
        let file = ServerInfoFile::new(HarnessHome::at(dir.path()));
        (dir, file)
    }

    fn angaben(pid: u32, port: u16) -> ServerInfo {
        ServerInfo {
            pid,
            port,
            started_at: "2026-10-29T10:00:00.000Z".into(),
        }
    }

    /// Eine Prozessnummer, die nie vergeben ist
    const TOTE_PID: u32 = 2_147_483_646;

    #[test]
    fn traegt_ein_wenn_die_datei_fehlt_oder_ein_beendeter_server_darin_steht() {
        let (_dir, file) = datei();
        assert!(file.claim(&angaben(std::process::id(), 4710)).unwrap());
        assert_eq!(file.read().unwrap().port, 4710);

        file.write(&angaben(TOTE_PID, 4711)).unwrap();
        assert!(file.claim(&angaben(std::process::id(), 4712)).unwrap());
        assert_eq!(file.read().unwrap().port, 4712);
    }

    #[test]
    fn ueberschreibt_keinen_anderen_laufenden_server() {
        let (_dir, file) = datei();
        let mut anderer = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        file.write(&angaben(anderer.id(), 4710)).unwrap();

        let eingetragen = file.claim(&angaben(std::process::id(), 4719)).unwrap();
        let gelesen = file.read();
        let _ = anderer.kill();
        let _ = anderer.wait();

        assert!(!eingetragen);
        assert_eq!(gelesen, Some(angaben(anderer.id(), 4710)));
    }

    #[test]
    fn loescht_beim_beenden_nur_die_eigenen_angaben() {
        let (_dir, file) = datei();
        file.write(&angaben(TOTE_PID, 4710)).unwrap();

        file.release(std::process::id()).unwrap();
        assert!(file.read().is_some());

        file.release(TOTE_PID).unwrap();
        assert_eq!(file.read(), None);
    }
}
