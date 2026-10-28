//! Parametrisierte Abfragen auf einer SQLite-Verbindung; portiert aus
//! `packages/db/src/sqlite/SqliteRunner.ts`.
//!
//! Je Aufruf genau eine Anweisung (Mehranweisungs-Batches der Repositories in mehrere Aufrufe
//! zerlegen). Benannte Parameter heißen wie in SQL Server `@name`; SQLite kennt diese Form
//! nativ. Gebunden werden nur die Namen, die im SQL vorkommen: die Repositories geben oft mehr
//! mit, als eine Anweisung braucht. Fehlende Namen bleiben NULL.

use std::borrow::Cow;

use rusqlite::types::{ToSqlOutput, Value};
use rusqlite::{CachedStatement, Connection, Row, ToSql};

use nexus_core::{Error, HarnessError, Result};

use super::error_map;

/// Werte, die der Runner binden kann; entspricht `SqliteValue` aus TypeScript
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Text(String),
    Int(i64),
    Real(f64),
    Bool(bool),
    /// Zeitpunkt als Text; gebunden wird er immer als `YYYY-MM-DDTHH:MM:SS.mmmZ`, weil
    /// Datumsvergleiche in SQLite Textvergleiche sind
    Date(String),
}

/// Benannte Parameter einer Anweisung; der Name steht ohne `@` davor. Er darf geliehen sein
/// (Namen im Quelltext) oder erzeugt (IN-Listen wie `@i0`, `@i1`, …).
pub type Params = Vec<(Cow<'static, str>, SqlValue)>;

impl From<&str> for SqlValue {
    fn from(value: &str) -> Self {
        SqlValue::Text(value.to_string())
    }
}

impl From<String> for SqlValue {
    fn from(value: String) -> Self {
        SqlValue::Text(value)
    }
}

impl From<&String> for SqlValue {
    fn from(value: &String) -> Self {
        SqlValue::Text(value.clone())
    }
}

impl From<i64> for SqlValue {
    fn from(value: i64) -> Self {
        SqlValue::Int(value)
    }
}

impl From<i32> for SqlValue {
    fn from(value: i32) -> Self {
        SqlValue::Int(i64::from(value))
    }
}

impl From<f64> for SqlValue {
    fn from(value: f64) -> Self {
        SqlValue::Real(value)
    }
}

impl From<bool> for SqlValue {
    fn from(value: bool) -> Self {
        SqlValue::Bool(value)
    }
}

impl<T: Into<SqlValue>> From<Option<T>> for SqlValue {
    fn from(value: Option<T>) -> Self {
        value.map_or(SqlValue::Null, Into::into)
    }
}

impl ToSql for SqlValue {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(match self {
            SqlValue::Null => ToSqlOutput::Owned(Value::Null),
            SqlValue::Text(text) => ToSqlOutput::Borrowed(text.as_str().into()),
            SqlValue::Int(value) => ToSqlOutput::Owned(Value::Integer(*value)),
            SqlValue::Real(value) => ToSqlOutput::Owned(Value::Real(*value)),
            // boolean → 0/1 wie in TypeScript
            SqlValue::Bool(value) => ToSqlOutput::Owned(Value::Integer(i64::from(*value))),
            SqlValue::Date(text) => ToSqlOutput::Owned(Value::Text(iso_millis(text))),
        })
    }
}

/// Schreibt `vec![(Cow::Borrowed("id"), SqlValue::from(…)), …]` als
/// `sqlite_params!{ "id": …, "rev": … }`. Erzeugte Namen (IN-Listen) kommen als
/// `Cow::Owned` dazu.
#[macro_export]
macro_rules! sqlite_params {
    () => { ::std::vec::Vec::new() };
    ($($name:literal : $value:expr),+ $(,)?) => {
        ::std::vec![$((
            ::std::borrow::Cow::Borrowed($name),
            $crate::sqlite::SqlValue::from($value),
        )),+]
    };
}

/// Führt parametrisierte Abfragen auf einer SQLite-Verbindung aus; Gegenstück zum `SqlRunner`
/// von SQL Server.
///
/// Hält die Verbindung ausschließlich geliehen (`&mut`): `rusqlite::Connection` ist `Send`, aber
/// nicht `Sync`, und nur `&mut Connection` darf über ein `.await` hinweg gehalten werden.
pub struct SqliteRunner<'c> {
    conn: &'c mut Connection,
}

impl<'c> SqliteRunner<'c> {
    /// Nimmt die laufende Verbindung; die Transaktion klammert der Aufrufer
    pub fn new(conn: &'c mut Connection) -> Self {
        Self { conn }
    }

