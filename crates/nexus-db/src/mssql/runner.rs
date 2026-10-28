//! Abfragen mit benannten Parametern auf SQL Server; portiert aus `packages/db/src/SqlRunner.ts`.
//!
//! # Warum hier umgeschrieben wird
//!
//! `mssql`/tedious kennt benannte Parameter (`@boardId`), tiberius nur positionale (`@P1`).
//! Damit die portierten SQL-Texte **wortgleich** aus TypeScript übernommen werden können,
//! schreibt [`rewrite_parameters`] die Namen vor dem Absetzen in `@P<n>` um und liefert die
//! Reihenfolge, in der die Werte zu binden sind.
//!
//! # Warum zwei Methoden statt einer
//!
//! In TypeScript liefert ein Aufruf beides: Zeilen *und* die Zahl der betroffenen Zeilen.
//! tiberius trennt das: `Client::query` liefert nur Zeilen, `Client::execute` nur
//! `rows_affected()`. Eine gemeinsame Methode müsste also immer beide Wege gehen oder einen
//! davon raten. Deshalb hat [`MssqlRunner`] zwei Methoden – [`MssqlRunner::query_rows`] für
//! Abfragen und [`MssqlRunner::execute`] für Änderungen. Wer beides aus *einer* Anweisung
//! braucht (`UPDATE … ; SELECT @@ROWCOUNT AS Affected;`), hängt die Zahl als Spalte an und
//! benutzt `query_rows`.

use std::borrow::Cow;
use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use nexus_core::{Error, Result};
use tiberius::numeric::Numeric;
use tiberius::{ColumnData, ToSql, Uuid};
use tokio::time::timeout;

use crate::mssql::error_map::MssqlErrorMap;

/// Zeitlimit einer einzelnen Abfrage; entspricht `requestTimeout: 30000` in TypeScript
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Verbindung, wie bb8-tiberius sie aus dem Pool gibt
pub type MssqlClient = bb8_tiberius::rt::Client;

// ---------------------------------------------------------------------------------------------
// Werte
// ---------------------------------------------------------------------------------------------

/// SQL-Typ eines Parameters ohne Wert (`NULL` braucht einen Typ, sonst weiß der Server nicht,
/// wogegen er vergleichen soll)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsType {
    /// `nvarchar(max)` – der Standard für Texte, damit Umlaute erhalten bleiben
    NVarChar,
    Int,
    BigInt,
    Float,
    Bit,
    /// `datetime2(3)`
    DateTime2,
    /// `decimal(p, scale)`
    Decimal(u8),
    /// `varchar(n)` – nur für die Spalten, die in der DB wirklich `varchar` sind
    VarChar,
    /// `uniqueidentifier`
    Guid,
}

/// Ein Wert, wie er an einen benannten Parameter gebunden wird. Die Varianten entsprechen den
/// Typen aus `SqlRunner.typeOf` samt den ausdrücklichen Typen der Repositories (bigint,
/// decimal(12,6), varchar(n)).
///
/// `DateTime2` hält die Zeit als [`NaiveDateTime`] in UTC und **nicht** als `DateTime<Utc>`:
/// tiberius bindet beide als `datetime2`, aber nur die naive Form bleibt von Zeitzonen
/// unberührt. Gebaut wird sie über [`MsValue::date`] aus einem ISO-Text.
#[derive(Debug, Clone, PartialEq)]
pub enum MsValue {
    /// `NULL` mit ausdrücklichem Typ
    Null(MsType),
    /// Text als `nvarchar`; Umlaute bleiben erhalten
    NVarChar(String),
    /// Text für eine `varchar`-Spalte (z. B. `ConfigHash varchar(64)`); der Treiber schickt ihn
    /// als `nvarchar`, SQL Server wandelt beim Schreiben um
    VarChar(String),
    Int(i32),
    BigInt(i64),
    Float(f64),
    Bit(bool),
    /// Zeitpunkt in UTC für eine `datetime2`-Spalte
    DateTime2(NaiveDateTime),
    /// Festkommazahl mit dieser Zahl von Nachkommastellen (z. B. `decimal(12,6)` → Skala 6)
    Decimal(f64, u8),
    /// `uniqueidentifier`
    Guid(Uuid),
}

impl MsValue {
    /// Text als `nvarchar(max)` – die Ableitung aus `SqlRunner.typeOf` für Zeichenketten
    pub fn text(value: impl Into<String>) -> Self {
        MsValue::NVarChar(value.into())
    }

