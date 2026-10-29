//! Liest und schreibt `~/.nexus/board.yaml` und prüft die Pflichtfelder. Die Datei teilen
//! sich TypeScript und Rust, deshalb sind Schlüssel, Standardwerte, Fehlercodes und
//! Meldungen wortgleich zu `packages/core/src/board/BoardConfigStore.ts`.

use std::fs;
use std::path::{Path, PathBuf};

use serde_yaml_ng::{Mapping, Value};

use crate::board::config::{is_guid, BoardConfig, BoardProvider, BOARD_PROVIDERS};
use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;
use crate::text::join_names;

/// Pflichtfelder für SQL Server (Reihenfolge = Reihenfolge in der Meldung)
const MSSQL_REQUIRED: &[&str] = &["server", "database", "user", "password"];

/// Pflichtfelder für jeden Anbieter
const COMMON_REQUIRED: &[&str] = &["userId", "userName"];

/// Felder, die als Text gelten, auch wenn YAML sie als Zahl liest
const TEXT_FIELDS: &[&str] = &["server", "database", "user", "password", "userName"];

/// Standardport von SQL Server
const DEFAULT_PORT: u16 = 1433;

/// Liest und schreibt die Verbindung zur Board-Datenbank.
#[derive(Debug, Clone)]
pub struct BoardConfigStore {
    home: HarnessHome,
    writer: AtomicWriter,
}