    /// Die Verbindung selbst – für Zugriffe, die über eine einzelne Anweisung hinausgehen
    pub fn connection(&mut self) -> &mut Connection {
        self.conn
    }

    /// Bindet die Parameter (`@name`) und liefert die Zeilen der Abfrage
    pub fn query_rows<T, F>(&mut self, sql: &str, params: &Params, mut map: F) -> Result<Vec<T>>
    where
        F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
    {
        single_statement(sql)?;
        let mut statement = self.conn.prepare_cached(sql).map_err(error_map::wrap)?;
        bind(&mut statement, params).map_err(error_map::wrap)?;
        let mut rows = statement.raw_query();
        let mut result = Vec::new();
        while let Some(row) = rows.next().map_err(error_map::wrap)? {
            result.push(map(row).map_err(error_map::wrap)?);
        }
        Ok(result)
    }

    /// Bindet die Parameter (`@name`) und liefert die Zahl der geänderten Zeilen – ohne die
    /// Änderungen der Trigger, damit optimistische Prüfungen richtig bleiben.
    ///
    /// Nur für ändernde Anweisungen ohne `RETURNING`; mit `RETURNING` ist
    /// [`SqliteRunner::query_rows`] zuständig.
    pub fn execute(&mut self, sql: &str, params: &Params) -> Result<usize> {
        single_statement(sql)?;
        let mut statement = self.conn.prepare_cached(sql).map_err(error_map::wrap)?;
        bind(&mut statement, params).map_err(error_map::wrap)?;
        statement.raw_execute().map_err(error_map::wrap)
    }
}

/// Bindet nur die Parameter, die im SQL vorkommen; unbekannte Namen bleiben unbeachtet,
/// fehlende Namen bleiben NULL
fn bind(statement: &mut CachedStatement<'_>, params: &Params) -> rusqlite::Result<()> {
    statement.clear_bindings();
    // Erst die Namen sammeln: `parameter_name` leiht die Anweisung, `raw_bind_parameter` braucht
    // sie ausschließlich
    let used: Vec<(usize, String)> = (1..=statement.parameter_count())
        .filter_map(|index| {
            statement
                .parameter_name(index)
                .map(|name| (index, name[1..].to_string()))
        })
        .collect();
    for (index, name) in used {
        if let Some((_, value)) = params.iter().find(|(key, _)| key.as_ref() == name) {
            statement.raw_bind_parameter(index, value)?;
        }
    }
    Ok(())
}

/// Meldet `sqlite-multi-statement`, wenn hinter dem ersten `;` noch etwas steht
fn single_statement(sql: &str) -> Result<()> {
    let plain = plain(sql);
    let after_semicolon = plain.split_once(';').map(|(_, rest)| rest.trim());
    if after_semicolon.is_some_and(|rest| !rest.is_empty()) {
        return Err(HarnessError::new(
            "sqlite-multi-statement",
            "SqliteRunner führt je Aufruf nur eine Anweisung aus; Batches in einzelne Aufrufe zerlegen.",
        )
        .into());
    }
    Ok(())
}

/// SQL ohne Kommentare und ohne den Inhalt von `'…'` und `"…"` – nur zum Erkennen von `;`
/// (Port von `SqliteRunner.plain`)
fn plain(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // Zeilenkommentar bis zum Zeilenende
            '-' if chars.peek() == Some(&'-') => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        break;
                    }
                }
                result.push_str(" \n");
            }
            // Blockkommentar bis zum passenden Ende
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut last = '\0';
                for next in chars.by_ref() {
                    if last == '*' && next == '/' {
                        break;
                    }
                    last = next;
                }
                result.push(' ');
            }
            // Text in Anführungszeichen; '' bzw. "" darin ist das Zeichen selbst
            '\'' | '"' => {
                while let Some(next) = chars.next() {
                    if next != c {
                        continue;
                    }
                    if chars.peek() == Some(&c) {
                        chars.next();
                        continue;
                    }
                    break;
                }
                result.push(c);
                result.push(c);
            }
            other => result.push(other),
        }
    }
    result
}

/// 0/1 aus einer INTEGER-Spalte als `bool`
pub fn bool_of(value: i64) -> bool {
    value != 0
}

