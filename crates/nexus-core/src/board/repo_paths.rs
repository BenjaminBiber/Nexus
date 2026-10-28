//! Zentrale Stelle für den Repo-Pfad eines Boards: jeder Benutzer pflegt seinen Pfad lokal
//! (`board-repos.yaml`). Portiert aus `packages/core/src/board/BoardRepoPaths.ts`.
//!
//! Wirksam ist der lokale Eintrag; fehlt er, gilt bei eigenen Boards der DB-Wert (Altbestand),
//! bei fremden nie.
//!
//! Anders als in TypeScript sind `read` und `write` nicht `async`: der Dateispeicher
//! ([`crate::board::repo_file::BoardRepoFile`]) arbeitet synchron, und die Zuordnung wird
//! immer außerhalb einer Transaktion angewandt.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::board::repo_file::{BoardRepoEntries, BoardRepoFile};
use crate::board::types::Board;
use crate::error::Result;

/// Herkunft des wirksamen Repo-Pfads: lokale Zuordnung dieses Benutzers oder DB-Altbestand
/// (`Board.repo_path`, nur eigene Boards)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "board.ts")]
pub enum BoardRepoSource {
    Local,
    Legacy,
}

impl BoardRepoSource {
    /// Schreibweise in JSON und in Befehlen
    pub fn as_str(&self) -> &'static str {
        match self {
            BoardRepoSource::Local => "local",
            BoardRepoSource::Legacy => "legacy",
        }
    }
}

/// Lokale Zuordnung Board → Repo-Pfad; `None` heißt „ausdrücklich kein Pfad“ (verdrängt den
/// Altbestand)
pub trait BoardRepoStore: Send + Sync {
    fn read(&self) -> Result<BoardRepoEntries>;
    fn write(&self, entries: &BoardRepoEntries) -> Result<()>;
}

/// Zuordnung im Speicher (Tests und Laufzeiten ohne Benutzerordner)
#[derive(Debug, Default)]
pub struct MemoryBoardRepoStore {
    entries: Mutex<BoardRepoEntries>,
}

impl MemoryBoardRepoStore {
    /// Eine leere Zuordnung
    pub fn new() -> Self {
        Self::default()
    }

    /// Eine Zuordnung mit vorhandenen Einträgen
    pub fn with_entries(entries: BoardRepoEntries) -> Self {
        Self {
            entries: Mutex::new(entries),
        }
    }
}

impl BoardRepoStore for MemoryBoardRepoStore {
    fn read(&self) -> Result<BoardRepoEntries> {
        Ok(self.entries.lock().expect("Zuordnung ist nicht vergiftet").clone())
    }

    fn write(&self, entries: &BoardRepoEntries) -> Result<()> {
        *self.entries.lock().expect("Zuordnung ist nicht vergiftet") = entries.clone();
        Ok(())
    }
}

/// Die Datei `~/.harness/board-repos.yaml` als Speicher (P3-1)
impl BoardRepoStore for BoardRepoFile {
    fn read(&self) -> Result<BoardRepoEntries> {
        BoardRepoFile::read(self)
    }

    fn write(&self, entries: &BoardRepoEntries) -> Result<()> {
        BoardRepoFile::write(self, entries)
    }
}

/// Ein Board mit dem wirksamen Repo-Pfad des Benutzers und dessen Herkunft.
/// Entspricht `WithRepoSource<Board>` aus TypeScript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct BoardWithRepo {
    #[serde(flatten)]
    #[ts(flatten)]
    pub board: Board,
    pub repo_source: Option<BoardRepoSource>,
}

impl std::ops::Deref for BoardWithRepo {
    type Target = Board;

    fn deref(&self) -> &Board {
        &self.board
    }
}

/// Alles, worauf sich ein wirksamer Repo-Pfad setzen lässt (Board und Board-Übersicht)
pub trait RepoPathTarget {
    fn board_id(&self) -> i32;
    fn owner_id(&self) -> &str;
    fn repo_path(&self) -> Option<&str>;
    fn set_repo_path(&mut self, path: Option<String>);
    fn set_repo_source(&mut self, source: Option<BoardRepoSource>);
}

impl RepoPathTarget for BoardWithRepo {
    fn board_id(&self) -> i32 {
        self.board.id
    }

    fn owner_id(&self) -> &str {
        &self.board.owner_id
    }

    fn repo_path(&self) -> Option<&str> {
        self.board.repo_path.as_deref()
    }

    fn set_repo_path(&mut self, path: Option<String>) {
        self.board.repo_path = path;
    }

    fn set_repo_source(&mut self, source: Option<BoardRepoSource>) {
        self.repo_source = source;
    }
}

/// Setzt den wirksamen Pfad jedes Boards.
#[derive(Clone)]
pub struct BoardRepoPaths {
    store: Arc<dyn BoardRepoStore>,
    user_id: Option<String>,
}

impl BoardRepoPaths {
    /// Zuordnung aus diesem Speicher für diesen Benutzer; ohne Benutzer gibt es keinen
    /// Rückfall auf den Altbestand
    pub fn new(store: Arc<dyn BoardRepoStore>, user_id: Option<String>) -> Self {
        Self { store, user_id }
    }

    /// Zuordnung im Speicher ohne Benutzer – der Standard, wenn kein Benutzerordner im Spiel ist
    pub fn memory() -> Self {
        Self::new(Arc::new(MemoryBoardRepoStore::new()), None)
    }

    /// Boards mit wirksamem Pfad und dessen Herkunft
    pub fn apply(&self, boards: Vec<Board>) -> Result<Vec<BoardWithRepo>> {
        let mut result: Vec<BoardWithRepo> = boards
            .into_iter()
            .map(|board| BoardWithRepo {
                board,
                repo_source: None,
            })
            .collect();
        self.apply_to(&mut result)?;
        Ok(result)
    }

