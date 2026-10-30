//! Beendet den laufenden Harness-Server (nach Updates, damit die neue Version startet).
//! Portiert aus `packages/core/src/install/ServerStopper.ts`.

use std::time::{Duration, Instant};

use crate::error::Result;
use crate::registry::{HarnessHome, ServerInfoFile};

/// Wie lange auf das Ende des Servers gewartet wird (`WAIT_MS` in TypeScript)
const WAIT: Duration = Duration::from_secs(5);

/// Abstand zweier Prüfungen beim Warten
const STEP: Duration = Duration::from_millis(100);

/// Was der Stopper von außen braucht: die Prüfung des Ports und das Beenden des Prozesses.
/// In TypeScript ist das `StopDeps`.
pub trait StopDeps {
    /// Antwortet unter diesem Port ein Harness-Server?
    fn is_ours(&self, port: u16) -> bool;
    /// Beendet den Prozess
    fn kill(&self, pid: u32);
}

/// Beendet den laufenden Harness-Server.
pub struct ServerStopper<D: StopDeps> {
    info: ServerInfoFile,
    deps: D,
    /// Port des zuletzt beendeten Servers (für einen Neustart)
    stopped_port: Option<u16>,
}

impl<D: StopDeps> ServerStopper<D> {
    /// Stopper für diesen Benutzerordner mit eigenen Abhängigkeiten
    pub fn new(home: HarnessHome, deps: D) -> Self {
        Self {
            info: ServerInfoFile::new(home),
            deps,
            stopped_port: None,
        }
    }

    /// Port des zuletzt beendeten Servers
    pub fn stopped_port(&self) -> Option<u16> {
        self.stopped_port
    }

    /// Beendet den Server; liefert `false`, wenn keiner läuft. Veraltete Angaben werden bereinigt.
    pub fn stop(&mut self) -> Result<bool> {
        let Some(info) = self.info.read() else {
            return Ok(false);
        };
        if !self.deps.is_ours(info.port) {
            self.info.remove()?;
            return Ok(false);
        }
        // Ist der Prozess schon weg, wird unten geprüft, ob der Port noch antwortet
        self.deps.kill(info.pid);
        let deadline = Instant::now() + WAIT;
        while Instant::now() < deadline && self.deps.is_ours(info.port) {
            std::thread::sleep(STEP);
        }
        self.info.remove()?;
        self.stopped_port = Some(info.port);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ServerInfo;
    use std::cell::RefCell;

    /// Abhängigkeiten der Tests: merkt sich die beendeten Prozesse; `alive` steuert die Prüfung
    struct TestDeps {
        alive: RefCell<bool>,
        stirbt_beim_beenden: bool,
        kills: RefCell<Vec<u32>>,
    }

    impl TestDeps {
        fn new(alive: bool, stirbt_beim_beenden: bool) -> Self {
            Self {
                alive: RefCell::new(alive),
                stirbt_beim_beenden,
                kills: RefCell::new(Vec::new()),
            }
        }
    }

    impl StopDeps for TestDeps {
        fn is_ours(&self, _port: u16) -> bool {
            *self.alive.borrow()
        }

        fn kill(&self, pid: u32) {
            self.kills.borrow_mut().push(pid);
            if self.stirbt_beim_beenden {
                *self.alive.borrow_mut() = false;
            }
        }
    }

    fn home() -> (tempfile::TempDir, HarnessHome) {
        let dir = tempfile::Builder::new()
            .prefix("harness-stop-")
            .tempdir()
            .unwrap();
        let home = HarnessHome::at(dir.path());
        (dir, home)
    }

    fn angaben() -> ServerInfo {
        ServerInfo {
            pid: 4242,
            port: 4710,
            started_at: "jetzt".into(),
            app_token: None,
        }
    }

    #[test]
    fn meldet_false_wenn_es_keine_serverangaben_gibt() {
        let (_dir, home) = home();
        let mut stopper = ServerStopper::new(home, TestDeps::new(false, false));
        assert!(!stopper.stop().unwrap());
        assert!(stopper.deps.kills.borrow().is_empty());
        assert_eq!(stopper.stopped_port(), None);
    }

    #[test]
    fn beendet_den_prozess_aus_den_angaben_und_raeumt_auf() {
        let (_dir, home) = home();
        ServerInfoFile::new(home.clone()).write(&angaben()).unwrap();
        let mut stopper = ServerStopper::new(home.clone(), TestDeps::new(true, true));
        assert!(stopper.stop().unwrap());
        assert_eq!(*stopper.deps.kills.borrow(), vec![4242]);
        assert_eq!(stopper.stopped_port(), Some(4710));
        assert!(!home.server_file().exists());
    }

    #[test]
    fn beendet_nichts_wenn_unter_dem_port_kein_harness_server_antwortet() {
        let (_dir, home) = home();
        ServerInfoFile::new(home.clone()).write(&angaben()).unwrap();
        let mut stopper = ServerStopper::new(home.clone(), TestDeps::new(false, false));
        assert!(!stopper.stop().unwrap());
        assert!(stopper.deps.kills.borrow().is_empty());
        assert_eq!(stopper.stopped_port(), None);
        assert!(!home.server_file().exists());
    }
}
