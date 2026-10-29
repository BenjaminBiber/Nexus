//! Die Datenbank, auf der die Dienste des Servers arbeiten.
//!
//! [`nexus_core::board::database::BoardDatabase`] ist wegen der generischen `transaction`
//! nicht objektsicher, die Dienste ([`nexus_core::board::services::BoardServices`]) sind
//! deshalb generisch. Der Server hält seine Laufzeit aber hinter einem Trait-Objekt
//! ([`crate::board_runtime::BoardBackend`]) und braucht darum **einen** festen Datenbanktyp.
//! Dieses Enum ist er: im Betrieb die geöffnete Datenbank aus `board.yaml`
//! ([`AnyBoardDatabase`]), in Tests die Datenbank im Speicher – das Gegenstück zu
//! `packages/server/tests/helpers/MemoryBoardBackend.ts`.

use nexus_core::board::database::{BoardDatabase, TransactionWork};
use nexus_core::board::in_memory::InMemoryBoardDatabase;
use nexus_core::board::services::BoardServices;
use nexus_core::Result;
use nexus_db::AnyBoardDatabase;

/// Die Datenbank des Servers: im Betrieb die geöffnete, in Tests die im Speicher
pub enum ServerBoardDatabase {
    /// Die Datenbank aus `board.yaml` (SQLite oder SQL Server)
    Open(AnyBoardDatabase),
    /// Datenbank im Speicher; nur Tests bauen sie. Sie liegt im Kasten, weil sie ihre Tabellen
    /// selbst hält und sonst jede Laufzeit des Betriebs so groß wäre wie sie
    Memory(Box<InMemoryBoardDatabase>),
}

/// Die Dienste des Boards, wie der Server sie hält
pub type ServerBoardServices = BoardServices<ServerBoardDatabase>;

impl BoardDatabase for ServerBoardDatabase {
    async fn transaction<T, W>(&self, work: W) -> Result<T>
    where
        T: Send + 'static,
        W: TransactionWork<T> + 'static,
    {
        match self {
            ServerBoardDatabase::Open(db) => db.transaction(work).await,
            ServerBoardDatabase::Memory(db) => db.transaction(work).await,
        }
    }

    async fn change_marker(&self, board_id: i32) -> Result<String> {
        match self {
            ServerBoardDatabase::Open(db) => db.change_marker(board_id).await,
            ServerBoardDatabase::Memory(db) => db.change_marker(board_id).await,
        }
    }

    async fn question_marker(&self, project_id: i32) -> Result<String> {
        match self {
            ServerBoardDatabase::Open(db) => db.question_marker(project_id).await,
            ServerBoardDatabase::Memory(db) => db.question_marker(project_id).await,
        }
    }

    async fn close(&self) -> Result<()> {
        match self {
            ServerBoardDatabase::Open(db) => db.close().await,
            ServerBoardDatabase::Memory(db) => db.close().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::board::types::BoardUser;
    use nexus_core::tx;

    /// Die Datenbank im Speicher reicht jeden Aufruf weiter
    #[tokio::test]
    async fn reicht_transaktionen_an_die_datenbank_im_speicher_weiter() {
        let db = ServerBoardDatabase::Memory(Box::new(InMemoryBoardDatabase::new()));
        let user = BoardUser {
            id: "11111111-1111-1111-1111-111111111111".into(),
            name: "Florian".into(),
        };
        tx!(db, [user], |u| { u.users().upsert(&user).await })
            .await
            .expect("Benutzer anlegen");
        let found = tx!(db, |u| { u.users().list().await })
            .await
            .expect("Benutzer lesen");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Florian");
    }
}
