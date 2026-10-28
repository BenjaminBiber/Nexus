//! Hält genau eine Board-Laufzeit für den Server; portiert aus
//! `packages/server/src/dbboard/BoardRuntimeHolder.ts` und `BoardBackend.ts`.
//!
//! Aufbau beim ersten Bedarf, Neuaufbau nach Verbindungsverlust oder geänderter Konfig. Was
//! als Verbindungsverlust zählt, entscheidet die Datenzugriffsschicht: Datei- und
//! Zugriffsfehler an der Board-Datei
//! ([`nexus_db::sqlite::error_map::is_file_error`]), E/A-, TLS- und Protokollfehler des
//! SQL-Server-Treibers ([`MssqlErrorMap::is_connection_error`]) und die Fehler, die beide
//! Schichten dafür schon übersetzt haben (`board-db-unreachable`, `board-db-closed`).

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use nexus_core::board::basics::BoardActor;
use nexus_core::{Error, HarnessError, Result};
use nexus_db::mssql::error_map::MssqlErrorMap;
use nexus_db::sqlite::error_map as sqlite_errors;
use tokio::sync::Mutex;

/// Was der Server von einer Board-Laufzeit braucht.
///
/// Die Dienste des Boards ([`nexus_core::board`]) hängen an der Datenbank und sind deshalb
/// generisch; sie kommen mit der Rust-Board-Laufzeit dazu (P4-3), sobald die Routen der
/// `/api/db`-Gruppe entstehen. Hier steht nur, was das Grundgerüst schon braucht.
#[async_trait]
pub trait BoardBackend: Send + Sync {
    /// Der Handelnde dieser Laufzeit; ohne Namen der eigene Benutzer
    fn actor(&self, name: Option<&str>) -> BoardActor;

    /// Schließt die Verbindung
    async fn close(&self) -> Result<()>;
}

/// Der Aufbau einer Laufzeit als Zukunft
pub type BoardBackendFuture =
    Pin<Box<dyn Future<Output = Result<Arc<dyn BoardBackend>>> + Send + 'static>>;

/// Baut eine Laufzeit auf (verbindet, prüft das Schema, legt den Benutzer an)
pub type BoardBackendFactory = Arc<dyn Fn() -> BoardBackendFuture + Send + Sync>;

/// Baut eine Fabrik aus einer Funktion, die den Aufbau als Zukunft liefert.
///
/// Der Aufruf schreibt `Box::pin(…)` selbst und nennt [`BoardBackendFuture`] als
/// Rückgabetyp des Abschlusses: ohne diese Angabe scheitert der Compiler an einer bekannten
/// Grenze (rust-lang/rust#100013), sobald die Zukunft etwas aus der Umgebung festhält.
pub fn factory<F>(build: F) -> BoardBackendFactory
where
    F: Fn() -> BoardBackendFuture + Send + Sync + 'static,
{
    Arc::new(build)
}

/// Eine aufgebaute Laufzeit samt ihrer Nummer; die Nummer unterscheidet die alte von der
/// inzwischen neu gebauten Laufzeit
#[derive(Clone)]
struct Current {
    generation: u64,
    backend: Arc<dyn BoardBackend>,
}

/// Hält genau eine Board-Laufzeit für den Server.
pub struct BoardRuntimeHolder {
    factory: BoardBackendFactory,
    /// Die Sperre wird über den ganzen Aufbau gehalten: gleichzeitige Aufrufe von
    /// [`BoardRuntimeHolder::get`] teilen sich damit einen Aufbau, so wie sie sich in
    /// TypeScript dasselbe Promise teilen
    current: Mutex<Option<Current>>,
    generations: AtomicU64,
}

impl BoardRuntimeHolder {
    /// Halter mit der Fabrik, die die Laufzeit baut
    pub fn new(factory: BoardBackendFactory) -> Self {
        Self {
            factory,
            current: Mutex::new(None),
            generations: AtomicU64::new(0),
        }
    }

    /// Übersetzt Fehler des Aufbaus in die Fehler, die der Server als 503 meldet
    pub fn translate(error: Error) -> Error {
        if error.code() == Some("board-config-invalid") {
            let message = error.to_string();
            return HarnessError::new(
                "board-config-broken",
                format!("{message} Unter Einstellungen → Board-Datenbank reparieren."),
            )
            .into();
        }
        if sqlite_errors::is_file_error(&error) {
            return HarnessError::new(
                "board-db-unreachable",
                format!("Board-Datei ist nicht erreichbar: {error}"),
            )
            .into();
        }
        error
    }

