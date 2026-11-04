//! Speichert die Claude-Accounts in `~/.nexus/accounts.json` (atomar); portiert aus
//! `packages/core/src/accounts/AccountStore.ts`.
//!
//! Der Account „standard“ steht immer an erster Stelle und hat nie einen eigenen Ordner.

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::accounts::account::{
    account_limit, is_account_id, standard_account, ClaudeAccount, MAX_ACCOUNT_NAME_LENGTH,
    STANDARD_ACCOUNT_ID,
};
use crate::accounts::paths::AccountPaths;
use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;
use crate::text::SlugBuilder;

/// Ein neuer Account
#[derive(Debug, Clone, Default)]
pub struct NewAccount {
    pub name: String,
    /// Vorhandener Ordner (z. B. ein MCACC-Profil); leer = neuer Ordner unter
    /// `~/.nexus/accounts/<id>`
    pub config_dir: Option<String>,
}

/// Änderungen an einem Account; nicht genannte Felder bleiben, wie sie sind
#[derive(Debug, Clone, Default)]
pub struct AccountPatch {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub fallback: Option<bool>,
    /// Obergrenzen in Prozent (1–100); `Some(None)` entfernt sie
    pub limit_five_hour: Option<Option<i64>>,
    pub limit_week: Option<Option<i64>>,
}

/// Liest und schreibt `accounts.json`.
#[derive(Debug, Clone)]
pub struct AccountStore {
    home: HarnessHome,
    paths: AccountPaths,
    writer: AtomicWriter,
}

impl AccountStore {
    /// Store im angegebenen Benutzerordner mit den Pfaden des Rechners
    pub fn new(home: HarnessHome) -> Self {
        Self::with_paths(home, AccountPaths::from_process_env())
    }

    /// Store mit eigenen Pfaden (für Tests)
    pub fn with_paths(home: HarnessHome, paths: AccountPaths) -> Self {
        Self {
            home,
            paths,
            writer: AtomicWriter::new(),
        }
    }

    /// Liest die Accounts; fehlende oder beschädigte Datei ergibt nur den Standard-Account
    pub fn list(&self) -> Vec<ClaudeAccount> {
        let file = self.home.accounts_file();
        if !file.exists() {
            return vec![standard_account()];
        }
        match fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        {
            Some(raw) => normalize(&raw),
            None => vec![standard_account()],
        }
    }

    /// Account zur ID oder `None`
    pub fn find(&self, id: &str) -> Option<ClaudeAccount> {
        self.list().into_iter().find(|account| account.id == id)
    }

    /// Legt einen Account an; ohne Ordner entsteht `~/.nexus/accounts/<id>`, ein angegebener
    /// Ordner muss existieren
    pub fn create(&self, input: &NewAccount) -> Result<ClaudeAccount> {
        let name = check_name(&input.name)?;
        let mut accounts = self.list();
        let id = unique_id(&slug(&name), &accounts);
        let given = input
            .config_dir
            .as_deref()
            .map(str::trim)
            .filter(|dir| !dir.is_empty());
        let config_dir = match given {
            Some(given) => {
                let dir = self.paths.expand(given);
                // has_root statt is_absolute: unter Windows gilt „/ordner“ sonst als relativ
                if !dir.has_root() {
                    return Err(invalid(
                        "Der Ordner muss ein absoluter Pfad sein (oder mit ~ beginnen).",
                    ));
                }
                if !dir.exists() {
                    return Err(invalid(format!("Den Ordner {given} gibt es nicht.")));
                }
                if accounts.iter().any(|account| {
                    account
                        .config_dir
                        .as_deref()
                        .is_some_and(|other| self.paths.expand(other) == dir)
                }) {
                    return Err(invalid(format!(
                        "Der Ordner {given} gehört schon zu einem anderen Account."
                    )));
                }
                given.to_string()
            }
            None => {
                let dir = self.home.accounts_dir().join(&id);
                fs::create_dir_all(&dir).map_err(Error::unexpected)?;
                dir.to_string_lossy().to_string()
            }
        };
        let account = ClaudeAccount {
            id,
            name,
            config_dir: Some(config_dir),
            enabled: true,
            fallback: false,
            limit_five_hour: None,
            limit_week: None,
        };
        accounts.push(account.clone());
        self.save(&accounts)?;
        Ok(account)
    }

