//! Vergibt jeder Session eines Projekts dauerhaft eine fortlaufende Nummer
//! (dieselbe Session behält ihre Nummer).

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::sync::Arc;

use crate::counter::{Counter, CounterKind};
use crate::error::{Error, Result};
use crate::locking::{AtomicWriter, FileLock, FileLockOptions};
use crate::project::HarnessLayout;

/// Zuordnung Session → Nummer. Die Schlüssel stehen sortiert in der Datei;
/// JSON kennt keine Reihenfolge, TypeScript liest dieselbe Datei unverändert weiter.
type NumberMap = BTreeMap<String, i64>;

/// Vergibt und merkt sich die Nummern der Sessions eines Projekts.
#[derive(Debug)]
pub struct SessionNumbers {
    layout: HarnessLayout,
    counter: Arc<Counter>,
    lock: FileLock,
    writer: AtomicWriter,
}

impl SessionNumbers {
    /// Nummern im Harness-Ordner dieses Projekts; der Zähler vergibt die neuen Nummern
    pub fn new(layout: HarnessLayout, counter: Arc<Counter>) -> Self {
        let lock = FileLock::new(layout.session_numbers_lock(), FileLockOptions::default());
        Self {
            layout,
            counter,
            lock,
            writer: AtomicWriter::new(),
        }
    }

    /// Liefert die Nummer der Session; vergibt eine neue, falls sie noch keine hat
    pub fn assign(&self, session_id: &str) -> Result<i64> {
        self.lock.with_lock(|| {
            let mut map = self.read()?;
            if let Some(known) = map.get(session_id) {
                return Ok(*known);
            }
            let number = self.counter.next(CounterKind::Session)?;
            map.insert(session_id.to_string(), number);
            let json = serde_json::to_string(&map).map_err(Error::unexpected)?;
            self.writer
                .write(&self.layout.session_numbers_file(), &json)?;
            Ok(number)
        })
    }

    /// Liest die Zuordnung; fehlende Datei = noch keine Sessions. Eine beschädigte Datei
    /// meldet einen Fehler, statt still neu zu beginnen – sonst gäbe es Nummern doppelt.
    fn read(&self) -> Result<NumberMap> {
        match fs::read_to_string(self.layout.session_numbers_file()) {
            Ok(text) => serde_json::from_str(&text).map_err(Error::unexpected),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(NumberMap::new()),
            Err(error) => Err(Error::unexpected(error)),
        }
    }
}
