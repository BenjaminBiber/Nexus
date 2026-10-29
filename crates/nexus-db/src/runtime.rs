//! Laufzeit des Boards: Konfig lesen, verbinden, Schema prüfen, Benutzer anlegen, Dienste
//! bereitstellen. Portiert aus `packages/db/src/BoardRuntime.ts`.

use std::sync::Arc;

use nexus_core::board::basics::BoardActor;
use nexus_core::board::config::BoardConfig;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::board::database::BoardDatabase;
use nexus_core::board::repo_file::BoardRepoFile;
use nexus_core::board::repo_paths::BoardRepoPaths;
use nexus_core::board::services::BoardServices;
use nexus_core::clock::Clock;
use nexus_core::{HarnessHome, Result};

use crate::factory::{open_checked, AnyBoardDatabase};
use crate::managed::ConnectOptions;

/// Die geöffnete Board-Datenbank samt ihren Diensten, der Konfig und dem Handelnden.
pub struct BoardRuntime {
    /// Verbindung und Identität aus `board.yaml`
    pub config: BoardConfig,
    /// Alle Dienste des Boards über dieser Datenbank
    pub services: BoardServices<AnyBoardDatabase>,
    /// Erweiterungen, die dem Schema auch nach dem Einspielen fehlen
    pub missing_features: Vec<String>,
}

impl BoardRuntime {
    /// Öffnet das Board mit `<HARNESS_HOME>/board.yaml`; eigene Repo-Pfade aus
    /// `<HARNESS_HOME>/board-repos.yaml`
    pub async fn open(home: &HarnessHome, options: ConnectOptions) -> Result<Self> {
        let config = BoardConfigStore::new(home.clone()).require()?;
        Self::connect(config, options, home).await
    }

    /// Verbindet mit einer gegebenen Konfig (auch für den Verbindungstest der Oberfläche)
    pub async fn connect(
        config: BoardConfig,
        options: ConnectOptions,
        home: &HarnessHome,
    ) -> Result<Self> {
        let opened = open_checked(&config, options).await?;
        let repo_paths = BoardRepoPaths::new(
            Arc::new(BoardRepoFile::new(home.clone())),
            Some(config.user_id.clone()),
        );
        let services = BoardServices::new(Arc::new(opened.db), Clock::system(), repo_paths);
        Ok(Self {
            config,
            services,
            missing_features: opened.missing_features,
        })
    }

    /// Handelnder: eigene GUID, Name aus `--by`/`--author`/`--agent` oder der Anzeigename
    pub fn actor(&self, name: Option<&str>) -> BoardActor {
        let name = name
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.config.user_name);
        BoardActor {
            user_id: self.config.user_id.clone(),
            name: name.to_string(),
        }
    }

    /// Schließt die Verbindung
    pub async fn close(&self) -> Result<()> {
        self.services.db.close().await
    }
}

#[cfg(test)]
mod tests {
    use nexus_core::board::basics::TicketStatus;
    use nexus_core::board::ticket_service::{NewTicketInput, WriteOptions};

    use super::*;

    /// Ein Home mit einer `board.yaml` auf eine frische SQLite-Datei
    fn home_mit_sqlite() -> (tempfile::TempDir, HarnessHome) {
        let dir = tempfile::Builder::new()
            .prefix("harness-runtime-")
            .tempdir()
            .expect("Temp-Ordner");
        let home = HarnessHome::at(dir.path());
        std::fs::create_dir_all(home.dir()).expect("Home-Ordner");
        std::fs::write(
            home.board_config_file(),
            format!(
                "provider: sqlite\nfile: {}\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: Vertrag\n",
                home.board_db_file().display()
            ),
        )
        .expect("board.yaml");
        (dir, home)
    }

    #[tokio::test]
    async fn offnet_die_datenbank_der_konfig_und_stellt_die_dienste_bereit() {
        let (_dir, home) = home_mit_sqlite();

        let rt = BoardRuntime::open(&home, ConnectOptions::default())
            .await
            .expect("die Laufzeit öffnet");

        let actor = rt.actor(Some("claude"));
        rt.services
            .projects
            .create("ARC", "Arches", &actor)
            .await
            .expect("Projekt");
        let board = rt
            .services
            .boards
            .create("ARC", "Sprint", &actor)
            .await
            .expect("Board");
        let ticket = rt
            .services
            .tickets
            .create(
                board.board.id,
                &NewTicketInput::titled("Login bauen"),
                &actor,
                &WriteOptions::default(),
            )
            .await
            .expect("Ticket");

        assert_eq!(ticket.ticket.id, "ARC-0001");
        assert_eq!(ticket.ticket.status, TicketStatus::Backlog);
        assert!(rt.missing_features.is_empty(), "{:?}", rt.missing_features);
        rt.close().await.expect("die Laufzeit schließt");
    }

    #[tokio::test]
    async fn der_handelnde_tragt_die_eigene_guid_und_den_namen_der_option() {
        let (_dir, home) = home_mit_sqlite();
        let rt = BoardRuntime::open(&home, ConnectOptions::default())
            .await
            .expect("die Laufzeit öffnet");

        assert_eq!(
            rt.actor(Some("claude")),
            BoardActor {
                user_id: "11111111-1111-1111-1111-111111111111".into(),
                name: "claude".into(),
            }
        );
        // Leer oder nicht angegeben: der Anzeigename aus der Konfig
        assert_eq!(rt.actor(Some("  ")).name, "Vertrag");
        assert_eq!(rt.actor(None).name, "Vertrag");
        rt.close().await.expect("die Laufzeit schließt");
    }

    #[tokio::test]
    async fn ohne_board_yaml_meldet_das_offnen_board_config_missing() {
        let dir = tempfile::Builder::new()
            .prefix("harness-runtime-")
            .tempdir()
            .expect("Temp-Ordner");
        let home = HarnessHome::at(dir.path());

        // `BoardRuntime` ist nicht `Debug` (die Datenbank ist es nicht), deshalb von Hand
        let Err(error) = BoardRuntime::open(&home, ConnectOptions::default()).await else {
            panic!("ohne Konfig gibt es keine Laufzeit");
        };

        assert_eq!(error.code(), Some("board-config-missing"));
    }
}