    /// Ändert Name, Schalter „aktiv“ und „nur als Fallback“ oder die Obergrenzen
    pub fn update(&self, id: &str, patch: &AccountPatch) -> Result<ClaudeAccount> {
        let mut accounts = self.list();
        let Some(index) = accounts.iter().position(|account| account.id == id) else {
            return Err(not_found(id));
        };
        if let Some(name) = &patch.name {
            accounts[index].name = check_name(name)?;
        }
        if let Some(enabled) = patch.enabled {
            accounts[index].enabled = enabled;
        }
        if let Some(fallback) = patch.fallback {
            accounts[index].fallback = fallback;
        }
        if let Some(limit) = patch.limit_five_hour {
            accounts[index].limit_five_hour = check_limit(limit, "5 Stunden")?;
        }
        if let Some(limit) = patch.limit_week {
            accounts[index].limit_week = check_limit(limit, "Woche")?;
        }
        self.save(&accounts)?;
        Ok(accounts[index].clone())
    }

    /// Entfernt einen Account; `delete_dir` löscht auch seinen Ordner, aber nur einen unter
    /// `~/.nexus/accounts`
    pub fn remove(&self, id: &str, delete_dir: bool) -> Result<()> {
        if id == STANDARD_ACCOUNT_ID {
            return Err(invalid(
                "Den Standard-Account kann man nur abschalten, nicht entfernen.",
            ));
        }
        let accounts = self.list();
        let Some(account) = accounts.iter().find(|account| account.id == id) else {
            return Err(not_found(id));
        };
        if delete_dir {
            if let Some(config_dir) = &account.config_dir {
                let dir = self.paths.expand(config_dir);
                if !is_inside(&self.home.accounts_dir(), &dir) {
                    return Err(invalid(format!(
                        "Der Ordner {config_dir} liegt nicht unter {} – bitte selbst löschen.",
                        self.home.accounts_dir().display()
                    )));
                }
                if dir.exists() {
                    fs::remove_dir_all(&dir).map_err(Error::unexpected)?;
                }
            }
        }
        let rest: Vec<ClaudeAccount> = accounts
            .into_iter()
            .filter(|account| account.id != id)
            .collect();
        self.save(&rest)
    }

    /// Schreibt die Liste atomar; der Benutzerordner wird bei Bedarf angelegt
    fn save(&self, accounts: &[ClaudeAccount]) -> Result<()> {
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        // Zwei Leerzeichen Einrückung wie `JSON.stringify(x, null, 2)`
        let json = serde_json::to_string_pretty(&json!({ "accounts": accounts }))
            .map_err(Error::unexpected)?;
        self.writer.write(&self.home.accounts_file(), &json)
    }
}

/// Übernimmt nur gültige Einträge (doppelte IDs: der erste gilt); der Standard-Account fehlt nie
/// und steht vorn
fn normalize(raw: &Value) -> Vec<ClaudeAccount> {
    let list = raw
        .get("accounts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<ClaudeAccount> = Vec::new();
    for entry in &list {
        let Some(id) = entry.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !is_account_id(id) || out.iter().any(|account| account.id == id) {
            continue;
        }
        let standard = id == STANDARD_ACCOUNT_ID;
        let name = entry
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(|name| name.chars().take(MAX_ACCOUNT_NAME_LENGTH).collect())
            .unwrap_or_else(|| {
                if standard {
                    standard_account().name
                } else {
                    id.to_string()
                }
            });
        let config_dir = if standard {
            None
        } else {
            entry
                .get("configDir")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|dir| !dir.is_empty())
                .map(ToString::to_string)
        };
        if !standard && config_dir.is_none() {
            continue;
        }
        out.push(ClaudeAccount {
            id: id.to_string(),
            name,
            config_dir,
            enabled: entry.get("enabled") != Some(&Value::Bool(false)),
            fallback: entry.get("fallback") == Some(&Value::Bool(true)),
            limit_five_hour: entry.get("limitFiveHour").and_then(account_limit),
            limit_week: entry.get("limitWeek").and_then(account_limit),
        });
    }
    let standard = out
        .iter()
        .find(|account| account.id == STANDARD_ACCOUNT_ID)
        .cloned()
        .unwrap_or_else(standard_account);
    let mut sorted = vec![standard];
    sorted.extend(
        out.into_iter()
            .filter(|account| account.id != STANDARD_ACCOUNT_ID),
    );
    sorted
}