    /// Text oder `NULL`
    pub fn text_opt(value: Option<&str>) -> Self {
        match value {
            Some(text) => MsValue::NVarChar(text.to_string()),
            None => MsValue::Null(MsType::NVarChar),
        }
    }

    /// ISO-Zeitpunkt (`2026-01-01T00:00:00.000Z`) als `datetime2`; entspricht `SqlRunner.date`
    pub fn date(iso: &str) -> Result<Self> {
        Ok(MsValue::DateTime2(parse_iso(iso)?))
    }

    /// ISO-Zeitpunkt oder `NULL`; entspricht `SqlRunner.date(null)`
    pub fn date_opt(iso: Option<&str>) -> Result<Self> {
        match iso {
            Some(iso) => MsValue::date(iso),
            None => Ok(MsValue::Null(MsType::DateTime2)),
        }
    }
}

/// ISO-Text (mit oder ohne Zeitzone) als Zeitpunkt in UTC
fn parse_iso(iso: &str) -> Result<NaiveDateTime> {
    if let Ok(mit_zone) = DateTime::parse_from_rfc3339(iso) {
        return Ok(mit_zone.with_timezone(&Utc).naive_utc());
    }
    for muster in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d"] {
        if let Ok(ohne_zone) = NaiveDateTime::parse_from_str(iso, muster) {
            return Ok(ohne_zone);
        }
    }
    if let Ok(nur_datum) = chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d") {
        return Ok(nur_datum.and_hms_opt(0, 0, 0).unwrap_or_default());
    }
    Err(Error::unexpected_message(format!(
        "„{iso}“ ist kein Zeitpunkt in ISO-Schreibweise."
    )))
}

impl ToSql for MsValue {
    fn to_sql(&self) -> ColumnData<'_> {
        match self {
            MsValue::NVarChar(text) | MsValue::VarChar(text) => {
                ColumnData::String(Some(Cow::Borrowed(text)))
            }
            MsValue::Int(value) => ColumnData::I32(Some(*value)),
            MsValue::BigInt(value) => ColumnData::I64(Some(*value)),
            MsValue::Float(value) => ColumnData::F64(Some(*value)),
            MsValue::Bit(value) => ColumnData::Bit(Some(*value)),
            MsValue::DateTime2(value) => value.to_sql(),
            MsValue::Decimal(value, scale) => ColumnData::Numeric(Some(numeric(*value, *scale))),
            MsValue::Guid(value) => ColumnData::Guid(Some(*value)),
            MsValue::Null(kind) => match kind {
                MsType::NVarChar | MsType::VarChar => ColumnData::String(None),
                MsType::Int => ColumnData::I32(None),
                MsType::BigInt => ColumnData::I64(None),
                MsType::Float => ColumnData::F64(None),
                MsType::Bit => ColumnData::Bit(None),
                MsType::DateTime2 => ColumnData::DateTime2(None),
                MsType::Decimal(_) => ColumnData::Numeric(None),
                MsType::Guid => ColumnData::Guid(None),
            },
        }
    }
}

/// Festkommazahl mit der verlangten Zahl von Nachkommastellen
fn numeric(value: f64, scale: u8) -> Numeric {
    let faktor = 10f64.powi(i32::from(scale));
    Numeric::new_with_scale((value * faktor).round() as i128, scale)
}

// ---------------------------------------------------------------------------------------------
// Zeilen
// ---------------------------------------------------------------------------------------------

/// Eine Zelle einer gelesenen Zeile. Die Varianten fassen zusammen, was das Board-Schema
/// hergibt; alle Ganzzahltypen landen in [`MsCell::Int`], Festkommazahlen in [`MsCell::Float`].
#[derive(Debug, Clone, PartialEq)]
pub enum MsCell {
    Null,
    Text(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    /// `datetime2`/`datetime` als Zeitpunkt in UTC
    DateTime(NaiveDateTime),
    /// `uniqueidentifier`, immer kleingeschrieben
    Guid(String),
    /// `rowversion`/`varbinary`
    Binary(Vec<u8>),
}

/// Eine Zeile, ansprechbar über die Spaltennamen – wie die Objekte, die `mssql` in TypeScript
/// liefert. Spaltennamen werden in der Schreibweise des SQL-Textes geführt.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MsRow(HashMap<String, MsCell>);

impl MsRow {
    /// Zeile aus Spaltenname und Zelle (für Tests und Ersatz-Runner)
    pub fn from_cells(cells: impl IntoIterator<Item = (String, MsCell)>) -> Self {
        MsRow(cells.into_iter().collect())
    }

