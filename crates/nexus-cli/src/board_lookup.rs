//! Sucht das Board des eigenen Repos, nach bestem Bemühen: jeder Fehler und jede
//! Zeitüberschreitung wird zum Hinweis, nie zum Abbruch. Portiert aus dem Board-Teil von
//! `packages/cli/src/commands/InitCommand.ts`.
//!
//! Die geprüfte Laufzeit (`BoardRuntime` in TypeScript: Schema-Version, fehlende
//! Erweiterungen, Benutzer anlegen) kommt mit P2-15 und P3-2; hier wird die Datenbank nur
//! gelesen, um den Namen des Boards zu nennen.

use std::sync::Arc;
use std::time::Duration;

use nexus_core::board::board_service::BoardService;
use nexus_core::board::config::{BoardConfig, BoardProvider};
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::board::database::BoardDatabase;
use nexus_core::board::repo_board_resolver::RepoBoardOwner;
use nexus_core::board::repo_file::BoardRepoFile;
use nexus_core::board::repo_paths::BoardRepoPaths;
use nexus_core::clock::Clock;
use nexus_core::{HarnessHome, Result};
use nexus_db::mssql::MssqlBoardDatabase;
use nexus_db::sqlite::SqliteBoardDatabase;
use nexus_db::ConnectOptions;
use serde::Serialize;

/// Längstens so lange wartet `init` auf die Board-Datenbank
pub const BOARD_TIMEOUT_MS: u64 = 3000;

/// Board des Repos, wie `init` es ausgibt
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BoardName {
    pub id: i32,
    pub name: String,
}

/// Ergebnis der Suche: entweder das Board oder ein Hinweis, warum es keines gibt
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoardInfo {
    pub board: Option<BoardName>,
    pub hint: Option<String>,
}

impl BoardInfo {
    /// Nur ein Hinweis, kein Board
    fn hint(text: impl Into<String>) -> Self {
        Self {
            board: None,
            hint: Some(text.into()),
        }
    }
}

/// Board des Repos über den eigenen Repo-Pfad (`board-repos.yaml`, sonst Altbestand), mit
/// Zeitlimit; Fehler und Zeitüberschreitung werden zum Hinweis
pub fn find_board(home: &HarnessHome, root: &str) -> BoardInfo {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return BoardInfo::hint(unavailable(&error.to_string())),
    };
    runtime.block_on(async {
        let limit = Duration::from_millis(BOARD_TIMEOUT_MS);
        match tokio::time::timeout(limit, lookup(home, root)).await {
            Ok(info) => info,
            Err(_) => BoardInfo::hint(format!(
                "Board-Datenbank antwortet nicht innerhalb von {} s; das Board dieses Repos zeigt „harness board boards list“.",
                BOARD_TIMEOUT_MS / 1000
            )),
        }
    })
}

/// Sucht das Board; jeder Fehler ergibt einen Hinweis statt eines Abbruchs
async fn lookup(home: &HarnessHome, root: &str) -> BoardInfo {
    let config = match BoardConfigStore::new(home.clone()).read() {
        Ok(Some(config)) => config,
        Ok(None) => {
            return BoardInfo::hint(
                "Board-Datenbank ist noch nicht eingerichtet: mit „harness board“ die Oberfläche öffnen, Einstellungen → Board-Datenbank.",
            )
        }
        Err(error) => return BoardInfo::hint(unavailable(&error.to_string())),
    };
    match for_repo(home, root, &config).await {
        Ok(Some(board)) => BoardInfo {
            board: Some(board),
            hint: None,
        },
        Ok(None) => BoardInfo::hint(format!(
            "Diesem Repo ist kein Board zugeordnet. Mit „harness board boards repo <id> {root}“ zuordnen."
        )),
        Err(error) => BoardInfo::hint(unavailable(&error.to_string())),
    }
}

/// Öffnet die Datenbank des Anbieters und fragt das Board des Repos ab
async fn for_repo(home: &HarnessHome, root: &str, config: &BoardConfig) -> Result<Option<BoardName>> {
    let options = ConnectOptions {
        connection_timeout_ms: Some(BOARD_TIMEOUT_MS),
    };
    match config.provider {
        BoardProvider::Sqlite => {
            let db = SqliteBoardDatabase::open(&config.file)?;
            ask(home, root, config, db).await
        }
        BoardProvider::Mssql => {
            let db = MssqlBoardDatabase::connect(config, options).await?;
            ask(home, root, config, db).await
        }
    }
}

/// Fragt den Board-Dienst und schließt die Verbindung danach immer
async fn ask<D: BoardDatabase>(
    home: &HarnessHome,
    root: &str,
    config: &BoardConfig,
    db: D,
) -> Result<Option<BoardName>> {
    let db = Arc::new(db);
    let paths = BoardRepoPaths::new(
        Arc::new(BoardRepoFile::new(home.clone())),
        Some(config.user_id.clone()),
    );
    let service = BoardService::new(db.clone(), Clock::system(), paths);
    let me = RepoBoardOwner {
        user_id: config.user_id.clone(),
        active_board: config.active_board,
    };
    let found = service.for_repo(root, &me).await;
    let _ = db.close().await;
    Ok(found?.map(|board| BoardName {
        id: board.board.id,
        name: board.board.name,
    }))
}

/// Hinweistext wie in TypeScript, wenn die Datenbank nicht antwortet
fn unavailable(message: &str) -> String {
    format!("Board-Datenbank nicht verfügbar: {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ohne_board_yaml_weist_die_suche_auf_die_einrichtung_hin() {
        let home = tempfile::Builder::new()
            .prefix("harness-board-")
            .tempdir()
            .unwrap();
        let info = find_board(&HarnessHome::at(home.path()), "/p");
        assert_eq!(info.board, None);
        assert!(
            info.hint
                .as_deref()
                .is_some_and(|hint| hint.contains("Board-Datenbank ist noch nicht eingerichtet")),
            "{info:?}"
        );
    }

    #[test]
    fn eine_leere_sqlite_datenbank_meldet_kein_zugeordnetes_board() {
        let home = tempfile::Builder::new()
            .prefix("harness-board-")
            .tempdir()
            .unwrap();
        let home = HarnessHome::at(home.path());
        std::fs::create_dir_all(home.dir()).unwrap();
        std::fs::write(
            home.board_config_file(),
            format!(
                "provider: sqlite\nfile: {}\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: Test\n",
                home.board_db_file().display()
            ),
        )
        .unwrap();

        let info = find_board(&home, "/p");

        assert_eq!(info.board, None);
        assert!(
            info.hint
                .as_deref()
                .is_some_and(|hint| hint.contains("kein Board zugeordnet")),
            "{info:?}"
        );
    }
}