impl BoardConfigStore {
    /// Konfig im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
        }
    }

    /// Pfad der Datei (für Meldungen)
    pub fn file(&self) -> PathBuf {
        self.home.board_config_file()
    }

    /// Liest die Konfig; None, wenn die Datei fehlt
    pub fn read(&self) -> Result<Option<BoardConfig>> {
        let file = self.file();
        if !file.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&file).map_err(Error::unexpected)?;
        let raw: Value = serde_yaml_ng::from_str(&text).map_err(|_| {
            HarnessError::new(
                "board-config-invalid",
                format!("{} ist kein gültiges YAML.", file.display()),
            )
        })?;
        Ok(Some(self.validate(&raw)?))
    }

    /// Liest die Konfig oder meldet, wie sie angelegt wird
    pub fn require(&self) -> Result<BoardConfig> {
        self.read()?.ok_or_else(|| {
            HarnessError::new(
                "board-config-missing",
                format!(
                    "{} fehlt – mit „nexus board“ die Oberfläche öffnen und unter Einstellungen → Board-Datenbank anlegen.",
                    self.file().display()
                ),
            )
            .into()
        })
    }

    /// Schreibt die Konfig atomar; der Ordner wird bei Bedarf angelegt
    pub fn write(&self, config: &BoardConfig) -> Result<()> {
        let raw = serde_yaml_ng::to_value(config).map_err(Error::unexpected)?;
        let checked = self.validate(&raw)?;
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        let yaml = serde_yaml_ng::to_string(&checked).map_err(Error::unexpected)?;
        self.writer.write(&self.file(), &yaml)
    }

    /// Prüft Pflichtfelder und Typen und ergänzt Standardwerte
    pub fn validate(&self, raw: &Value) -> Result<BoardConfig> {
        let mut data = match raw {
            Value::Mapping(mapping) => mapping.clone(),
            _ => Mapping::new(),
        };
        // Unquotierte Zahlen im YAML (z. B. password: 1234) gelten als Text
        for key in TEXT_FIELDS {
            if let Some(number) = data
                .get(*key)
                .and_then(Value::as_f64)
                .filter(|number| number.is_finite())
            {
                data.insert(Value::from(*key), Value::from(number_to_text(number)));
            }
        }
        let provider = self.provider(&data)?;
        self.require_fields(&data, provider)?;
        let user_id = text(&data, "userId");
        if !is_guid(&user_id) {
            return Err(self.invalid(format!(
                "userId in {} ist keine GUID.",
                self.file().display()
            )));
        }
        let port = self.port(&data, provider)?;
        let file = self.sqlite_file(&data, provider)?;
        let active_board = data
            .get("activeBoard")
            .filter(|value| !value.is_null())
            .and_then(js_number)
            .filter(|number| is_integer(*number))
            .and_then(|number| i32::try_from(number as i64).ok());
        Ok(BoardConfig {
            provider,
            file,
            server: text(&data, "server"),
            port,
            database: text(&data, "database"),
            user: text(&data, "user"),
            password: text(&data, "password"),
            encrypt: flag(&data, "encrypt"),
            trust_server_certificate: flag(&data, "trustServerCertificate"),
            user_id: user_id.to_lowercase(),
            user_name: text(&data, "userName"),
            active_board,
        })
    }

    /// Anbieter der Board-Datenbank; fehlt er, gilt SQL Server
    fn provider(&self, data: &Mapping) -> Result<BoardProvider> {
        let value = data.get("provider").filter(|value| !value.is_null());
        let Some(value) = value else {
            return Ok(BoardProvider::Mssql);
        };
        value
            .as_str()
            .and_then(|name| {
                BOARD_PROVIDERS
                    .into_iter()
                    .find(|provider| provider.as_str() == name)
            })
            .ok_or_else(|| {
                self.invalid(format!(
                    "provider in {} ist ungültig (erlaubt: {}).",
                    self.file().display(),
                    join_names(BOARD_PROVIDERS.map(|provider| provider.as_str()))
                ))
            })
    }

    /// Meldet alle fehlenden Pflichtfelder auf einmal
    fn require_fields(&self, data: &Mapping, provider: BoardProvider) -> Result<()> {
        let required: Vec<&str> = match provider {
            BoardProvider::Mssql => MSSQL_REQUIRED
                .iter()
                .chain(COMMON_REQUIRED)
                .copied()
                .collect(),
            BoardProvider::Sqlite => COMMON_REQUIRED.to_vec(),
        };
        let missing: Vec<&str> = required
            .into_iter()
            .filter(|key| {
                !data
                    .get(*key)
                    .and_then(Value::as_str)
                    .is_some_and(|text| !text.trim().is_empty())
            })
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        let label = match provider {
            BoardProvider::Mssql => " für SQL Server",
            BoardProvider::Sqlite => "",
        };
        Err(self.invalid(format!(
            "In {} fehlen{label}: {}.",
            self.file().display(),
            missing.join(", ")
        )))
    }

    /// Port der Datenbank; bei SQLite zählt ein ungültiger Port als nicht gesetzt
    fn port(&self, data: &Mapping, provider: BoardProvider) -> Result<u16> {
        let port = match data.get("port") {
            None => f64::from(DEFAULT_PORT),
            Some(value) => js_number(value).unwrap_or(f64::NAN),
        };
        let valid = is_integer(port) && (1.0..=65535.0).contains(&port);
        match (valid, provider) {
            (true, _) => Ok(port as u16),
            (false, BoardProvider::Sqlite) => Ok(DEFAULT_PORT),
            (false, BoardProvider::Mssql) => {
                Err(self.invalid(format!("port in {} ist ungültig.", self.file().display())))
            }
        }
    }

    /// Pfad der SQLite-Datei; fehlt er (oder ist er leer), gilt `<HARNESS_HOME>/board.db`.
    /// Relative Pfade gelten ab HARNESS_HOME, nicht ab dem Arbeitsordner.
    fn sqlite_file(&self, data: &Mapping, provider: BoardProvider) -> Result<String> {
        let value = data.get("file").filter(|value| !value.is_null());
        let text = value.and_then(Value::as_str).map(str::trim).unwrap_or("");
        if provider == BoardProvider::Sqlite && value.is_some() && text.is_empty() {
            return Err(self.invalid(format!(
                "file in {} ist leer – Pfad der SQLite-Datei angeben oder die Zeile weglassen (Standard: {}).",
                self.file().display(),
                self.home.board_db_file().display()
            )));
        }
        if text.is_empty() {
            return Ok(self.home.board_db_file().to_string_lossy().into_owned());
        }
        let path = Path::new(text);
        if path.is_absolute() {
            Ok(text.to_string())
        } else {
            Ok(self.home.dir().join(path).to_string_lossy().into_owned())
        }
    }

    /// Fehler mit dem Code, den auch TypeScript meldet
    fn invalid(&self, message: String) -> Error {
        HarnessError::new("board-config-invalid", message).into()
    }
}

