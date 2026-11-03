//! Langzeit-Tokens der Claude-Accounts in `~/.nexus/account-tokens.json` (nur für den
//! Benutzer lesbar).
//!
//! Ein Token aus `claude setup-token` gilt ein Jahr und wird nie erneuert. Bekommen die Läufe
//! des Dispatchers es als `CLAUDE_CODE_OAUTH_TOKEN`, erneuern sie die Anmeldung im Ordner des
//! Accounts nicht mehr – mehrere Läufe können sich dann nicht gegenseitig das Refresh-Token
//! verbrauchen und die Anmeldung damit ungültig machen. Die Tokens stehen bewusst nicht in
//! `accounts.json`: die Liste geht an die Oberfläche, die Tokens nie.

use std::collections::BTreeMap;
use std::fs;

use serde_json::{json, Value};

use crate::accounts::account::is_account_id;
use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;

/// Anfang eines OAuth-Tokens von Claude Code
const TOKEN_PREFIX: &str = "sk-ant-oat";

/// Liest und schreibt `account-tokens.json`.
#[derive(Debug, Clone)]
pub struct AccountTokenStore {
    home: HarnessHome,
    writer: AtomicWriter,
}

impl AccountTokenStore {
    /// Speicher im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
        }
    }

    /// Token des Accounts oder `None`
    pub fn get(&self, id: &str) -> Option<String> {
        self.read().remove(id)
    }

    /// Hat der Account ein Token?
    pub fn has(&self, id: &str) -> bool {
        self.get(id).is_some()
    }

    /// Setzt das Token des Accounts (getrimmt, muss wie ein OAuth-Token von Claude Code aussehen)
    pub fn set(&self, id: &str, token: &str) -> Result<()> {
        let token = token.trim();
        if !token.starts_with(TOKEN_PREFIX) || token.chars().any(char::is_whitespace) {
            return Err(HarnessError::new(
                "account-invalid",
                format!(
                    "Das sieht nicht wie ein Token aus `claude setup-token` aus (beginnt mit {TOKEN_PREFIX})."
                ),
            )
            .into());
        }
        let mut tokens = self.read();
        tokens.insert(id.to_string(), token.to_string());
        self.save(&tokens)
    }

    /// Entfernt das Token des Accounts; ohne Token geschieht nichts
    pub fn remove(&self, id: &str) -> Result<()> {
        let mut tokens = self.read();
        if tokens.remove(id).is_none() {
            return Ok(());
        }
        self.save(&tokens)
    }

    /// Alle gültigen Einträge; fehlende oder beschädigte Datei ergibt keine
    fn read(&self) -> BTreeMap<String, String> {
        let Some(raw) = fs::read_to_string(self.home.account_tokens_file())
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        else {
            return BTreeMap::new();
        };
        raw.get("tokens")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter(|(id, _)| is_account_id(id))
            .filter_map(|(id, token)| {
                token
                    .as_str()
                    .filter(|token| !token.is_empty())
                    .map(|token| (id.clone(), token.to_string()))
            })
            .collect()
    }

    /// Schreibt die Datei atomar und nur für den Benutzer lesbar
    fn save(&self, tokens: &BTreeMap<String, String>) -> Result<()> {
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        let json = serde_json::to_string_pretty(&json!({ "tokens": tokens }))
            .map_err(Error::unexpected)?;
        self.writer
            .write_private(&self.home.account_tokens_file(), &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "sk-ant-oat01-abc";

    #[test]
    fn speichert_liest_und_entfernt_ein_token_je_account() {
        let temp = tempfile::tempdir().unwrap();
        let store = AccountTokenStore::new(HarnessHome::at(temp.path()));
        assert_eq!(store.get("standard"), None);
        store.set("standard", &format!("  {TOKEN}\n")).unwrap();
        assert_eq!(store.get("standard").as_deref(), Some(TOKEN));
        assert!(store.has("standard"));
        assert!(!store.has("max"));
        store.remove("standard").unwrap();
        assert!(!store.has("standard"));
        store.remove("standard").unwrap();
    }

    #[test]
    fn nimmt_nur_tokens_von_claude_code() {
        let temp = tempfile::tempdir().unwrap();
        let store = AccountTokenStore::new(HarnessHome::at(temp.path()));
        assert!(store.set("standard", "sk-ant-api03-key").is_err());
        assert!(store.set("standard", "sk-ant-oat01 abc").is_err());
        assert!(store.set("standard", "").is_err());
        assert!(!temp.path().join("account-tokens.json").exists());
    }

    #[test]
    fn eine_beschaedigte_datei_ergibt_keine_tokens() {
        let temp = tempfile::tempdir().unwrap();
        let home = HarnessHome::at(temp.path());
        fs::write(home.account_tokens_file(), "{kaputt").unwrap();
        assert_eq!(AccountTokenStore::new(home).get("standard"), None);
    }
}