    /// Die Zelle; fehlt die Spalte, ist das ein Fehler im SQL-Text und kein Datenfehler
    pub fn cell(&self, column: &str) -> Result<&MsCell> {
        self.0.get(column).ok_or_else(|| {
            Error::unexpected_message(format!("Die Abfrage liefert keine Spalte {column}."))
        })
    }

    /// Ob die Zeile diese Spalte hat
    pub fn has(&self, column: &str) -> bool {
        self.0.contains_key(column)
    }

    /// Text der Spalte; `NULL` ist ein Fehler
    pub fn get_str(&self, column: &str) -> Result<String> {
        self.get_opt_str(column)?
            .ok_or_else(|| fehlt(column, "ein Text"))
    }

    /// Text der Spalte oder `None` bei `NULL`
    pub fn get_opt_str(&self, column: &str) -> Result<Option<String>> {
        match self.cell(column)? {
            MsCell::Null => Ok(None),
            MsCell::Text(text) => Ok(Some(text.clone())),
            MsCell::Guid(guid) => Ok(Some(guid.clone())),
            MsCell::Int(value) => Ok(Some(value.to_string())),
            MsCell::Bool(value) => Ok(Some(i32::from(*value).to_string())),
            MsCell::Float(value) => Ok(Some(value.to_string())),
            _ => Err(falsch(column, "ein Text")),
        }
    }

    /// Ganzzahl der Spalte; `NULL` ist ein Fehler
    pub fn get_i32(&self, column: &str) -> Result<i32> {
        self.get_opt_i32(column)?
            .ok_or_else(|| fehlt(column, "eine Zahl"))
    }

    /// Ganzzahl der Spalte oder `None` bei `NULL`
    pub fn get_opt_i32(&self, column: &str) -> Result<Option<i32>> {
        match self.get_opt_i64(column)? {
            None => Ok(None),
            Some(value) => i32::try_from(value).map(Some).map_err(|_| {
                Error::unexpected_message(format!(
                    "Die Spalte {column} liefert {value}; das passt nicht in eine 32-Bit-Zahl."
                ))
            }),
        }
    }

    /// Ganzzahl der Spalte (`bigint`, `COUNT_BIG`); `NULL` ist ein Fehler
    pub fn get_i64(&self, column: &str) -> Result<i64> {
        self.get_opt_i64(column)?
            .ok_or_else(|| fehlt(column, "eine Zahl"))
    }

    /// Ganzzahl der Spalte oder `None` bei `NULL`
    pub fn get_opt_i64(&self, column: &str) -> Result<Option<i64>> {
        match self.cell(column)? {
            MsCell::Null => Ok(None),
            MsCell::Int(value) => Ok(Some(*value)),
            MsCell::Bool(value) => Ok(Some(i64::from(*value))),
            _ => Err(falsch(column, "eine Zahl")),
        }
    }

    /// Kommazahl der Spalte; `NULL` ist ein Fehler
    pub fn get_f64(&self, column: &str) -> Result<f64> {
        self.get_opt_f64(column)?
            .ok_or_else(|| fehlt(column, "eine Zahl"))
    }

    /// Kommazahl der Spalte oder `None` bei `NULL`
    pub fn get_opt_f64(&self, column: &str) -> Result<Option<f64>> {
        match self.cell(column)? {
            MsCell::Null => Ok(None),
            MsCell::Float(value) => Ok(Some(*value)),
            MsCell::Int(value) => Ok(Some(*value as f64)),
            _ => Err(falsch(column, "eine Zahl")),
        }
    }

    /// `bit` der Spalte; `NULL` ist ein Fehler
    pub fn get_bool(&self, column: &str) -> Result<bool> {
        self.get_opt_bool(column)?
            .ok_or_else(|| fehlt(column, "ein Ja/Nein"))
    }

    /// `bit` der Spalte oder `None` bei `NULL`
    pub fn get_opt_bool(&self, column: &str) -> Result<Option<bool>> {
        match self.cell(column)? {
            MsCell::Null => Ok(None),
            MsCell::Bool(value) => Ok(Some(*value)),
            MsCell::Int(value) => Ok(Some(*value != 0)),
            _ => Err(falsch(column, "ein Ja/Nein")),
        }
    }