/// Text unter dem Schlüssel; alles andere zählt wie in TypeScript als leerer Text
fn text(data: &Mapping, key: &str) -> String {
    data.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Schalter: fehlt er, gilt `true`; sonst zählt nur ein echtes `true`
fn flag(data: &Mapping, key: &str) -> bool {
    match data.get(key) {
        None => true,
        Some(value) => value.as_bool() == Some(true),
    }
}

/// Wert, den JavaScript `Number(x)` liefern würde; None steht für NaN
fn js_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::Bool(true) => Some(1.0),
        Value::Bool(false) | Value::Null => Some(0.0),
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                Some(0.0)
            } else {
                trimmed.parse::<f64>().ok()
            }
        }
        _ => None,
    }
}

/// Ganze Zahl wie `Number.isInteger(x)` in JavaScript
fn is_integer(number: f64) -> bool {
    number.is_finite() && number.fract() == 0.0
}

/// Zahl als Text, wie `String(x)` in JavaScript
fn number_to_text(number: f64) -> String {
    if is_integer(number) {
        format!("{}", number as i64)
    } else {
        format!("{number}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUID: &str = "11111111-1111-1111-1111-111111111111";

    /// Benutzerordner und Konfig darin
    fn store() -> (tempfile::TempDir, BoardConfigStore) {
        let dir = tempfile::Builder::new()
            .prefix("harness-boardcfg-")
            .tempdir()
            .unwrap();
        let store = BoardConfigStore::new(HarnessHome::at(dir.path()));
        (dir, store)
    }

    /// Vollständige, gültige Konfig
    fn valid(dir: &tempfile::TempDir) -> BoardConfig {
        BoardConfig {
            provider: BoardProvider::Mssql,
            file: dir
                .path()
                .join("harness-board.db")
                .to_string_lossy()
                .into_owned(),
            server: "dev-sql-5".into(),
            port: 1433,
            database: "AiHarness".into(),
            user: "harness".into(),
            password: "geheim".into(),
            encrypt: true,
            trust_server_certificate: true,
            user_id: GUID.into(),
            user_name: "Florian".into(),
            active_board: None,
        }
    }

    /// Rohform der Konfig als YAML-Wert
    fn raw(config: &BoardConfig) -> Mapping {
        match serde_yaml_ng::to_value(config).unwrap() {
            Value::Mapping(mapping) => mapping,
            other => panic!("Konfig ist keine Zuordnung: {other:?}"),
        }
    }

    /// Setzt einen Schlüssel in der Rohform
    fn with(mut data: Mapping, key: &str, value: Value) -> Value {
        data.insert(Value::from(key), value);
        Value::Mapping(data)
    }

    /// Entfernt einen Schlüssel aus der Rohform
    fn without(mut data: Mapping, key: &str) -> Value {
        data.remove(key);
        Value::Mapping(data)
    }

    /// Schreibt rohen YAML-Text in die Konfigdatei
    fn write_raw(dir: &tempfile::TempDir, content: &str) {
        fs::write(dir.path().join("board.yaml"), content).unwrap();
    }

    #[test]
    fn liefert_null_ohne_datei_und_require_nennt_den_weg_uber_die_einstellungen() {
        let (_dir, store) = store();

        assert_eq!(store.read().unwrap(), None);

        let error = store.require().unwrap_err();
        assert_eq!(error.code(), Some("board-config-missing"));
        assert!(error.to_string().contains("Einstellungen"), "{error}");
    }

    #[test]
    fn schreibt_yaml_und_liest_es_wieder() {
        let (dir, store) = store();
        let config = valid(&dir);

        store.write(&config).unwrap();

        let text = fs::read_to_string(dir.path().join("board.yaml")).unwrap();
        assert!(text.contains("server: dev-sql-5"), "{text}");
        assert!(text.contains("provider: mssql"), "{text}");
        assert_eq!(store.read().unwrap(), Some(config));
    }

    #[test]
    fn erganzt_standardwerte_fur_port_verschlusselung_und_aktives_board() {
        let (dir, store) = store();
        write_raw(
            &dir,
            "server: s\ndatabase: d\nuser: u\npassword: p\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: F\n",
        );

        let config = store.read().unwrap().unwrap();

        assert_eq!(config.port, 1433);
        assert!(config.encrypt);
        assert!(config.trust_server_certificate);
        assert_eq!(config.active_board, None);
    }

    #[test]
    fn nennt_fehlende_pflichtfelder() {
        let (dir, store) = store();
        write_raw(&dir, "server: s\n");

        let error = store.read().unwrap_err();

        assert_eq!(error.code(), Some("board-config-invalid"));
        assert!(error.to_string().contains("database"), "{error}");
        assert!(error.to_string().contains("SQL Server"), "{error}");
    }

    #[test]
    fn lehnt_eine_ungultige_benutzer_guid_ab() {
        let (dir, store) = store();
        let data = with(raw(&valid(&dir)), "userId", Value::from("abc"));

        let error = store.validate(&data).unwrap_err();

        assert_eq!(error.code(), Some("board-config-invalid"));
        assert!(error.to_string().contains("GUID"), "{error}");
    }

    #[test]
    fn ladt_eine_bestehende_board_yaml_ohne_provider_als_mssql_mit_standarddatei() {
        let (dir, store) = store();
        write_raw(
            &dir,
            "server: s\ndatabase: d\nuser: u\npassword: p\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: F\n",
        );

        let config = store.read().unwrap().unwrap();

        assert_eq!(config.provider, BoardProvider::Mssql);
        assert_eq!(config.file, dir.path().join("board.db").to_string_lossy());
    }

    #[test]
    fn ergibt_bei_provider_sqlite_ohne_file_den_standardpfad_unter_harness_home() {
        let (dir, store) = store();
        let data = with(raw(&valid(&dir)), "provider", Value::from("sqlite"));
        let data = match data {
            Value::Mapping(mapping) => without(mapping, "file"),
            other => other,
        };

        let config = store.validate(&data).unwrap();

        assert_eq!(config.provider, BoardProvider::Sqlite);
        assert_eq!(config.file, dir.path().join("board.db").to_string_lossy());
    }

    #[test]
    fn ubernimmt_einen_eigenen_pfad_relative_pfade_gelten_ab_harness_home() {
        let (dir, store) = store();
        let relativ = Path::new("daten").join("b.db");
        let data = with(
            raw(&valid(&dir)),
            "file",
            Value::from(relativ.to_string_lossy().into_owned()),
        );
        assert_eq!(
            store.validate(&data).unwrap().file,
            dir.path().join("daten").join("b.db").to_string_lossy()
        );

        let absolut = dir.path().join("x.db");
        let data = with(
            raw(&valid(&dir)),
            "file",
            Value::from(absolut.to_string_lossy().into_owned()),
        );
        assert_eq!(
            store.validate(&data).unwrap().file,
            absolut.to_string_lossy()
        );
    }

    #[test]
    fn lehnt_einen_unbekannten_provider_ab() {
        let (dir, store) = store();
        let data = with(raw(&valid(&dir)), "provider", Value::from("oracle"));

        let error = store.validate(&data).unwrap_err();

        assert_eq!(error.code(), Some("board-config-invalid"));
        assert!(error.to_string().contains("provider"), "{error}");
        assert!(error.to_string().contains("mssql, sqlite"), "{error}");
    }

    #[test]
    fn ubernimmt_unquotierte_zahlen_als_text_statt_sie_als_fehlend_zu_melden() {
        let (dir, store) = store();
        write_raw(
            &dir,
            "server: 10\ndatabase: 2024\nuser: 42\npassword: 1234\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: 7\n",
        );

        let config = store.read().unwrap().unwrap();

        assert_eq!(config.server, "10");
        assert_eq!(config.database, "2024");
        assert_eq!(config.user, "42");
        assert_eq!(config.password, "1234");
        assert_eq!(config.user_name, "7");
    }

    #[test]
    fn sqlite_ohne_server_database_user_und_password_ist_gultig() {
        let (dir, store) = store();
        let mut data = Mapping::new();
        data.insert(Value::from("provider"), Value::from("sqlite"));
        data.insert(Value::from("userId"), Value::from(GUID));
        data.insert(Value::from("userName"), Value::from("F"));

        let config = store.validate(&Value::Mapping(data)).unwrap();

        assert_eq!(
            config,
            BoardConfig {
                provider: BoardProvider::Sqlite,
                file: dir.path().join("board.db").to_string_lossy().into_owned(),
                server: String::new(),
                port: 1433,
                database: String::new(),
                user: String::new(),
                password: String::new(),
                encrypt: true,
                trust_server_certificate: true,
                user_id: GUID.into(),
                user_name: "F".into(),
                active_board: None,
            }
        );
    }

    #[test]
    fn sqlite_mit_leerem_file_ist_ungultig() {
        let (dir, store) = store();
        for leer in [Value::from(""), Value::from("   "), Value::from(5)] {
            let data = with(raw(&valid(&dir)), "provider", Value::from("sqlite"));
            let data = match data {
                Value::Mapping(mapping) => with(mapping, "file", leer.clone()),
                other => other,
            };

            let error = store.validate(&data).unwrap_err();

            assert_eq!(error.code(), Some("board-config-invalid"));
            assert!(error.to_string().contains("file"), "{error}");
        }
    }

    #[test]
    fn sqlite_behalt_vorhandene_sql_server_felder() {
        let (dir, store) = store();
        let data = with(raw(&valid(&dir)), "provider", Value::from("sqlite"));

        let config = store.validate(&data).unwrap();

        assert_eq!(config.server, "dev-sql-5");
        assert_eq!(config.password, "geheim");
    }

    #[test]
    fn sqlite_nennt_nur_die_eigenen_fehlenden_felder() {
        let (_dir, store) = store();
        let mut data = Mapping::new();
        data.insert(Value::from("provider"), Value::from("sqlite"));
        data.insert(Value::from("userName"), Value::from("F"));

        let error = store.validate(&Value::Mapping(data)).unwrap_err();

        assert_eq!(error.code(), Some("board-config-invalid"));
        assert!(error.to_string().contains("userId"), "{error}");
        assert!(!error.to_string().contains("server"), "{error}");
    }

    #[test]
    fn sqlite_ignoriert_einen_ungultigen_port() {
        let (_dir, store) = store();
        let mut data = Mapping::new();
        data.insert(Value::from("provider"), Value::from("sqlite"));
        data.insert(Value::from("userId"), Value::from(GUID));
        data.insert(Value::from("userName"), Value::from("F"));
        data.insert(Value::from("port"), Value::from("abc"));

        assert_eq!(store.validate(&Value::Mapping(data)).unwrap().port, 1433);
    }

    #[test]
    fn mssql_lehnt_einen_ungultigen_port_ab() {
        let (dir, store) = store();
        let data = with(raw(&valid(&dir)), "port", Value::from(0));

        let error = store.validate(&data).unwrap_err();

        assert_eq!(error.code(), Some("board-config-invalid"));
        assert!(error.to_string().contains("port"), "{error}");
    }

    #[test]
    fn sqlite_konfig_ubersteht_schreiben_und_lesen() {
        let (dir, store) = store();
        let config = BoardConfig {
            provider: BoardProvider::Sqlite,
            file: dir.path().join("b.db").to_string_lossy().into_owned(),
            server: String::new(),
            database: String::new(),
            user: String::new(),
            password: String::new(),
            ..valid(&dir)
        };

        store.write(&config).unwrap();

        assert_eq!(store.read().unwrap(), Some(config));
    }

    #[test]
    fn meldet_kaputtes_yaml() {
        let (dir, store) = store();
        write_raw(&dir, ": : [");

        let error = store.read().unwrap_err();

        assert_eq!(error.code(), Some("board-config-invalid"));
        assert!(error.to_string().contains("YAML"), "{error}");
    }

    #[test]
    fn ubernimmt_ein_aktives_board() {
        let (dir, store) = store();
        let data = with(raw(&valid(&dir)), "activeBoard", Value::from(7));

        assert_eq!(store.validate(&data).unwrap().active_board, Some(7));
    }
}
