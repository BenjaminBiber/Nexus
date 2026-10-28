//! Gibt in der Konfiguration von Claude Code die Importe von außerhalb des Projekts frei.
//! Portiert aus `packages/core/src/install/ClaudeImportApproval.ts`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;

/// Schlüssel der Freigabe in der Konfiguration von Claude Code
const APPROVED: &str = "hasClaudeMdExternalIncludesApproved";

/// Schlüssel des schon gezeigten Hinweises
const SHOWN: &str = "hasClaudeMdExternalIncludesWarningShown";

/// Zustand der Freigabe
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalStatus {
    /// gesetzt
    Approved,
    /// war schon gesetzt
    Unchanged,
    /// Claude Code lief auf diesem Rechner noch nie
    NoConfig,
}

/// Ergebnis der Freigabe
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApprovalResult {
    /// `approved`: gesetzt; `unchanged`: war schon gesetzt; `no-config`: Claude Code lief noch nie
    pub status: ApprovalStatus,
    /// Konfigurationsdatei von Claude Code, die geprüft oder geändert wurde
    pub file: String,
    /// Projektschlüssel, deren Freigabe jetzt gesetzt ist
    pub keys: Vec<String>,
}

/// Gibt in der Konfiguration von Claude Code die Importe von außerhalb des Projekts frei, damit
/// die Rules `@~/.harness/rules/docs.md` und `@~/.harness/rules/board.md` laden dürfen. Die
/// Freigabe gilt je Projekt für alle Importe. Claude Code fragt danach sonst nur beim Start einer
/// interaktiven Session, und das nie, wenn der Import erst beim Zugriff auf `docs/` geladen wird.
///
/// Die beiden Werte sind nicht dokumentiert; Claude Code liest sie im Eintrag des Git-Stamms,
/// auch beim Start in einem Unterordner.
#[derive(Debug, Clone)]
pub struct ClaudeImportApproval {
    config_dir: Option<String>,
    home: PathBuf,
    windows: bool,
    writer: AtomicWriter,
}

impl Default for ClaudeImportApproval {
    fn default() -> Self {
        Self::from_process_env()
    }
}

impl ClaudeImportApproval {
    /// Freigabe aus der Umgebung des laufenden Prozesses (`CLAUDE_CONFIG_DIR`, Benutzerordner)
    pub fn from_process_env() -> Self {
        Self::new(
            std::env::var("CLAUDE_CONFIG_DIR")
                .ok()
                .filter(|dir| !dir.is_empty()),
            home_dir(),
            cfg!(windows),
        )
    }

    /// Freigabe mit ausdrücklich gesetzter Umgebung (Tests)
    pub fn new(config_dir: Option<String>, home: PathBuf, windows: bool) -> Self {
        Self {
            config_dir,
            home,
            windows,
            writer: AtomicWriter::new(),
        }
    }

    /// Datei wie bei Claude Code: ältere `~/.claude/.config.json`, sonst `.claude.json` in
    /// `CLAUDE_CONFIG_DIR` oder im Benutzerordner
    pub fn config_file(&self) -> PathBuf {
        let base = match &self.config_dir {
            Some(dir) => PathBuf::from(dir),
            None => self.home.join(".claude"),
        };
        let legacy = base.join(".config.json");
        if legacy.exists() {
            return legacy;
        }
        match &self.config_dir {
            Some(dir) => PathBuf::from(dir).join(".claude.json"),
            None => self.home.join(".claude.json"),
        }
    }

    /// Schlüssel des Projekts: unter Windows mit Schrägstrichen und Laufwerk groß und klein
    /// (VS Code meldet es klein), dazu jede vorhandene Schreibweise desselben Pfads
    pub fn keys(&self, project_dir: &str, projects: &Map<String, Value>) -> Vec<String> {
        if !self.windows {
            return vec![project_dir.to_string()];
        }
        let slashed = project_dir.replace('\\', "/");
        let mut variants: BTreeSet<String> = BTreeSet::new();
        variants.insert(with_first(&slashed, true));
        variants.insert(with_first(&slashed, false));
        let same = comparable(&slashed);
        for key in projects.keys() {
            if comparable(key) == same {
                variants.insert(key.clone());
            }
        }
        variants.into_iter().collect()
    }