    /// `datetime2` als ISO-Text mit Millisekunden und `Z`; entspricht `SqlRunner.iso`.
    /// `NULL` ist ein Fehler.
    pub fn get_iso(&self, column: &str) -> Result<String> {
        self.get_opt_iso(column)?
            .ok_or_else(|| fehlt(column, "ein Zeitpunkt"))
    }

    /// `datetime2` als ISO-Text oder `None` bei `NULL`
    pub fn get_opt_iso(&self, column: &str) -> Result<Option<String>> {
        match self.cell(column)? {
            MsCell::Null => Ok(None),
            MsCell::DateTime(value) => Ok(Some(to_iso(*value))),
            // Ein als Text gelesener Zeitpunkt (CONVERT(nvarchar, …)) wird wie ein ISO-Text behandelt
            MsCell::Text(text) => Ok(Some(to_iso(parse_iso(text)?))),
            _ => Err(falsch(column, "ein Zeitpunkt")),
        }
    }

    /// `uniqueidentifier` kleingeschrieben; entspricht `SqlRunner.guid`. `NULL` ist ein Fehler.
    pub fn get_guid(&self, column: &str) -> Result<String> {
        self.get_opt_guid(column)?
            .ok_or_else(|| fehlt(column, "eine GUID"))
    }

    /// `uniqueidentifier` kleingeschrieben oder `None` bei `NULL`
    pub fn get_opt_guid(&self, column: &str) -> Result<Option<String>> {
        Ok(self.get_opt_str(column)?.map(|guid| guid.to_lowercase()))
    }
}

/// ISO-Text mit Millisekunden und `Z`, wie `Date.toISOString()` in TypeScript
pub fn to_iso(value: NaiveDateTime) -> String {
    value.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Fehler: die Spalte ist `NULL`, obwohl ein Wert erwartet wird
fn fehlt(column: &str, erwartet: &str) -> Error {
    Error::unexpected_message(format!(
        "Die Spalte {column} ist NULL, erwartet wurde {erwartet}."
    ))
}

/// Fehler: die Spalte hat einen anderen Typ als erwartet
fn falsch(column: &str, erwartet: &str) -> Error {
    Error::unexpected_message(format!("Die Spalte {column} liefert nicht {erwartet}."))
}

/// Zelle aus dem, was tiberius gelesen hat
fn cell_of(data: &ColumnData<'static>) -> MsCell {
    match data {
        ColumnData::U8(None)
        | ColumnData::I16(None)
        | ColumnData::I32(None)
        | ColumnData::I64(None)
        | ColumnData::F32(None)
        | ColumnData::F64(None)
        | ColumnData::Bit(None)
        | ColumnData::String(None)
        | ColumnData::Guid(None)
        | ColumnData::Binary(None)
        | ColumnData::Numeric(None)
        | ColumnData::Xml(None)
        | ColumnData::DateTime(None)
        | ColumnData::SmallDateTime(None)
        | ColumnData::Time(None)
        | ColumnData::Date(None)
        | ColumnData::DateTime2(None)
        | ColumnData::DateTimeOffset(None) => MsCell::Null,
        ColumnData::U8(Some(value)) => MsCell::Int(i64::from(*value)),
        ColumnData::I16(Some(value)) => MsCell::Int(i64::from(*value)),
        ColumnData::I32(Some(value)) => MsCell::Int(i64::from(*value)),
        ColumnData::I64(Some(value)) => MsCell::Int(*value),
        ColumnData::F32(Some(value)) => MsCell::Float(f64::from(*value)),
        ColumnData::F64(Some(value)) => MsCell::Float(*value),
        ColumnData::Bit(Some(value)) => MsCell::Bool(*value),
        ColumnData::String(Some(text)) => MsCell::Text(text.to_string()),
        ColumnData::Guid(Some(guid)) => MsCell::Guid(guid.to_string().to_lowercase()),
        ColumnData::Binary(Some(bytes)) => MsCell::Binary(bytes.to_vec()),
        ColumnData::Numeric(Some(value)) => {
            MsCell::Float(value.value() as f64 / 10f64.powi(i32::from(value.scale())))
        }
        ColumnData::Xml(Some(xml)) => MsCell::Text(xml.to_string()),
        // Zeitpunkte liest tiberius über die chrono-Umsetzung; der Weg über `FromSql` ist hier
        // nicht möglich, weil die Zelle schon als `ColumnData` vorliegt
        ColumnData::DateTime(_)
        | ColumnData::SmallDateTime(_)
        | ColumnData::Time(_)
        | ColumnData::Date(_)
        | ColumnData::DateTime2(_)
        | ColumnData::DateTimeOffset(_) => time_cell(data),
    }
}

/// Zeitpunkt einer Zelle über die chrono-Umsetzung von tiberius
fn time_cell(data: &ColumnData<'static>) -> MsCell {
    use tiberius::FromSql;
    match NaiveDateTime::from_sql(data) {
        Ok(Some(value)) => MsCell::DateTime(value),
        _ => match chrono::NaiveDate::from_sql(data) {
            Ok(Some(datum)) => MsCell::DateTime(datum.and_hms_opt(0, 0, 0).unwrap_or_default()),
            _ => MsCell::Null,
        },
    }
}

/// Zeile aus dem, was tiberius gelesen hat
fn row_of(row: tiberius::Row) -> MsRow {
    MsRow(
        row.cells()
            .map(|(column, data)| (column.name().to_string(), cell_of(data)))
            .collect(),
    )
}

// ---------------------------------------------------------------------------------------------
// Umschreiben der benannten Parameter
// ---------------------------------------------------------------------------------------------

/// Zustand beim Lesen des SQL-Textes: nur außerhalb von Texten, Klammer-Namen und Kommentaren
/// wird ein `@name` ersetzt
enum Scan {
    Code,
    /// in `'…'`
    Text,
    /// in `[…]`
    Bracket,
    /// nach `--` bis zum Zeilenende
    LineComment,
    /// in `/* … */`; T-SQL erlaubt Verschachtelung, daher die Tiefe
    BlockComment(u32),
}

/// Gehört das Zeichen zu einem Parameternamen?
fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '#' || c == '$'
}

/// Schreibt die benannten Parameter (`@boardId`) in die positionalen von tiberius (`@P1`) um
/// und liefert dazu die Reihenfolge, in der die Werte zu binden sind (Indizes in `names`).
///
/// * Derselbe Name ergibt immer dieselbe Nummer.
/// * Namen, die im Text nicht vorkommen, werden nicht gebunden – sie fehlen in der Reihenfolge.
/// * `@@TRANCOUNT`, `@@ROWCOUNT` und andere Server-Variablen bleiben unberührt.
/// * In `'…'`, `[…]`, `-- …` und `/* … */` wird nichts ersetzt.
pub fn rewrite_parameters(sql: &str, names: &[&str]) -> (String, Vec<usize>) {
    let chars: Vec<char> = sql.chars().collect();
    let mut out = String::with_capacity(sql.len());
    let mut order: Vec<usize> = Vec::new();
    let mut state = Scan::Code;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match state {
            Scan::Code => {
                let next = chars.get(i + 1).copied();
                if c == '\'' {
                    state = Scan::Text;
                    out.push(c);
                    i += 1;
                } else if c == '[' {
                    state = Scan::Bracket;
                    out.push(c);
                    i += 1;
                } else if c == '-' && next == Some('-') {
                    state = Scan::LineComment;
                    out.push_str("--");
                    i += 2;
                } else if c == '/' && next == Some('*') {
                    state = Scan::BlockComment(1);
                    out.push_str("/*");
                    i += 2;
                } else if c == '@' && next == Some('@') {
                    // @@TRANCOUNT, @@ROWCOUNT …: Variablen des Servers, keine Parameter
                    out.push_str("@@");
                    i += 2;
                    while i < chars.len() && is_name_char(chars[i]) {
                        out.push(chars[i]);
                        i += 1;
                    }
                } else if c == '@' {
                    let start = i + 1;
                    let mut end = start;
                    while end < chars.len() && is_name_char(chars[end]) {
                        end += 1;
                    }
                    let name: String = chars[start..end].iter().collect();
                    match names.iter().position(|known| *known == name) {
                        Some(index) => {
                            let position = match order.iter().position(|bound| *bound == index) {
                                Some(position) => position,
                                None => {
                                    order.push(index);
                                    order.len() - 1
                                }
                            };
                            out.push_str("@P");
                            out.push_str(&(position + 1).to_string());
                        }
                        // Unbekannt: eine im Batch selbst erklärte Variable (DECLARE @x) – stehen lassen
                        None => {
                            out.push('@');
                            out.push_str(&name);
                        }
                    }
                    i = end;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            Scan::Text => {
                out.push(c);
                i += 1;
                if c == '\'' {
                    // Ein verdoppeltes '' beginnt hier sofort wieder einen Text – das Ergebnis
                    // ist dasselbe, weil dazwischen nichts ersetzt werden kann
                    state = Scan::Code;
                }
            }
            Scan::Bracket => {
                out.push(c);
                i += 1;
                if c == ']' {
                    state = Scan::Code;
                }
            }
            Scan::LineComment => {
                out.push(c);
                i += 1;
                if c == '\n' {
                    state = Scan::Code;
                }
            }
            Scan::BlockComment(depth) => {
                let next = chars.get(i + 1).copied();
                if c == '/' && next == Some('*') {
                    state = Scan::BlockComment(depth + 1);
                    out.push_str("/*");
                    i += 2;
                } else if c == '*' && next == Some('/') {
                    state = if depth <= 1 {
                        Scan::Code
                    } else {
                        Scan::BlockComment(depth - 1)
                    };
                    out.push_str("*/");
                    i += 2;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
        }
    }
    (out, order)
}

// ---------------------------------------------------------------------------------------------
// Der Runner
// ---------------------------------------------------------------------------------------------

/// Setzt parametrisierte Anweisungen ab. Die Repositories arbeiten nur gegen diesen Trait,
/// damit sie ohne SQL Server prüfbar sind (siehe [`crate::mssql::testing::FakeRunner`]).
#[async_trait]
pub trait MssqlRunner: Send {
    /// Zeilen aller Ergebnismengen der Anweisung
    async fn query_rows(&mut self, sql: &str, params: &[(&str, MsValue)]) -> Result<Vec<MsRow>>;

    /// Zahl der betroffenen Zeilen (Summe über alle Anweisungen des Textes)
    async fn execute(&mut self, sql: &str, params: &[(&str, MsValue)]) -> Result<u64>;
}

/// Der Runner über einer tiberius-Verbindung aus dem Pool.
///
/// Benannte Instanzen (`server\INSTANZ`) spielen hier keine Rolle: die Konfig von TypeScript
/// kennt nur Server und Port, und `BoardConfigStore` lässt nichts anderes zu.
pub struct TiberiusRunner<'c> {
    client: &'c mut MssqlClient,
    timeout: Duration,
}

impl<'c> TiberiusRunner<'c> {
    /// Runner über dieser Verbindung mit dem Abfrage-Zeitlimit von 30 s
    pub fn new(client: &'c mut MssqlClient) -> Self {
        Self::with_timeout(client, QUERY_TIMEOUT)
    }

    /// Runner mit eigenem Abfrage-Zeitlimit (für Tests)
    pub fn with_timeout(client: &'c mut MssqlClient, timeout: Duration) -> Self {
        Self { client, timeout }
    }
}

/// Fehler, wenn eine Anweisung ihr Zeitlimit überschreitet
fn zeitlimit(limit: Duration) -> Error {
    Error::unexpected_message(format!(
        "Die Anweisung an die Board-Datenbank hat das Zeitlimit von {} s überschritten.",
        limit.as_secs()
    ))
}

#[async_trait]
impl MssqlRunner for TiberiusRunner<'_> {
    async fn query_rows(&mut self, sql: &str, params: &[(&str, MsValue)]) -> Result<Vec<MsRow>> {
        let names: Vec<&str> = params.iter().map(|(name, _)| *name).collect();
        let (text, order) = rewrite_parameters(sql, &names);
        let bound: Vec<&dyn ToSql> = order
            .iter()
            .map(|index| &params[*index].1 as &dyn ToSql)
            .collect();
        let client = &mut *self.client;
        let results = timeout(self.timeout, async move {
            client
                .query(text.as_str(), &bound)
                .await?
                .into_results()
                .await
        })
        .await
        .map_err(|_| zeitlimit(self.timeout))?
        .map_err(MssqlErrorMap::map)?;
        Ok(results.into_iter().flatten().map(row_of).collect())
    }

    async fn execute(&mut self, sql: &str, params: &[(&str, MsValue)]) -> Result<u64> {
        let names: Vec<&str> = params.iter().map(|(name, _)| *name).collect();
        let (text, order) = rewrite_parameters(sql, &names);
        let bound: Vec<&dyn ToSql> = order
            .iter()
            .map(|index| &params[*index].1 as &dyn ToSql)
            .collect();
        let client = &mut *self.client;
        let result = timeout(self.timeout, async move {
            client.execute(text.as_str(), &bound).await
        })
        .await
        .map_err(|_| zeitlimit(self.timeout))?
        .map_err(MssqlErrorMap::map)?;
        Ok(result.rows_affected().iter().sum())
    }
}