    /// Die Laufzeit; gleichzeitige Aufrufe teilen sich den Aufbau, ein Fehlschlag wird nicht
    /// gemerkt
    pub async fn get(&self) -> Result<Arc<dyn BoardBackend>> {
        Ok(self.current().await?.backend)
    }

    /// Wie [`BoardRuntimeHolder::get`], aber mit der Nummer der Laufzeit
    async fn current(&self) -> Result<Current> {
        let mut slot = self.current.lock().await;
        if let Some(current) = slot.as_ref() {
            return Ok(current.clone());
        }
        let backend = (self.factory)().await.map_err(Self::translate)?;
        let current = Current {
            generation: self.generations.fetch_add(1, Ordering::Relaxed),
            backend,
        };
        *slot = Some(current.clone());
        Ok(current)
    }

    /// Führt Arbeit mit der Laufzeit aus; ein Verbindungsfehler verwirft sie und wird zu
    /// `board-db-unreachable`
    pub async fn run<T, F, Fut>(&self, work: F) -> Result<T>
    where
        F: FnOnce(Arc<dyn BoardBackend>) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let current = self.current().await?;
        match work(current.backend.clone()).await {
            Ok(value) => Ok(value),
            Err(error) if Self::is_connection_error(&error) => {
                self.discard(current.generation).await;
                Err(HarnessError::new(
                    "board-db-unreachable",
                    format!("Verbindung zur Board-Datenbank verloren: {error}"),
                )
                .into())
            }
            // Lesezugriffe laufen außerhalb von Transaktionen und werden dort nicht
            // übersetzt (z. B. gesperrte Datei, Speichermangel)
            Err(error) => Err(sqlite_errors::map(error)),
        }
    }

    /// Verwirft die Laufzeit (nach geänderter Konfig oder beim Beenden); die nächste Anfrage
    /// baut neu auf
    pub async fn reset(&self) {
        let current = self.current.lock().await.take();
        if let Some(current) = current {
            // Aufbau oder Schließen war ohnehin gescheitert
            let _ = current.backend.close().await;
        }
    }

    /// Verwirft nur die fehlerhafte Laufzeit; ist inzwischen eine neue gebaut, bleibt sie
    /// unberührt
    async fn discard(&self, generation: u64) {
        let mut slot = self.current.lock().await;
        let same = slot
            .as_ref()
            .is_some_and(|current| current.generation == generation);
        if !same {
            return;
        }
        let failed = slot.take();
        drop(slot);
        if let Some(failed) = failed {
            let _ = failed.backend.close().await;
        }
    }

    /// Netz- oder Verbindungsfehler: Datei-/Zugriffsfehler an der Board-Datei, E/A-, TLS- und
    /// Protokollfehler des SQL-Server-Treibers sowie die Fehler, die die Datenzugriffsschicht
    /// dafür schon übersetzt hat. Alles andere (auch Konflikte und Speichermangel) ist ein
    /// Fach- oder Programmfehler.
    pub fn is_connection_error(error: &Error) -> bool {
        matches!(
            error.code(),
            Some("board-db-unreachable") | Some("board-db-closed")
        ) || sqlite_errors::is_file_error(error)
            || MssqlErrorMap::is_connection_error(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eine_kaputte_board_konfig_wird_zu_board_config_broken() {
        let translated = BoardRuntimeHolder::translate(
            HarnessError::new("board-config-invalid", "board.yaml ist kein gültiges YAML.").into(),
        );
        let harness = translated.harness().unwrap();
        assert_eq!(harness.code, "board-config-broken");
        assert_eq!(
            harness.message,
            "board.yaml ist kein gültiges YAML. Unter Einstellungen → Board-Datenbank reparieren."
        );
    }

    #[test]
    fn fremde_fehler_bleiben_beim_aufbau_unverandert() {
        let translated =
            BoardRuntimeHolder::translate(Error::unexpected_message("Rechte fehlen beim Öffnen"));
        assert_eq!(translated.to_string(), "Rechte fehlen beim Öffnen");
        assert_eq!(translated.code(), None);
    }
}
