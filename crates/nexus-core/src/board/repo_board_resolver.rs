//! Ordnet einem Git-Repo (Hauptordner) sein Board über den wirksamen Repo-Pfad zu.
//! Portiert aus `packages/core/src/board/RepoBoardResolver.ts`.

use crate::board::repo_paths::BoardWithRepo;

/// Wer fragt: eigener Benutzer und aktives Board aus `board.yaml`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoBoardOwner {
    pub user_id: String,
    pub active_board: Option<i32>,
}

/// Sucht das Board zu einem Repo-Ordner.
#[derive(Debug, Clone, Copy)]
pub struct RepoBoardResolver {
    /// Unter Windows zählt die Schreibweise der Pfade nicht
    windows: bool,
}

impl Default for RepoBoardResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl RepoBoardResolver {
    /// Für die Plattform, auf der der Prozess läuft (`process.platform` in TypeScript)
    pub fn new() -> Self {
        Self {
            windows: cfg!(windows),
        }
    }

    /// Für eine bestimmte Plattform (Tests)
    pub fn for_windows(windows: bool) -> Self {
        Self { windows }
    }

    /// Schrägstriche statt Rückstriche, ohne Endstrich; unter Windows ohne Groß-/Kleinschreibung
    pub fn normalize(&self, path: &str) -> String {
        let cleaned = path.trim().replace('\\', "/");
        let cleaned = cleaned.trim_end_matches('/');
        if self.windows {
            cleaned.to_lowercase()
        } else {
            cleaned.to_string()
        }
    }