    /// Setzt beide Werte in allen Schlüsseln des Projekts; alles andere in der Datei bleibt
    /// unverändert. Eine fehlende Datei wird nicht angelegt, kaputtes JSON nie überschrieben.
    pub fn approve(&self, project_dir: &str) -> Result<ApprovalResult> {
        let file = self.config_file();
        let name = file.to_string_lossy().into_owned();
        if !file.exists() {
            return Ok(ApprovalResult {
                status: ApprovalStatus::NoConfig,
                file: name,
                keys: Vec::new(),
            });
        }
        let mut config = self.read(&file)?;
        let mut projects = projects(&config);
        let keys = self.keys(project_dir, &projects);
        if keys
            .iter()
            .all(|key| is_set(&projects, key, APPROVED) && is_set(&projects, key, SHOWN))
        {
            return Ok(ApprovalResult {
                status: ApprovalStatus::Unchanged,
                file: name,
                keys,
            });
        }
        for key in &keys {
            let mut entry = match projects.get(key) {
                Some(Value::Object(object)) => object.clone(),
                _ => Map::new(),
            };
            entry.insert(APPROVED.to_string(), Value::Bool(true));
            entry.insert(SHOWN.to_string(), Value::Bool(true));
            projects.insert(key.clone(), Value::Object(entry));
        }
        config.insert("projects".to_string(), Value::Object(projects));
        let json =
            serde_json::to_string_pretty(&Value::Object(config)).map_err(Error::unexpected)?;
        self.writer.write(&file, &format!("{json}\n"))?;
        self.verify(&file, &keys)?;
        Ok(ApprovalResult {
            status: ApprovalStatus::Approved,
            file: name,
            keys,
        })
    }

    /// Liest die Datei erneut: hat eine gleichzeitig laufende Session sie überschrieben, gibt es
    /// einen Fehler mit Hinweis
    fn verify(&self, file: &Path, keys: &[String]) -> Result<()> {
        let projects = projects(&self.read(file)?);
        if keys.iter().all(|key| is_set(&projects, key, APPROVED)) {
            return Ok(());
        }
        Err(HarnessError::new(
            "approval-lost",
            format!(
                "Die Freigabe in {} wurde gleich wieder überschrieben. Bitte `harness init` wiederholen.",
                file.display()
            ),
        )
        .into())
    }

    /// Liest die Konfiguration als Objekt
    fn read(&self, file: &Path) -> Result<Map<String, Value>> {
        let text = fs::read_to_string(file).map_err(Error::unexpected)?;
        if let Ok(Value::Object(object)) = serde_json::from_str::<Value>(&text) {
            return Ok(object);
        }
        Err(HarnessError::new(
            "claude-config-unreadable",
            format!(
                "{} ist kein gültiges JSON. Bitte prüfen; die Datei wurde nicht verändert.",
                file.display()
            ),
        )
        .into())
    }
}

/// Der Abschnitt `projects` als Objekt
fn projects(config: &Map<String, Value>) -> Map<String, Value> {
    match config.get("projects") {
        Some(Value::Object(object)) => object.clone(),
        _ => Map::new(),
    }
}

/// Steht der Wert im Projekteintrag auf `true`?
fn is_set(projects: &Map<String, Value>, key: &str, name: &str) -> bool {
    projects
        .get(key)
        .and_then(|entry| entry.get(name))
        .is_some_and(|value| value == &Value::Bool(true))
}

/// Derselbe Pfad mit großem bzw. kleinem erstem Zeichen (das Laufwerk)
fn with_first(path: &str, upper: bool) -> String {
    let mut chars = path.chars();
    match chars.next() {
        None => String::new(),
        Some(first) if upper => format!("{}{}", first.to_uppercase(), chars.as_str()),
        Some(first) => format!("{}{}", first.to_lowercase(), chars.as_str()),
    }
}

