//! Einspielen der additiven Erweiterungen (004, 005, …); portiert aus
//! `packages/db/src/MssqlSchemaRunner.ts`.
//!
//! # Warum die Skripte eingebettet sind
//!
//! In TypeScript sucht der Runner die Datei zur Laufzeit in mehreren Ordnern
//! (`HARNESS_SQL_DIR`, neben dem Programm, im Paket). In Rust liegen die Texte über
//! [`include_str!`] fest im Programm: ein Sidecar bringt dann kein zusätzliches `sql/`-Verzeichnis
//! mit und es gibt keinen Pfad, der auf einer der drei Plattformen anders aussieht. Die Quelle
//! bleibt dieselbe Datei unter `packages/db/sql`, damit alte und neue Fassung dasselbe einspielen.

use async_trait::async_trait;

use crate::schema_version_guard::{SchemaVersionGuard, REQUIRED_SCHEMA_FEATURES};

/// „Erweiterung ist schon da“ – zwei Sessions haben gleichzeitig eingespielt, das Ergebnis
/// stimmt trotzdem
const ALREADY_APPLIED: u32 = 50002;

/// „Vorbedingung fehlt“ – das vorige Skript ist noch nicht eingespielt
const PRECONDITION: u32 = 50001;

/// Fehlendes ALTER/CREATE-Recht auf dem Schema `board` (SQL Server meldet je nach Anweisung
/// eine andere Nummer)
const PERMISSION: [u32; 5] = [229, 230, 262, 2760, 15247];

/// Fehler eines Skript-Batches: die Nummer des SQL Servers, falls es eine gibt, und die Meldung.
/// So lässt sich der Runner ohne Treiber prüfen – `tiberius::error::TokenError` kann man von
/// außen nicht bauen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaBatchError {
    pub number: Option<u32>,
    pub message: String,
}

impl SchemaBatchError {
    /// Fehler mit Nummer des Servers
    pub fn numbered(number: u32, message: impl Into<String>) -> Self {
        Self {
            number: Some(number),
            message: message.into(),
        }
    }

    /// Fehler ohne Nummer (Netz, TLS, Protokoll)
    pub fn plain(message: impl Into<String>) -> Self {
        Self {
            number: None,
            message: message.into(),
        }
    }
}

impl From<tiberius::error::Error> for SchemaBatchError {
    fn from(error: tiberius::error::Error) -> Self {
        Self {
            number: error.code(),
            message: error.to_string(),
        }
    }
}

/// Nur das, was der Runner vom Verbindungspool braucht – so lässt er sich ohne SQL Server prüfen
#[async_trait]
pub trait SchemaBatchRunner: Send {
    /// Setzt den ganzen Skripttext als einen Batch ab
    async fn batch(&mut self, sql: &str) -> Result<(), SchemaBatchError>;
}

/// Wohin Warnungen gehen; Standard ist die Standardfehlerausgabe
pub type Warn = Box<dyn Fn(&str) + Send + Sync>;

/// Spielt fehlende additive Erweiterungen (004, 005, …) selbst ein, wenn der Dienst-Login DDL
/// auf `board` darf. Der Verbindungsaufbau darf daran nie scheitern: jeder Fehler wird gemeldet,
/// nicht geworfen – fehlt die Erweiterung danach weiter, verweigert sich nur der Zugriff, der
/// sie wirklich braucht.
pub struct MssqlSchemaRunner<R> {
    runner: R,
    warn: Warn,
}

impl<R: SchemaBatchRunner> MssqlSchemaRunner<R> {
    /// Runner, der auf die Standardfehlerausgabe warnt
    pub fn new(runner: R) -> Self {
        Self::with_warn(runner, Box::new(|message| eprintln!("{message}")))
    }

    /// Runner mit eigenem Warnweg (für Tests und die Oberfläche)
    pub fn with_warn(runner: R, warn: Warn) -> Self {
        Self { runner, warn }
    }

    /// Der Ausführer darunter (Tests prüfen daran die abgesetzten Batches)
    pub fn runner(&self) -> &R {
        &self.runner
    }

    /// Spielt die fehlenden Erweiterungen in Skript-Reihenfolge ein und liefert die
    /// eingespielten Namen
    pub async fn apply(&mut self, missing: &[String]) -> Vec<String> {
        let mut applied = Vec::new();
        for (feature, _) in REQUIRED_SCHEMA_FEATURES {
            if !missing.iter().any(|fehlt| fehlt == feature) {
                continue;
            }
            let Some(text) = script_text(feature) else {
                continue;
            };
            if self.run(feature, &text).await {
                applied.push(feature.to_string());
            }
        }
        applied
    }

    /// `true`, wenn die Erweiterung danach eingespielt ist
    async fn run(&mut self, feature: &str, text: &str) -> bool {
        let Err(error) = self.runner.batch(text).await else {
            return true;
        };
        if error.number == Some(ALREADY_APPLIED) {
            return true;
        }
        let kein_recht = error
            .number
            .is_some_and(|number| PERMISSION.contains(&number))
            || hat_rechtetext(&error.message);
        if kein_recht {
            (self.warn)(&format!(
                "Erweiterung {feature} konnte nicht eingespielt werden, es fehlt das Recht ALTER auf dem Schema board. Skript packages/db/sql/{} als Administrator ausführen.",
                SchemaVersionGuard::script(feature)
            ));
            return false;
        }
        (self.warn)(&if error.number == Some(PRECONDITION) {
            format!(
                "Erweiterung {feature} konnte nicht eingespielt werden, eine Vorbedingung fehlt: {}",
                error.message
            )
        } else {
            format!(
                "Erweiterung {feature} konnte nicht eingespielt werden: {}",
                error.message
            )
        });
        false
    }
}

/// Nennt die Meldung ein fehlendes Recht? Entspricht `/permission|Berechtigung/i` in TypeScript.
fn hat_rechtetext(message: &str) -> bool {
    let klein = message.to_lowercase();
    klein.contains("permission") || klein.contains("berechtigung")
}

/// Skripttext der Erweiterung ohne das abschließende `GO`; `None` bei unbekanntem Namen.
///
/// `GO` ist ein Trenner der Werkzeuge, keine Anweisung des Servers – ein Batch über den Treiber
/// darf es nicht enthalten. Entfernt wird wie in TypeScript nur das erste Vorkommen.
pub fn script_text(feature: &str) -> Option<String> {
    let text = match feature {
        "dispatch-runs" => include_str!("../../../../packages/db/sql/004_dispatch_runs.sql"),
        "note-structure" => include_str!("../../../../packages/db/sql/005_note_structure.sql"),
        "dispatch-run-details" => {
            include_str!("../../../../packages/db/sql/006_dispatch_run_details.sql")
        }
        "target-branch" => include_str!("../../../../packages/db/sql/007_target_branch.sql"),
        _ => return None,
    };
    Some(without_first_go(text))
}

/// Der Text ohne die erste Zeile, die nur aus `GO` besteht
fn without_first_go(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut entfernt = false;
    for line in text.split_inclusive('\n') {
        if !entfernt && line.trim() == "GO" {
            entfernt = true;
            continue;
        }
        out.push_str(line);
    }
    out
}
