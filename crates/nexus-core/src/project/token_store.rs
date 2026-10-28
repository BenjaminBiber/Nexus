//! Zugriffs-Token für spätere Hook-Aufrufe; wird einmal bei `harness init` erzeugt.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};

use crate::error::{Error, Result};
use crate::project::HarnessLayout;
use crate::random;

/// Legt das Token an und liest es.
#[derive(Debug, Clone)]
pub struct TokenStore {
    layout: HarnessLayout,
}

impl TokenStore {
    /// Token im Harness-Ordner dieses Projekts
    pub fn new(layout: HarnessLayout) -> Self {
        Self { layout }
    }

    /// Legt das Token an, falls es noch fehlt (bestehendes Token bleibt unverändert)
    pub fn ensure(&self) -> Result<()> {
        let file = self.layout.token_file();
        if file.exists() {
            return Ok(());
        }
        match OpenOptions::new().write(true).create_new(true).open(&file) {
            Ok(mut handle) => handle
                .write_all(random::hex(32).as_bytes())
                .map_err(Error::unexpected),
            // Ein anderer Prozess war schneller – sein Token gilt
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(Error::unexpected(error)),
        }
    }

    /// Liest das Token
    pub fn read(&self) -> Result<String> {
        fs::read_to_string(self.layout.token_file())
            .map(|text| text.trim().to_string())
            .map_err(Error::unexpected)
    }
}
