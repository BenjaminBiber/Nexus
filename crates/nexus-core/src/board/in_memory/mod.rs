//! Board-Datenbank im Speicher für Tests; portiert aus
//! `packages/core/src/board/InMemoryBoardDatabase.ts`.
//!
//! Dieselbe Bedeutung wie SQL Server: jede Aktion läuft in genau einer Transaktion, die auf
//! einer Kopie des Zustands arbeitet und sie nur bei Erfolg übernimmt; `update` prüft die
//! Version (`rev`) und meldet sonst `conflict`; `change_marker` und `question_marker` steigen
//! nur bei erfolgreichen Transaktionen.
//!
//! Die Warteschlange aus TypeScript (`this.queue`) ersetzt hier ein
//! [`tokio::sync::Mutex`]: Transaktionen laufen nacheinander, auch wenn mehrere Aufgaben
//! gleichzeitig arbeiten.
//!
//! ```
//! use nexus_core::board::database::{BoardDatabase, BoardUnitOfWork};
//! use nexus_core::board::in_memory::InMemoryBoardDatabase;
//! use nexus_core::board::types::BoardUser;
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! let db = InMemoryBoardDatabase::new();
//! let user = BoardUser { id: "u-1".into(), name: "Florian".into() };
//! nexus_core::tx!(db, [user], |u| { u.users().upsert(&user).await }).await.unwrap();
//! let geladen = nexus_core::tx!(db, |u| { u.users().by_id("u-1").await }).await.unwrap();
//! assert_eq!(geladen.unwrap().name, "Florian");
//! # });
//! ```

mod search;
mod state;
mod unit_of_work;

use std::collections::BTreeMap;
use std::sync::Mutex;

use tokio::sync::Mutex as AsyncMutex;

use crate::board::database::{BoardDatabase, BoardUnitOfWork, TransactionWork};
use crate::error::Result;

pub use search::{InMemoryBoardSearch, SearchableState};

use state::{State, Touched};
use unit_of_work::InMemoryUnitOfWork;

/// Board-Datenbank im Speicher für Tests
#[derive(Debug, Default)]
pub struct InMemoryBoardDatabase {
    /// Der Zustand; die Sperre sorgt dafür, dass Transaktionen nacheinander laufen
    state: AsyncMutex<State>,
    /// Änderungszähler je Board (Grundlage für `change_marker`)
    markers: Mutex<BTreeMap<i32, u64>>,
    /// Änderungszähler der Fragen je Projekt (Grundlage für `question_marker`)
    question_markers: Mutex<BTreeMap<i32, u64>>,
}

impl InMemoryBoardDatabase {
    /// Eine leere Datenbank
    pub fn new() -> Self {
        Self::default()
    }

    /// Zählt die berührten Kennwerte weiter
    fn bump(counters: &Mutex<BTreeMap<i32, u64>>, ids: impl IntoIterator<Item = i32>) {
        let mut counters = counters.lock().expect("Kennwerte sind nicht vergiftet");
        for id in ids {
            *counters.entry(id).or_insert(0) += 1;
        }
    }

    /// Stand eines Zählers als Text (0, wenn es ihn noch nicht gibt)
    fn marker(counters: &Mutex<BTreeMap<i32, u64>>, id: i32) -> String {
        let counters = counters.lock().expect("Kennwerte sind nicht vergiftet");
        counters.get(&id).copied().unwrap_or(0).to_string()
    }
}

impl BoardDatabase for InMemoryBoardDatabase {
    /// Führt die Arbeit auf einer Kopie aus und übernimmt sie nur bei Erfolg; berührte Boards
    /// und Projekte zählen weiter
    async fn transaction<'a, T, W>(&'a self, work: W) -> Result<T>
    where
        T: Send + 'static,
        W: TransactionWork<T> + 'a,
    {
        let mut state = self.state.lock().await;
        let mut draft = state.clone();
        let mut touched = Touched::default();
        let result = {
            let mut uow = InMemoryUnitOfWork {
                state: &mut draft,
                touched: &mut touched,
            };
            work(&mut uow as &mut dyn BoardUnitOfWork).await?
        };
        *state = draft;
        drop(state);
        Self::bump(&self.markers, touched.boards);
        Self::bump(&self.question_markers, touched.projects);
        Ok(result)
    }

    /// Zähler des Boards als Text; steigt mit jeder erfolgreichen Transaktion, die das Board
    /// berührt
    async fn change_marker(&self, board_id: i32) -> Result<String> {
        Ok(Self::marker(&self.markers, board_id))
    }

    /// Zähler der Fragen eines Projekts als Text
    async fn question_marker(&self, project_id: i32) -> Result<String> {
        Ok(Self::marker(&self.question_markers, project_id))
    }

    /// Nichts zu schließen
    async fn close(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