/// Kurztitel des Namens als Account-ID (höchstens 40 Zeichen, ohne Bindestrich am Ende)
fn slug(name: &str) -> String {
    let slug = SlugBuilder::build(name);
    let cut: String = slug.chars().take(40).collect();
    let trimmed = cut.trim_end_matches('-');
    if trimmed.is_empty() || !is_account_id(trimmed) {
        "account".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Freie ID: der Kurztitel selbst oder mit `-2`, `-3`, …
fn unique_id(base: &str, accounts: &[ClaudeAccount]) -> String {
    let taken = |id: &str| accounts.iter().any(|account| account.id == id);
    if !taken(base) {
        return base.to_string();
    }
    let short: String = base.chars().take(36).collect();
    for n in 2.. {
        let id = format!("{short}-{n}");
        if !taken(&id) {
            return id;
        }
    }
    unreachable!("es gibt immer eine freie Nummer")
}

/// Getrimmter Name mit Längenprüfung
fn check_name(raw: &str) -> Result<String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(invalid("Der Account braucht einen Namen."));
    }
    if name.chars().count() > MAX_ACCOUNT_NAME_LENGTH {
        return Err(invalid(format!(
            "Der Name darf höchstens {MAX_ACCOUNT_NAME_LENGTH} Zeichen haben."
        )));
    }
    Ok(name.to_string())
}

/// Obergrenze prüfen; `None` entfernt sie
fn check_limit(value: Option<i64>, label: &str) -> Result<Option<i64>> {
    let Some(value) = value else {
        return Ok(None);
    };
    match account_limit(&json!(value)) {
        Some(limit) => Ok(Some(limit)),
        None => Err(invalid(format!(
            "Die Obergrenze für {label} muss eine ganze Zahl von 1 bis 100 sein."
        ))),
    }
}

/// Liegt `dir` echt unterhalb von `base`?
fn is_inside(base: &Path, dir: &Path) -> bool {
    dir.strip_prefix(base)
        .is_ok_and(|rest| rest.components().next().is_some())
}

/// Fehler „account-invalid“
fn invalid(message: impl Into<String>) -> Error {
    HarnessError::new("account-invalid", message).into()
}

