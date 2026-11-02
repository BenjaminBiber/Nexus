//! Verweise auf Claude-Accounts in der rohen Dispatcher-Einstellung
//! (`profiles.*.accounts`, `rules[].accounts`); portiert aus
//! `packages/core/src/dispatch/AccountReferences.ts`.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::dispatch::js_value::js_string;

/// Findet und entfernt Verweise auf einen Account in der rohen Einstellung.
#[derive(Debug, Clone, Copy, Default)]
pub struct AccountReferences;

impl AccountReferences {
    /// Neue Suche (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Account-ID → Stellen, die sie nennen (z. B. „Profil schwer“, „Regel 2“)
    pub fn find(&self, raw: &Value) -> BTreeMap<String, Vec<String>> {
        let mut refs: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut add = |ids: Option<&Value>, where_: String| {
            let Some(items) = ids.and_then(Value::as_array) else {
                return;
            };
            for id in items {
                refs.entry(js_string(id).trim().to_string())
                    .or_default()
                    .push(where_.clone());
            }
        };
        if let Some(profiles) = raw.get("profiles").and_then(Value::as_object) {
            for (name, profile) in profiles {
                add(profile.get("accounts"), format!("Profil {name}"));
            }
        }
        if let Some(rules) = raw.get("rules").and_then(Value::as_array) {
            for (index, rule) in rules.iter().enumerate() {
                add(rule.get("accounts"), format!("Regel {}", index + 1));
            }
        }
        refs
    }

    /// Einstellung ohne den Account; eine dadurch leere Liste fällt weg
    /// (Profil: jeder aktive, Regel: wie das Profil)
    pub fn without(&self, raw: &Value, id: &str) -> Value {
        let mut out = raw.clone();
        if let Some(profiles) = out.get_mut("profiles").and_then(Value::as_object_mut) {
            for profile in profiles.values_mut() {
                strip(profile, id);
            }
        }
        if let Some(rules) = out.get_mut("rules").and_then(Value::as_array_mut) {
            for rule in rules.iter_mut() {
                strip(rule, id);
            }
        }
        out
    }
}

/// Entfernt die ID aus `accounts`; eine dadurch leere Liste fällt ganz weg
fn strip(item: &mut Value, id: &str) {
    if item.is_null() {
        *item = Value::Object(Map::new());
        return;
    }
    let Some(object) = item.as_object_mut() else {
        return;
    };
    let Some(accounts) = object.get("accounts").and_then(Value::as_array) else {
        return;
    };
    let rest: Vec<Value> = accounts
        .iter()
        .filter(|account| js_string(account).trim() != id)
        .cloned()
        .collect();
    if rest.is_empty() {
        object.shift_remove("accounts");
    } else {
        object.insert("accounts".to_string(), Value::Array(rest));
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Die rohe Einstellung der TypeScript-Tests
    fn raw() -> Value {
        json!({
            "profiles": {
                "klein": { "model": "sonnet", "accounts": ["pro-1", "pro-2", "pro-1"] },
                "standard": { "model": "opus" },
                "schwer": { "model": "opus", "effort": "max", "accounts": ["max"] },
            },
            "defaultProfile": "standard",
            "rules": [
                { "when": { "labels": ["firma"] }, "profile": "standard", "accounts": ["firma"] },
                { "when": { "labels": ["doku"] }, "profile": "klein" },
            ],
            "escalation": ["klein", "standard", "schwer"],
        })
    }

    #[test]
    fn findet_jede_stelle_die_einen_account_nennt() {
        let refs = AccountReferences::new().find(&raw());
        assert_eq!(
            refs,
            BTreeMap::from([
                (
                    "pro-1".to_string(),
                    vec!["Profil klein".to_string(), "Profil klein".to_string()]
                ),
                ("pro-2".to_string(), vec!["Profil klein".to_string()]),
                ("max".to_string(), vec!["Profil schwer".to_string()]),
                ("firma".to_string(), vec!["Regel 1".to_string()]),
            ])
        );
    }

    #[test]
    fn entfernt_einen_account_und_laesst_die_leere_liste_weg() {
        let refs = AccountReferences::new();
        let ohne_max = refs.without(&raw(), "max");
        assert_eq!(
            ohne_max["profiles"]["schwer"],
            json!({ "model": "opus", "effort": "max" })
        );
        assert_eq!(
            ohne_max["profiles"]["klein"]["accounts"],
            json!(["pro-1", "pro-2", "pro-1"])
        );
        assert_eq!(
            refs.without(&raw(), "firma")["rules"][0],
            json!({ "when": { "labels": ["firma"] }, "profile": "standard" })
        );
    }

    #[test]
    fn eine_einstellung_ohne_accounts_bleibt_unveraendert() {
        let plain = json!({ "profiles": { "standard": { "model": "opus" } } });
        assert_eq!(AccountReferences::new().without(&plain, "max"), plain);
        assert!(AccountReferences::new().find(&plain).is_empty());
    }
}
