//! Was Hooks von der Board-Laufzeit brauchen: die Dienste, die eigene Kennung und das
//! Schließen. Portiert aus `packages/cli/src/hooks/BoardAccess.ts`.
//!
//! Der Trait hält die Laufzeit austauschbar: im Betrieb die [`BoardRuntime`] aus `nexus-db`,
//! in Tests eine Laufzeit im Speicher oder eine, die nie antwortet.

use nexus_core::board::database::BoardDatabase;
use nexus_core::board::repo_board_resolver::RepoBoardOwner;
use nexus_core::board::services::BoardServices;
use nexus_core::HarnessHome;
use nexus_db::{BoardRuntime, ConnectOptions};

/// Eine geöffnete Board-Laufzeit, wie Hooks sie benutzen
// Die Methoden laufen alle im selben Thread; ein `Send`-Future braucht hier niemand.
#[allow(async_fn_in_trait)]
pub trait BoardAccess {
    /// Datenbank hinter den Diensten
    type Db: BoardDatabase;

    /// Alle Dienste des Boards
    fn services(&self) -> &BoardServices<Self::Db>;

    /// Eigene Kennung und aktives Board (`config` in TypeScript)
    fn owner(&self) -> RepoBoardOwner;

    /// Schließt die Verbindung
    async fn close(&self);
}

/// Öffnet die Board-Laufzeit; nichts, wenn keine Konfig da oder die DB nicht erreichbar ist
// Wie [`BoardAccess`]: ein Thread, kein `Send`.
#[allow(async_fn_in_trait)]
pub trait BoardOpener: Send + 'static {
    /// Die Laufzeit, die dieser Öffner liefert
    type Access: BoardAccess;

    /// Öffnet die Laufzeit zu diesem Harness-Ordner
    async fn open(&self, home: &HarnessHome) -> Option<Self::Access>;
}

impl BoardAccess for BoardRuntime {
    type Db = nexus_db::AnyBoardDatabase;

    fn services(&self) -> &BoardServices<Self::Db> {
        &self.services
    }

    fn owner(&self) -> RepoBoardOwner {
        RepoBoardOwner {
            user_id: self.config.user_id.clone(),
            active_board: self.config.active_board,
        }
    }

    async fn close(&self) {
        // Eine Verbindung, die ohnehin weg ist, bleibt ohne Folgen
        let _ = BoardRuntime::close(self).await;
    }
}

/// Öffnet die echte Board-Laufzeit aus `board.yaml`; jeder Fehler bedeutet „kein Board“
#[derive(Debug, Clone, Copy)]
pub struct RuntimeOpener {
    /// Wartezeit beim Verbinden; mehr als das Zeitbudget des Hooks bringt nichts
    budget_ms: u64,
}

impl RuntimeOpener {
    /// Öffner mit diesem Zeitbudget
    pub fn new(budget_ms: u64) -> Self {
        Self { budget_ms }
    }
}

impl BoardOpener for RuntimeOpener {
    type Access = BoardRuntime;

    async fn open(&self, home: &HarnessHome) -> Option<BoardRuntime> {
        let options = ConnectOptions {
            connection_timeout_ms: Some(self.budget_ms),
        };
        BoardRuntime::open(home, options).await.ok()
    }
}