    /// Ein Board mit wirksamem Pfad
    pub fn one(&self, board: Board) -> Result<BoardWithRepo> {
        Ok(self.apply(vec![board])?.remove(0))
    }

    /// Setzt den wirksamen Pfad auf allem, was einen trägt (etwa Board-Übersichten)
    pub fn apply_to<T: RepoPathTarget>(&self, targets: &mut [T]) -> Result<()> {
        if targets.is_empty() {
            return Ok(());
        }
        let entries = self.store.read()?;
        for target in targets {
            self.resolve(target, &entries);
        }
        Ok(())
    }

    /// Setzt den eigenen Pfad des Boards; leer oder `None` hinterlegt „kein Pfad“
    pub fn set(&self, board_id: i32, path: Option<&str>) -> Result<()> {
        let mut entries = self.store.read()?;
        let value = path
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_string);
        entries.insert(i64::from(board_id), value);
        self.store.write(&entries)
    }

    /// Wirksamer Pfad eines Boards aus einer gelesenen Zuordnung
    fn resolve<T: RepoPathTarget>(&self, target: &mut T, entries: &BoardRepoEntries) {
        if let Some(path) = entries.get(&i64::from(target.board_id())) {
            let source = path.as_ref().map(|_| BoardRepoSource::Local);
            target.set_repo_path(path.clone());
            target.set_repo_source(source);
            return;
        }
        let legacy = self.user_id.as_deref() == Some(target.owner_id()) && target.repo_path().is_some();
        if legacy {
            target.set_repo_source(Some(BoardRepoSource::Legacy));
            return;
        }
        target.set_repo_path(None);
        target.set_repo_source(None);
    }
}

impl std::fmt::Debug for BoardRepoPaths {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoardRepoPaths")
            .field("user_id", &self.user_id)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "11111111-1111-1111-1111-111111111111";
    const OTHER: &str = "22222222-2222-2222-2222-222222222222";

    /// Board mit Besitzer und DB-Wert des Repo-Pfads
    fn board(id: i32, owner_id: &str, repo_path: Option<&str>) -> Board {
        Board {
            id,
            project_id: 1,
            owner_id: owner_id.to_string(),
            name: format!("B{id}"),
            repo_path: repo_path.map(str::to_string),
            created_at: String::new(),
        }
    }

    /// Zuordnung mit diesen Einträgen für den Benutzer `ME`
    fn paths(entries: &[(i64, Option<&str>)], user_id: Option<&str>) -> BoardRepoPaths {
        let entries: BoardRepoEntries = entries
            .iter()
            .map(|(id, path)| (*id, path.map(str::to_string)))
            .collect();
        BoardRepoPaths::new(
            Arc::new(MemoryBoardRepoStore::with_entries(entries)),
            user_id.map(str::to_string),
        )
    }

    #[test]
    fn nimmt_den_lokalen_eintrag_auch_fur_fremde_boards() {
        let paths = paths(&[(1, Some("C:/mein")), (2, Some("D:/kollege"))], Some(ME));
        let result = paths
            .apply(vec![
                board(1, ME, Some("C:/alt")),
                board(2, OTHER, Some("X:/fremd")),
            ])
            .unwrap();

        assert_eq!(result[0].repo_path.as_deref(), Some("C:/mein"));
        assert_eq!(result[0].repo_source, Some(BoardRepoSource::Local));
        assert_eq!(result[1].repo_path.as_deref(), Some("D:/kollege"));
        assert_eq!(result[1].repo_source, Some(BoardRepoSource::Local));
    }

    #[test]
    fn fallt_nur_bei_eigenen_boards_auf_den_db_wert_zuruck() {
        let paths = paths(&[], Some(ME));
        let result = paths
            .apply(vec![
                board(1, ME, Some("C:/alt")),
                board(2, OTHER, Some("X:/fremd")),
            ])
            .unwrap();

        assert_eq!(result[0].repo_path.as_deref(), Some("C:/alt"));
        assert_eq!(result[0].repo_source, Some(BoardRepoSource::Legacy));
        assert_eq!(result[1].repo_path, None);
        assert_eq!(result[1].repo_source, None);
    }

    #[test]
    fn ohne_benutzer_gibt_es_keinen_ruckfall() {
        let result = paths(&[], None)
            .apply(vec![board(1, ME, Some("C:/alt"))])
            .unwrap();

        assert_eq!(result[0].repo_path, None);
        assert_eq!(result[0].repo_source, None);
    }

    #[test]
    fn setzt_und_entfernt_den_eigenen_pfad_entfernt_verdrangt_auch_den_altbestand() {
        let store = Arc::new(MemoryBoardRepoStore::new());
        let paths = BoardRepoPaths::new(store.clone(), Some(ME.to_string()));

        paths.set(1, Some("  C:/neu  ")).unwrap();
        let one = paths.one(board(1, ME, Some("C:/alt"))).unwrap();
        assert_eq!(one.repo_path.as_deref(), Some("C:/neu"));
        assert_eq!(one.repo_source, Some(BoardRepoSource::Local));

        paths.set(1, None).unwrap();
        let one = paths.one(board(1, ME, Some("C:/alt"))).unwrap();
        assert_eq!(one.repo_path, None);
        assert_eq!(one.repo_source, None);

        paths.set(1, Some(" ")).unwrap();
        assert_eq!(store.read().unwrap(), BoardRepoEntries::from([(1, None)]));
    }

    #[test]
    fn eine_leere_liste_liest_die_zuordnung_nicht() {
        assert!(BoardRepoPaths::memory().apply(Vec::new()).unwrap().is_empty());
    }
}