/// Fehler „account-not-found“
fn not_found(id: &str) -> Error {
    HarnessError::new(
        "account-not-found",
        format!("Account „{id}“ gibt es nicht."),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    /// Store in einem frischen Ordner; der Benutzerordner des Rechners liegt darin unter `user`
    fn store(dir: &Path) -> AccountStore {
        AccountStore::with_paths(
            HarnessHome::at(dir),
            AccountPaths::new(HashMap::new(), dir.join("user")),
        )
    }

    /// Die Meldung des erwartbaren Fehlers
    fn message(error: Error) -> String {
        error.harness().expect("erwartbarer Fehler").message.clone()
    }

    #[test]
    fn kennt_ohne_datei_nur_den_standard_account() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(store(temp.path()).list(), vec![standard_account()]);
    }

    #[test]
    fn legt_einen_account_mit_eigenem_ordner_unter_accounts_an() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(temp.path());
        let account = store
            .create(&NewAccount {
                name: "Max (privat)".to_string(),
                config_dir: None,
            })
            .unwrap();
        assert_eq!(account.id, "max-privat");
        assert_eq!(account.name, "Max (privat)");
        assert_eq!(
            account.config_dir.as_deref().map(Path::new),
            Some(temp.path().join("accounts").join("max-privat").as_path())
        );
        assert!(Path::new(account.config_dir.as_ref().unwrap()).is_dir());
        assert_eq!(
            store
                .list()
                .iter()
                .map(|a| a.id.clone())
                .collect::<Vec<String>>(),
            vec!["standard", "max-privat"]
        );
    }

    #[test]
    fn vergibt_bei_gleichem_namen_eine_freie_id() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(temp.path());
        let neu = |name: &str| NewAccount {
            name: name.to_string(),
            config_dir: None,
        };
        store.create(&neu("Pro")).unwrap();
        assert_eq!(store.create(&neu("Pro")).unwrap().id, "pro-2");
    }

    #[test]
    fn uebernimmt_einen_vorhandenen_ordner_nur_einmal_und_nur_wenn_es_ihn_gibt() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(temp.path());
        fs::create_dir_all(temp.path().join("user").join(".mcacc").join("p1")).unwrap();
        let account = store
            .create(&NewAccount {
                name: "MCACC".to_string(),
                config_dir: Some("~/.mcacc/p1".to_string()),
            })
            .unwrap();
        assert_eq!(account.config_dir.as_deref(), Some("~/.mcacc/p1"));

        let doppelt = store
            .create(&NewAccount {
                name: "Doppelt".to_string(),
                config_dir: Some(
                    temp.path()
                        .join("user")
                        .join(".mcacc")
                        .join("p1")
                        .to_string_lossy()
                        .to_string(),
                ),
            })
            .unwrap_err();
        assert!(message(doppelt).contains("schon zu einem anderen Account"));

        let fehlt = store
            .create(&NewAccount {
                name: "Fehlt".to_string(),
                config_dir: Some(
                    temp.path()
                        .join("gibt")
                        .join("es")
                        .join("nicht")
                        .to_string_lossy()
                        .to_string(),
                ),
            })
            .unwrap_err();
        assert!(message(fehlt).contains("gibt es nicht"));
    }

    #[test]
    fn merkt_sich_nur_als_fallback_und_liest_es_aus_der_datei() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(temp.path());
        store
            .create(&NewAccount {
                name: "Privat".to_string(),
                config_dir: None,
            })
            .unwrap();
        let geaendert = store
            .update(
                "privat",
                &AccountPatch {
                    fallback: Some(true),
                    ..AccountPatch::default()
                },
            )
            .unwrap();
        assert!(geaendert.fallback);
        assert!(store.find("privat").unwrap().fallback);
        assert!(!store.find("standard").unwrap().fallback);
    }

    #[test]
    fn speichert_eigene_obergrenzen_und_lehnt_ungueltige_ab() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(temp.path());
        store
            .create(&NewAccount {
                name: "Arbeit".to_string(),
                config_dir: None,
            })
            .unwrap();
        let gesetzt = store
            .update(
                "arbeit",
                &AccountPatch {
                    limit_five_hour: Some(Some(50)),
                    limit_week: Some(Some(80)),
                    ..AccountPatch::default()
                },
            )
            .unwrap();
        assert_eq!(
            (gesetzt.limit_five_hour, gesetzt.limit_week),
            (Some(50), Some(80))
        );

        let entfernt = store
            .update(
                "arbeit",
                &AccountPatch {
                    limit_week: Some(None),
                    ..AccountPatch::default()
                },
            )
            .unwrap();
        assert_eq!(
            (entfernt.limit_five_hour, entfernt.limit_week),
            (Some(50), None)
        );

        let zu_klein = store
            .update(
                "arbeit",
                &AccountPatch {
                    limit_five_hour: Some(Some(0)),
                    ..AccountPatch::default()
                },
            )
            .unwrap_err();
        assert!(message(zu_klein).contains("1 bis 100"));

        // Eine Obergrenze außerhalb des Bereichs in der Datei zählt wie keine
        fs::write(
            store.home.accounts_file(),
            r#"{"accounts":[{"id":"x","configDir":"/x","limitFiveHour":120,"limitWeek":30}]}"#,
        )
        .unwrap();
        let aus_datei = store.find("x").unwrap();
        assert_eq!(
            (aus_datei.limit_five_hour, aus_datei.limit_week),
            (None, Some(30))
        );
    }

    #[test]
    fn aendert_name_und_schalter_der_standard_account_laesst_sich_nur_abschalten() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(temp.path());
        store
            .update(
                "standard",
                &AccountPatch {
                    enabled: Some(false),
                    name: Some("Firma".to_string()),
                    ..AccountPatch::default()
                },
            )
            .unwrap();
        assert_eq!(
            store.find("standard").unwrap(),
            ClaudeAccount {
                id: "standard".to_string(),
                name: "Firma".to_string(),
                config_dir: None,
                enabled: false,
                fallback: false,
                limit_five_hour: None,
                limit_week: None,
            }
        );
        assert!(message(store.remove("standard", false).unwrap_err()).contains("nur abschalten"));
    }

    #[test]
    fn entfernt_einen_account_und_loescht_seinen_ordner_nur_unter_accounts() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(temp.path());
        let eigen = store
            .create(&NewAccount {
                name: "Eigen".to_string(),
                config_dir: None,
            })
            .unwrap();
        let eigener_ordner = std::path::PathBuf::from(eigen.config_dir.clone().unwrap());
        store.remove(&eigen.id, true).unwrap();
        assert!(!eigener_ordner.exists());

        let fremder_ordner = temp.path().join("user").join("outside");
        fs::create_dir_all(&fremder_ordner).unwrap();
        let fremd = store
            .create(&NewAccount {
                name: "Fremd".to_string(),
                config_dir: Some(fremder_ordner.to_string_lossy().to_string()),
            })
            .unwrap();
        assert!(
            message(store.remove(&fremd.id, true).unwrap_err()).contains("bitte selbst löschen")
        );
        store.remove(&fremd.id, false).unwrap();
        assert!(fremder_ordner.exists());
        assert_eq!(
            store
                .list()
                .iter()
                .map(|a| a.id.clone())
                .collect::<Vec<String>>(),
            vec!["standard"]
        );
    }

    #[test]
    fn verwirft_ungueltige_eintraege_und_stellt_den_standard_account_nach_vorn() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(temp.path());
        fs::create_dir_all(temp.path()).unwrap();
        fs::write(
            store.home.accounts_file(),
            serde_json::to_string(&json!({ "accounts": [
                { "id": "pro", "name": "Pro", "configDir": "/x" },
                { "id": "Böse ID", "configDir": "/y" },
                { "id": "ohne-ordner" },
                { "id": "pro", "configDir": "/z" },
                { "id": "standard", "enabled": false, "configDir": "/ignoriert" },
            ] }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            store.list(),
            vec![
                ClaudeAccount {
                    id: "standard".to_string(),
                    name: "Standard".to_string(),
                    config_dir: None,
                    enabled: false,
                    fallback: false,
                    limit_five_hour: None,
                    limit_week: None,
                },
                ClaudeAccount {
                    id: "pro".to_string(),
                    name: "Pro".to_string(),
                    config_dir: Some("/x".to_string()),
                    enabled: true,
                    fallback: false,
                    limit_five_hour: None,
                    limit_week: None,
                },
            ]
        );
    }

    #[test]
    fn faellt_bei_beschaedigter_datei_auf_den_standard_account_zurueck() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(temp.path());
        fs::write(store.home.accounts_file(), "{kaputt").unwrap();
        assert_eq!(
            store
                .list()
                .iter()
                .map(|a| a.id.clone())
                .collect::<Vec<String>>(),
            vec!["standard"]
        );
    }
}