/// Vergleichsform eines Windows-Pfads: Schrägstriche, ohne Schrägstrich am Ende, Kleinbuchstaben
fn comparable(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// Benutzerordner des angemeldeten Benutzers, wie `os.homedir()` in Node
fn home_dir() -> PathBuf {
    #[allow(deprecated)]
    std::env::home_dir().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Temporärer Benutzerordner und ein eigener Konfigurationsordner
    fn basis() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let base = tempfile::Builder::new()
            .prefix("harness-approval-")
            .tempdir()
            .unwrap();
        let home = base.path().join("home");
        let config = base.path().join("config");
        fs::create_dir(&home).unwrap();
        fs::create_dir(&config).unwrap();
        (base, home, config)
    }

    /// Freigabe auf einem Benutzerordner, ohne Windows-Pfadvarianten
    fn freigabe(home: &Path) -> ClaudeImportApproval {
        ClaudeImportApproval::new(None, home.to_path_buf(), false)
    }

    /// Schreibt die Konfiguration als JSON
    fn schreiben(file: &Path, value: Value) {
        fs::write(file, serde_json::to_string_pretty(&value).unwrap()).unwrap();
    }

    /// Liest die Konfiguration
    fn lesen(file: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(file).unwrap()).unwrap()
    }

    #[test]
    fn nimmt_claude_json_im_benutzerordner() {
        let (_base, home, _config) = basis();
        assert_eq!(freigabe(&home).config_file(), home.join(".claude.json"));
    }

    #[test]
    fn folgt_claude_config_dir_wie_claude_code_selbst() {
        let (_base, home, config) = basis();
        let approval =
            ClaudeImportApproval::new(Some(config.to_string_lossy().into_owned()), home, false);
        assert_eq!(approval.config_file(), config.join(".claude.json"));
    }

    #[test]
    fn nimmt_die_altere_config_json_wenn_es_sie_gibt() {
        let (_base, home, _config) = basis();
        fs::create_dir(home.join(".claude")).unwrap();
        schreiben(
            &home.join(".claude").join(".config.json"),
            serde_json::json!({}),
        );
        assert_eq!(
            freigabe(&home).config_file(),
            home.join(".claude").join(".config.json")
        );
    }

    #[test]
    fn schreibt_unter_windows_den_pfad_mit_schragstrichen_laufwerk_gross_und_klein() {
        let (_base, home, _config) = basis();
        let approval = ClaudeImportApproval::new(None, home, true);
        let mut keys = approval.keys("C:\\Dev\\Mein Projekt", &Map::new());
        keys.sort();
        assert_eq!(keys, ["C:/Dev/Mein Projekt", "c:/Dev/Mein Projekt"]);
    }

    #[test]
    fn nimmt_unter_windows_vorhandene_schreibweisen_desselben_pfads_mit() {
        let (_base, home, _config) = basis();
        let approval = ClaudeImportApproval::new(None, home, true);
        let projects: Map<String, Value> = serde_json::from_value(serde_json::json!({
            "C:\\Dev\\Mein Projekt": {},
            "c:/dev/mein projekt/": {},
            "C:/Dev/Anderes": {},
        }))
        .unwrap();
        let mut keys = approval.keys("C:/Dev/Mein Projekt", &projects);
        keys.sort();
        let mut erwartet = vec![
            "C:/Dev/Mein Projekt",
            "C:\\Dev\\Mein Projekt",
            "c:/Dev/Mein Projekt",
            "c:/dev/mein projekt/",
        ];
        erwartet.sort_unstable();
        assert_eq!(keys, erwartet);
    }

    #[test]
    fn lasst_den_pfad_sonst_unverandert() {
        let (_base, home, _config) = basis();
        assert_eq!(
            freigabe(&home).keys("/home/x/Projekt", &Map::new()),
            ["/home/x/Projekt"]
        );
    }

    #[test]
    fn setzt_beide_werte_und_lasst_alles_andere_unverandert() {
        let (_base, home, _config) = basis();
        let file = home.join(".claude.json");
        schreiben(
            &file,
            serde_json::json!({
                "userID": "u",
                "projects": {
                    "/p": { "allowedTools": ["Bash(ls)"], APPROVED: false, SHOWN: false },
                    "/q": { "x": 1 },
                },
            }),
        );

        let result = freigabe(&home).approve("/p").unwrap();

        assert_eq!(result.status, ApprovalStatus::Approved);
        assert_eq!(result.keys, ["/p"]);
        assert_eq!(result.file, file.to_string_lossy());
        assert_eq!(
            lesen(&file),
            serde_json::json!({
                "userID": "u",
                "projects": {
                    "/p": { "allowedTools": ["Bash(ls)"], APPROVED: true, SHOWN: true },
                    "/q": { "x": 1 },
                },
            })
        );
    }

    #[test]
    fn legt_einen_fehlenden_projekteintrag_an() {
        let (_base, home, _config) = basis();
        let file = home.join(".claude.json");
        schreiben(&file, serde_json::json!({ "projects": {} }));

        freigabe(&home).approve("/neu").unwrap();

        assert_eq!(
            lesen(&file)["projects"]["/neu"],
            serde_json::json!({ APPROVED: true, SHOWN: true })
        );
    }

    #[test]
    fn ist_wiederholbar_und_meldet_wenn_schon_alles_gesetzt_ist() {
        let (_base, home, _config) = basis();
        let file = home.join(".claude.json");
        schreiben(
            &file,
            serde_json::json!({ "projects": { "/p": { APPROVED: true, SHOWN: true } } }),
        );
        let vorher = fs::read_to_string(&file).unwrap();

        let result = freigabe(&home).approve("/p").unwrap();

        assert_eq!(result.status, ApprovalStatus::Unchanged);
        assert_eq!(fs::read_to_string(&file).unwrap(), vorher);
    }

    #[test]
    fn setzt_unter_windows_alle_schreibweisen_des_projekts() {
        let (_base, home, _config) = basis();
        let file = home.join(".claude.json");
        schreiben(
            &file,
            serde_json::json!({ "projects": { "C:\\Dev\\P": { APPROVED: false } } }),
        );

        ClaudeImportApproval::new(None, home, true)
            .approve("C:/Dev/P")
            .unwrap();

        let config = lesen(&file);
        for key in ["C:\\Dev\\P", "C:/Dev/P", "c:/Dev/P"] {
            assert_eq!(
                config["projects"][key][APPROVED],
                Value::Bool(true),
                "{key}"
            );
        }
    }

    #[test]
    fn meldet_fehlende_konfiguration_und_legt_keine_datei_an() {
        let (_base, home, _config) = basis();

        let result = freigabe(&home).approve("/p").unwrap();

        assert_eq!(result.status, ApprovalStatus::NoConfig);
        assert_eq!(result.keys, Vec::<String>::new());
        assert!(!home.join(".claude.json").exists());
    }

    #[test]
    fn uberschreibt_kaputtes_json_nie() {
        let (_base, home, _config) = basis();
        let file = home.join(".claude.json");
        fs::write(&file, "{ kaputt").unwrap();

        let error = freigabe(&home).approve("/p").unwrap_err();

        assert_eq!(error.code(), Some("claude-config-unreadable"));
        assert!(error.to_string().contains("kein gültiges JSON"), "{error}");
        assert_eq!(fs::read_to_string(&file).unwrap(), "{ kaputt");
    }

    #[test]
    fn der_zustand_steht_im_json_wie_in_typescript() {
        assert_eq!(
            serde_json::to_value(ApprovalStatus::NoConfig).unwrap(),
            Value::String("no-config".to_string())
        );
    }
}
