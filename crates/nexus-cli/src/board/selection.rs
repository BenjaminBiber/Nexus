//! Welches Board ein Befehl meint: `--board <id|all>`, sonst das Board des Repos (wenn der
//! Ordner in einem eingerichteten Repo mit zugeordnetem Board liegt), sonst das aktive Board
//! aus der Konfig. Portiert aus `packages/cli/src/board/BoardSelection.ts`.

use std::path::Path;

use nexus_core::board::board_service::BoardListFilter;
use nexus_core::board::config::BoardConfig;
use nexus_core::board::database::BoardDatabase;
use nexus_core::board::repo_board_resolver::RepoBoardOwner;
use nexus_core::board::services::BoardServices;
use nexus_core::{HarnessError, HarnessProject, Result};

/// Sucht das Board, auf dem ein Befehl arbeitet.
#[derive(Debug, Clone, Copy, Default)]
pub struct BoardSelection;

impl BoardSelection {
    /// Genau ein Board; `repo_dir` ist der Arbeitsordner, aus dem das Board des Repos ermittelt
    /// wird. Ein mit `--board` genanntes Board muss es geben (sonst `board-not-found`)
    pub async fn one<D: BoardDatabase>(
        &self,
        services: &BoardServices<D>,
        config: &BoardConfig,
        option: Option<&str>,
        repo_dir: Option<&Path>,
    ) -> Result<i32> {
        if let Some(option) = option {
            return Ok(services
                .boards
                .require(self.parse_id(option)?)
                .await?
                .board
                .id);
        }
        if let Some(dir) = repo_dir {
            if let Some(board) = self.repo_board(services, config, dir).await? {
                return Ok(board);
            }
        }
        let Some(active) = config.active_board else {
            return Err(HarnessError::new(
                "board-not-selected",
                "Kein aktives Board. Mit „harness board boards use <id>“ wählen oder --board <id> angeben.",
            )
            .into());
        };
        self.parse_id(&active.to_string())
    }

    /// Ein Board oder mit „all“ alle Boards
    pub async fn many<D: BoardDatabase>(
        &self,
        services: &BoardServices<D>,
        config: &BoardConfig,
        option: Option<&str>,
        repo_dir: Option<&Path>,
    ) -> Result<Vec<i32>> {
        if option == Some("all") {
            return Ok(services
                .boards
                .list(&BoardListFilter::default())
                .await?
                .into_iter()
                .map(|board| board.board.id)
                .collect());
        }
        Ok(vec![self.one(services, config, option, repo_dir).await?])
    }

    /// Board-Nummer aus Text
    pub fn parse_id(&self, raw: &str) -> Result<i32> {
        match raw.parse::<i32>() {
            Ok(id) if id >= 1 => Ok(id),
            _ => Err(HarnessError::new(
                "board-id-invalid",
                format!("„{raw}“ ist keine Board-Nummer."),
            )
            .into()),
        }
    }

    /// Board, das dem Hauptordner des eingerichteten Repos zugeordnet ist; außerhalb eines Repos
    /// oder ohne Zuordnung `None`
    async fn repo_board<D: BoardDatabase>(
        &self,
        services: &BoardServices<D>,
        config: &BoardConfig,
        dir: &Path,
    ) -> Result<Option<i32>> {
        let Ok(project) = HarnessProject::open(dir) else {
            return Ok(None);
        };
        let me = RepoBoardOwner {
            user_id: config.user_id.clone(),
            active_board: config.active_board,
        };
        Ok(services
            .boards
            .for_repo(&project.root, &me)
            .await?
            .map(|board| board.board.id))
    }
}

#[cfg(test)]
mod tests {
    use nexus_core::board::config::BoardProvider;
    use nexus_core::board::in_memory::InMemoryBoardDatabase;
    use nexus_core::clock::Clock;
    use nexus_core::testing::{BoardFixture, USER_ID, USER_NAME};

    use super::*;

    /// Dienste im Speicher samt Konfig, die das Board der Umgebung als aktiv führt
    async fn dienste() -> (BoardServices<InMemoryBoardDatabase>, BoardConfig, i32) {
        let f = BoardFixture::create().await;
        let services = BoardServices::new(f.db.clone(), Clock::system(), f.repo_paths.clone());
        let config = BoardConfig {
            active_board: Some(f.board_id),
            ..konfig()
        };
        (services, config, f.board_id)
    }

    /// Eine Konfig, von der nur Benutzer und aktives Board zählen
    fn konfig() -> BoardConfig {
        BoardConfig {
            provider: BoardProvider::Sqlite,
            file: String::new(),
            server: String::new(),
            port: 1433,
            database: String::new(),
            user: String::new(),
            password: String::new(),
            encrypt: false,
            trust_server_certificate: true,
            user_id: USER_ID.to_string(),
            user_name: USER_NAME.to_string(),
            active_board: None,
        }
    }

    #[tokio::test]
    async fn nimmt_ein_vorhandenes_board_aus_board_sonst_das_aktive_board() {
        let (services, config, id) = dienste().await;

        assert_eq!(
            BoardSelection
                .one(&services, &config, Some(&id.to_string()), None)
                .await
                .unwrap(),
            id
        );
        assert_eq!(
            BoardSelection
                .one(&services, &config, None, None)
                .await
                .unwrap(),
            id
        );
        assert_eq!(
            BoardSelection
                .many(&services, &config, Some("all"), None)
                .await
                .unwrap(),
            [id]
        );
    }

    #[tokio::test]
    async fn lehnt_ein_unbekanntes_board_aus_board_mit_board_not_found_ab() {
        let (services, config, _) = dienste().await;

        let error = BoardSelection
            .one(&services, &config, Some("99"), None)
            .await
            .unwrap_err();
        assert_eq!(error.code(), Some("board-not-found"));
        assert_eq!(error.to_string(), "Board 99 gibt es nicht.");
        assert_eq!(
            BoardSelection
                .many(&services, &config, Some("99"), None)
                .await
                .unwrap_err()
                .code(),
            Some("board-not-found")
        );
        assert_eq!(
            BoardSelection
                .one(&services, &config, Some("x"), None)
                .await
                .unwrap_err()
                .code(),
            Some("board-id-invalid")
        );
    }

    #[tokio::test]
    async fn ohne_aktives_board_meldet_die_auswahl_board_not_selected() {
        let (services, _, _) = dienste().await;

        let error = BoardSelection
            .one(&services, &konfig(), None, None)
            .await
            .unwrap_err();

        assert_eq!(error.code(), Some("board-not-selected"));
        assert_eq!(
            error.to_string(),
            "Kein aktives Board. Mit „harness board boards use <id>“ wählen oder --board <id> angeben."
        );
    }
}
