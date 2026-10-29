//! Verbindung zur Board-Datenbank und Identität dieses Rechners (Datei `~/.nexus/board.yaml`).
//! Hier stehen nur die Typen; gelesen und geschrieben wird die Datei später vom `BoardConfigStore`.

use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{Error, HarnessError};
use crate::text::join_names;

/// GUID in der Schreibweise der Konfig (Groß-/Kleinschreibung egal)
static GUID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?i)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").unwrap()
});

/// Ist der Text eine GUID, wie sie als Benutzer- oder Board-Kennung erwartet wird?
pub fn is_guid(value: &str) -> bool {
    GUID.is_match(value)
}

/// Art der Board-Datenbank: gemeinsamer SQL Server oder lokale SQLite-Datei (Einzelplatz)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardProvider {
    Mssql,
    Sqlite,
}

impl BoardProvider {
    /// Alle Anbieter in der Reihenfolge von TypeScript (`BOARD_PROVIDERS`)
    pub const ALL: [BoardProvider; 2] = [BoardProvider::Mssql, BoardProvider::Sqlite];

    /// Schreibweise in `board.yaml` und in JSON
    pub fn as_str(&self) -> &'static str {
        match self {
            BoardProvider::Mssql => "mssql",
            BoardProvider::Sqlite => "sqlite",
        }
    }
}

/// Alle Anbieter in der Reihenfolge von TypeScript (`BOARD_PROVIDERS`)
pub const BOARD_PROVIDERS: [BoardProvider; 2] = BoardProvider::ALL;

impl Default for BoardProvider {
    /// Fehlt der Anbieter in `board.yaml`, gilt SQL Server
    fn default() -> Self {
        BoardProvider::Mssql
    }
}

impl fmt::Display for BoardProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for BoardProvider {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        BoardProvider::ALL
            .into_iter()
            .find(|provider| provider.as_str() == input)
            .ok_or_else(|| {
                HarnessError::new(
                    "board-config-invalid",
                    format!(
                        "provider „{input}“ ist ungültig (erlaubt: {}).",
                        join_names(BoardProvider::ALL.map(|provider| provider.as_str()))
                    ),
                )
                .into()
            })
    }
}

/// Verbindung zur Board-Datenbank und Identität dieses Rechners
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardConfig {
    /// Anbieter der Board-Datenbank; fehlt er in `board.yaml`, gilt `mssql`
    pub provider: BoardProvider,
    /// Pfad der SQLite-Datei (nur bei Anbieter `sqlite` genutzt); Standard `<HARNESS_HOME>/board.db`
    pub file: String,
    pub server: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    pub encrypt: bool,
    pub trust_server_certificate: bool,
    /// GUID des Entwicklers; ordnet Boards, Kommentare und Verlauf zu
    pub user_id: String,
    pub user_name: String,
    /// Board, auf dem CLI-Befehle ohne `--board` arbeiten
    pub active_board: Option<i32>,
}

/// Von Hand geschrieben, damit das Passwort nicht in Protokollen und Fehlern landet
impl fmt::Debug for BoardConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let password = if self.password.is_empty() {
            "<leer>"
        } else {
            "<verborgen>"
        };
        f.debug_struct("BoardConfig")
            .field("provider", &self.provider)
            .field("file", &self.file)
            .field("server", &self.server)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("user", &self.user)
            .field("password", &password)
            .field("encrypt", &self.encrypt)
            .field("trust_server_certificate", &self.trust_server_certificate)
            .field("user_id", &self.user_id)
            .field("user_name", &self.user_name)
            .field("active_board", &self.active_board)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> BoardConfig {
        BoardConfig {
            provider: BoardProvider::Mssql,
            file: String::new(),
            server: "sql.example".into(),
            port: 1433,
            database: "harness".into(),
            user: "sa".into(),
            password: "geheim".into(),
            encrypt: true,
            trust_server_certificate: true,
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            user_name: "Benjamin".into(),
            active_board: None,
        }
    }

    #[test]
    fn anbieter_fahren_rund_und_sql_server_ist_der_standard() {
        assert_eq!(
            BOARD_PROVIDERS.map(|provider| provider.as_str()),
            ["mssql", "sqlite"]
        );
        assert_eq!(BoardProvider::default(), BoardProvider::Mssql);
        assert_eq!(
            "sqlite".parse::<BoardProvider>().unwrap(),
            BoardProvider::Sqlite
        );
        assert_eq!(
            serde_json::to_string(&BoardProvider::Sqlite).unwrap(),
            r#""sqlite""#
        );
    }

    #[test]
    fn unbekannter_anbieter_wird_abgelehnt() {
        let error = "postgres".parse::<BoardProvider>().unwrap_err();
        assert_eq!(error.code(), Some("board-config-invalid"));
        assert_eq!(
            error.to_string(),
            "provider „postgres“ ist ungültig (erlaubt: mssql, sqlite)."
        );
    }

    #[test]
    fn erkennt_guids_in_beiden_schreibweisen() {
        assert!(is_guid("11111111-1111-1111-1111-111111111111"));
        assert!(is_guid("AAAAAAAA-aaaa-AAAA-aaaa-AAAAAAAAAAAA"));
        assert!(!is_guid("11111111-1111-1111-1111-11111111111"));
        assert!(!is_guid("kein-guid"));
        assert!(!is_guid(" 11111111-1111-1111-1111-111111111111 "));
    }

    #[test]
    fn debug_zeigt_das_passwort_nicht() {
        let text = format!("{:?}", config());
        assert!(!text.contains("geheim"), "{text}");
        assert!(text.contains("password: \"<verborgen>\""), "{text}");
        assert!(text.contains("server: \"sql.example\""), "{text}");

        let ohne = BoardConfig {
            password: String::new(),
            ..config()
        };
        assert!(format!("{ohne:?}").contains("password: \"<leer>\""));
    }

    #[test]
    fn konfig_nutzt_die_schlussel_aus_board_yaml() {
        let json = serde_json::to_value(config()).unwrap();
        assert_eq!(json["trustServerCertificate"], serde_json::json!(true));
        assert_eq!(
            json["userId"],
            serde_json::json!("11111111-1111-1111-1111-111111111111")
        );
        assert_eq!(json["userName"], serde_json::json!("Benjamin"));
        assert_eq!(json["activeBoard"], serde_json::Value::Null);
        assert_eq!(
            serde_json::from_value::<BoardConfig>(json).unwrap(),
            config()
        );
    }
}
