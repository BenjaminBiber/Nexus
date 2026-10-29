//! Liest die Hook-Eingabe von der Standardeingabe; kehrt nach der Wartezeit auch ohne Ende
//! der Eingabe zurück. Portiert aus `packages/cli/src/hooks/StdinReader.ts`.

use std::io::Read;
use std::sync::mpsc;
use std::time::Duration;

/// Wartezeit, nach der die Eingabe auch unvollständig zählt (`timeoutMs` in TypeScript)
pub const DEFAULT_TIMEOUT_MS: u64 = 1500;

/// Liest die Standardeingabe mit Zeitlimit.
#[derive(Debug, Clone, Copy)]
pub struct StdinReader;

impl StdinReader {
    /// Liefert den gelesenen Text (leer bei Terminal, Fehler oder Zeitüberschreitung).
    /// Das Lesen läuft in einem eigenen Thread: eine Eingabe, die nie endet, hält den Hook
    /// nicht auf.
    pub fn read(timeout: Duration) -> String {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = std::io::stdin().read_to_string(&mut text);
            let _ = sender.send(text);
        });
        receiver.recv_timeout(timeout).unwrap_or_default()
    }

    /// Dieselbe Eingabe mit der Standard-Wartezeit
    pub fn read_default() -> String {
        Self::read(Duration::from_millis(DEFAULT_TIMEOUT_MS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eine_eingabe_die_nicht_kommt_endet_mit_leerem_text() {
        // Im Testlauf hängt an stdin nichts; nach der Frist ist der Text leer
        let text = StdinReader::read(Duration::from_millis(50));
        assert!(text.is_empty(), "{text}");
    }
}