/// Datumstext mit Millisekunden und `Z`, wie `datetime2(3)` in SQL Server und
/// `Date.toISOString()` in TypeScript. Eine nicht lesbare Eingabe bleibt unverändert.
pub fn iso_millis(value: &str) -> String {
    const MIT_MILLIS: &str = "%Y-%m-%dT%H:%M:%S%.3fZ";
    if let Ok(zeit) = chrono::DateTime::parse_from_rfc3339(value) {
        return zeit.to_utc().format(MIT_MILLIS).to_string();
    }
    // SQLite schreibt Zeitpunkte auch ohne Zonenangabe; sie gelten dann als UTC
    for form in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"] {
        if let Ok(zeit) = chrono::NaiveDateTime::parse_from_str(value, form) {
            return zeit.and_utc().format(MIT_MILLIS).to_string();
        }
    }
    value.to_string()
}

/// Datumstext aus der DB als ISO-Text mit Millisekunden; `None` bleibt `None`
pub fn iso(value: Option<String>) -> Option<String> {
    value.map(|text| iso_millis(&text))
}

/// GUIDs gelten im Harness kleingeschrieben (wie `SqlRunner.guid`)
pub fn guid(value: &str) -> String {
    value.to_lowercase()
}

/// Ein Zeitpunkt als Parameter; der Text wird beim Binden normalisiert
pub fn date(value: &str) -> SqlValue {
    SqlValue::Date(value.to_string())
}

/// Fehler, der zu einer Zeile nicht passt (etwa eine erwartete Zeile, die fehlt)
pub(crate) fn missing_row(what: &str) -> Error {
    Error::unexpected_message(format!("SQLite liefert keine Zeile für {what}."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kommentare_und_text_zahlen_nicht_als_anweisungsende() {
        assert_eq!(plain("SELECT ';' AS s"), "SELECT '' AS s");
        assert_eq!(plain("SELECT 1 -- ; zwei\n"), "SELECT 1  \n");
        assert_eq!(plain("SELECT /* ; */ 1"), "SELECT   1");
        assert_eq!(plain("SELECT 'a''b;'"), "SELECT ''");
        assert_eq!(plain("SELECT \"Plan\""), "SELECT \"\"");
    }

    #[test]
    fn nur_eine_anweisung_ist_erlaubt() {
        assert!(single_statement("SELECT 1").is_ok());
        assert!(single_statement("SELECT 1;").is_ok());
        assert!(single_statement("SELECT ';' AS s").is_ok());
        let fehler = single_statement("SELECT 1; SELECT 2").unwrap_err();
        assert_eq!(fehler.code(), Some("sqlite-multi-statement"));
    }

    #[test]
    fn datumstexte_bekommen_immer_millisekunden_und_z() {
        assert_eq!(
            iso_millis("2026-10-21T07:45:42.579Z"),
            "2026-10-21T07:45:42.579Z"
        );
        assert_eq!(
            iso_millis("2026-10-21T07:45:42Z"),
            "2026-10-21T07:45:42.000Z"
        );
        assert_eq!(
            iso_millis("2026-10-21T09:45:42+02:00"),
            "2026-10-21T07:45:42.000Z"
        );
        assert_eq!(
            iso_millis("2026-10-21T07:45:42.5"),
            "2026-10-21T07:45:42.500Z"
        );
        assert_eq!(
            iso_millis("2026-10-21 07:45:42"),
            "2026-10-21T07:45:42.000Z"
        );
        assert_eq!(iso_millis("kein Datum"), "kein Datum");
        assert_eq!(iso(None), None);
        assert_eq!(
            iso(Some("2026-10-21T07:45:42Z".into())),
            Some("2026-10-21T07:45:42.000Z".into())
        );
    }

    #[test]
    fn bool_und_guid_wie_in_typescript() {
        assert!(bool_of(1));
        assert!(!bool_of(0));
        assert!(bool_of(-1));
        assert_eq!(
            guid("ABCDEF00-0000-0000-0000-00000000000A"),
            "abcdef00-0000-0000-0000-00000000000a"
        );
    }

    #[test]
    fn werte_wandeln_sich_wie_in_typescript() {
        assert_eq!(SqlValue::from(true), SqlValue::Bool(true));
        assert_eq!(SqlValue::from(5i32), SqlValue::Int(5));
        assert_eq!(SqlValue::from("x"), SqlValue::Text("x".into()));
        assert_eq!(SqlValue::from(None::<i32>), SqlValue::Null);
        assert_eq!(SqlValue::from(Some(7i32)), SqlValue::Int(7));
        let params: Params = sqlite_params! { "id": "ARC-1", "rev": 2i32, "offen": None::<String> };
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].0, "id");
        assert_eq!(params[2].1, SqlValue::Null);
    }
}