    /// Mehrere Treffer: eigenes aktives Board, sonst eigenes (kleinste ID), sonst das erste
    /// (kleinste ID); kein Treffer: `None`
    pub fn pick(
        &self,
        boards: Vec<BoardWithRepo>,
        repo_root: &str,
        me: &RepoBoardOwner,
    ) -> Option<BoardWithRepo> {
        let wanted = self.normalize(repo_root);
        let mut hits: Vec<BoardWithRepo> = boards
            .into_iter()
            .filter(|board| {
                board
                    .repo_path
                    .as_deref()
                    .is_some_and(|path| self.normalize(path) == wanted)
            })
            .collect();
        hits.sort_by_key(|board| board.id);
        let own = hits
            .iter()
            .position(|board| board.owner_id == me.user_id && Some(board.id) == me.active_board);
        let index = own
            .or_else(|| hits.iter().position(|board| board.owner_id == me.user_id))
            .or(if hits.is_empty() { None } else { Some(0) })?;
        Some(hits.swap_remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::types::Board;

    const ME: &str = "11111111-1111-1111-1111-111111111111";
    const OTHER: &str = "22222222-2222-2222-2222-222222222222";

    /// Board mit wirksamem Repo-Pfad
    fn board(id: i32, owner_id: &str, repo_path: Option<&str>) -> BoardWithRepo {
        BoardWithRepo {
            board: Board {
                id,
                project_id: 1,
                owner_id: owner_id.to_string(),
                name: format!("B{id}"),
                repo_path: repo_path.map(str::to_string),
                created_at: String::new(),
            },
            repo_source: None,
        }
    }

    /// Wer fragt
    fn me(user_id: &str, active_board: Option<i32>) -> RepoBoardOwner {
        RepoBoardOwner {
            user_id: user_id.to_string(),
            active_board,
        }
    }

    /// Die vier Boards des TypeScript-Tests
    fn boards() -> Vec<BoardWithRepo> {
        vec![
            board(5, OTHER, Some("C:/Repo")),
            board(3, ME, Some("c:\\repo\\")),
            board(4, ME, Some("C:/Repo")),
            board(9, ME, Some("C:/Anderes")),
        ]
    }

    #[test]
    fn vereinheitlicht_schragstriche_endstrich_und_unter_windows_die_schreibweise() {
        assert_eq!(
            RepoBoardResolver::for_windows(true).normalize("C:\\Projects\\Arches\\"),
            "c:/projects/arches"
        );
        assert_eq!(
            RepoBoardResolver::for_windows(false).normalize("/home/x/Repo/"),
            "/home/x/Repo"
        );
    }

    #[test]
    fn nimmt_das_eigene_aktive_board_sonst_das_eigene_sonst_das_erste() {
        let win = RepoBoardResolver::for_windows(true);
        let linux = RepoBoardResolver::for_windows(false);

        assert_eq!(
            win.pick(boards(), "C:/Repo", &me(ME, Some(4)))
                .map(|b| b.id),
            Some(4)
        );
        assert_eq!(
            win.pick(boards(), "C:/Repo", &me(ME, Some(9)))
                .map(|b| b.id),
            Some(3)
        );
        assert_eq!(
            win.pick(
                boards(),
                "C:/Repo",
                &me("33333333-3333-3333-3333-333333333333", None)
            )
            .map(|b| b.id),
            Some(3)
        );
        assert!(win.pick(boards(), "D:/Fremd", &me(ME, None)).is_none());
        assert!(linux
            .pick(
                vec![board(1, ME, Some("/home/Repo"))],
                "/home/repo",
                &me(ME, None)
            )
            .is_none());
    }
}

#[cfg(test)]
mod dienst_tests {
    use super::RepoBoardOwner;
    use crate::board::database::{BoardDatabase, NewBoard};
    use crate::board::types::BoardUser;
    use crate::testing::BoardFixture;
    use crate::tx;

    const OTHER: &str = "22222222-2222-2222-2222-222222222222";

    /// Wer fragt: der Benutzer der Umgebung ohne aktives Board
    fn me(f: &BoardFixture) -> RepoBoardOwner {
        RepoBoardOwner {
            user_id: f.actor.user_id.clone(),
            active_board: None,
        }
    }

    #[tokio::test]
    async fn findet_das_board_uber_den_dienst_und_meldet_ein_fehlendes_verstandlich() {
        let f = BoardFixture::create().await;
        f.boards
            .set_repo_path(f.board_id, Some("C:\\Projects\\Demo"))
            .await
            .unwrap();

        assert_eq!(
            f.boards
                .for_repo("C:/Projects/Demo", &me(&f))
                .await
                .unwrap()
                .map(|board| board.id),
            Some(f.board_id)
        );
        assert!(f
            .boards
            .for_repo("C:/Nirgends", &me(&f))
            .await
            .unwrap()
            .is_none());
        let error = f
            .boards
            .require_for_repo("C:/Nirgends", &me(&f))
            .await
            .unwrap_err();
        assert_eq!(error.code(), Some("board-repo-unknown"));
        assert!(error.to_string().contains("harness board boards repo"));
    }

    #[tokio::test]
    async fn ordnet_uber_den_eigenen_lokalen_pfad_zu_nie_uber_den_db_wert_eines_fremden_boards() {
        let f = BoardFixture::create().await;
        let project_id = f.project_id().await;
        let foreign = tx!(f.db, |u| {
            u.users()
                .upsert(&BoardUser {
                    id: OTHER.into(),
                    name: "Kollege".into(),
                })
                .await?;
            u.boards()
                .insert(&NewBoard {
                    project_id,
                    owner_id: OTHER.into(),
                    name: "Fremd".into(),
                    repo_path: Some("C:/Projects/Demo".into()),
                    created_at: String::new(),
                })
                .await
        })
        .await
        .unwrap();

        assert!(f
            .boards
            .for_repo("C:/Projects/Demo", &me(&f))
            .await
            .unwrap()
            .is_none());

        f.boards
            .set_repo_path(foreign.id, Some("C:/Projects/Demo"))
            .await
            .unwrap();
        assert_eq!(
            f.boards
                .for_repo("C:/Projects/Demo", &me(&f))
                .await
                .unwrap()
                .map(|board| board.id),
            Some(foreign.id)
        );

        f.boards
            .set_repo_path(f.board_id, Some("C:/Projects/Demo"))
            .await
            .unwrap();
        assert_eq!(
            f.boards
                .for_repo("C:/Projects/Demo", &me(&f))
                .await
                .unwrap()
                .map(|board| board.id),
            Some(f.board_id)
        );
    }
}
