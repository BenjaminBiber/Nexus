//! Wendet die Änderungen eines Vorschlags auf die rohe Dispatcher-Einstellung an; portiert
//! aus `packages/core/src/dispatch/optimization/ConfigPatcher.ts`.
//!
//! Geprüft wird nur Pfad und Ausgangswert; ob das Ergebnis gültig ist, entscheidet
//! [`crate::dispatch::DispatchConfigStore`] beim Schreiben.

use serde_json::{Map, Value};

use crate::dispatch::config::DispatchConfig;
use crate::dispatch::optimization::types::ConfigPatchOp;
use crate::error::{HarnessError, Result};

/// Pfade, die ein Vorschlag ändern darf
const ALLOWED_PROFILE_KEYS: [&str; 4] = ["maxTurns", "timeoutMinutes", "model", "effort"];

/// Wendet die Änderungen eines Vorschlags auf eine Kopie der rohen Einstellung an.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConfigPatcher;

impl ConfigPatcher {
    /// Neuer Patcher (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Darf ein Vorschlag diesen Pfad ändern?
    pub fn allowed(&self, path: &str) -> bool {
        let parts: Vec<&str> = path.split('.').collect();
        match parts.as_slice() {
            ["profiles", name, key] => {
                !name.is_empty() && ALLOWED_PROFILE_KEYS.contains(key)
            }
            ["defaultProfile"] | ["escalation"] => true,
            ["rules", index, "profile"] => {
                !index.is_empty() && index.chars().all(|c| c.is_ascii_digit())
            }
            _ => false,
        }
    }

    /// Alle Änderungen auf eine Kopie der rohen Einstellung; veralteter Ausgangswert →
    /// `conflict` (`patch-stale`), fremder Pfad oder fehlende Regel → `input-invalid`
    pub fn apply(
        &self,
        raw: &Value,
        config: &DispatchConfig,
        ops: &[ConfigPatchOp],
    ) -> Result<Value> {
        let mut next = raw.clone();
        for op in ops {
            if !self.allowed(&op.path) {
                return Err(HarnessError::new(
                    "input-invalid",
                    format!("{} darf ein Vorschlag nicht ändern.", op.path),
                )
                .into());
            }
            if current(config, &op.path) != op.from {
                return Err(HarnessError::new(
                    "conflict",
                    format!(
                        "{} ist nicht mehr {} – der Vorschlag ist veraltet.",
                        op.path,
                        js_stringify(op.from.as_ref())
                    ),
                )
                .with_details(serde_json::json!({ "reason": "patch-stale" }))
                .into());
            }
            set(&mut next, &op.path, op.to.clone())?;
        }
        Ok(next)
    }
}

/// Wirksamer Wert (nach Normalisierung) – gegen ihn wurde der Vorschlag berechnet;
/// `None` steht für `undefined` in TypeScript
fn current(config: &DispatchConfig, path: &str) -> Option<Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let head = parts.first().copied().unwrap_or("");
    let a = parts.get(1).copied().unwrap_or("");
    let b = parts.get(2).copied().unwrap_or("");
    match head {
        "profiles" => {
            let profile = config.profiles.get(a)?;
            let value = serde_json::to_value(profile).ok()?;
            value.get(b).cloned()
        }
        "defaultProfile" => Some(Value::String(config.default_profile.clone())),
        "rules" => {
            let index: usize = a.parse().ok()?;
            let rule = config.rules.get(index)?;
            Some(Value::String(rule.profile.clone()))
        }
        _ => Some(Value::Array(
            config
                .ladder
                .iter()
                .map(|profile| Value::String(profile.clone()))
                .collect(),
        )),
    }
}

/// Setzt den Wert in der rohen Einstellung; die Reihenfolge der Schlüssel bleibt erhalten
fn set(raw: &mut Value, path: &str, value: Value) -> Result<()> {
    let parts: Vec<&str> = path.split('.').collect();
    let head = parts.first().copied().unwrap_or("");
    let a = parts.get(1).copied().unwrap_or("");
    let b = parts.get(2).copied().unwrap_or("");
    let object = raw.as_object_mut().ok_or_else(|| {
        HarnessError::new(
            "input-invalid",
            "Die Einstellung ist kein Objekt und kann nicht geändert werden.",
        )
    })?;
    match head {
        "profiles" => {
            let mut profiles = match object.get("profiles").and_then(Value::as_object) {
                Some(profiles) => profiles.clone(),
                None => Map::new(),
            };
            let mut profile = match profiles.get(a).and_then(Value::as_object) {
                Some(profile) => profile.clone(),
                None => Map::new(),
            };
            profile.insert(b.to_string(), value);
            profiles.insert(a.to_string(), Value::Object(profile));
            object.insert("profiles".to_string(), Value::Object(profiles));
        }
        "defaultProfile" => {
            object.insert("defaultProfile".to_string(), value);
        }
        "rules" => {
            let mut rules = match object.get("rules").and_then(Value::as_array) {
                Some(rules) => rules.clone(),
                None => Vec::new(),
            };
            let index: usize = a.parse().unwrap_or(usize::MAX);
            let rule = rules.get(index).and_then(Value::as_object).ok_or_else(|| {
                HarnessError::new(
                    "input-invalid",
                    format!("Regel {} gibt es nicht.", index.saturating_add(1)),
                )
            })?;
            let mut rule = rule.clone();
            rule.insert("profile".to_string(), value);
            rules[index] = Value::Object(rule);
            object.insert("rules".to_string(), Value::Array(rules));
        }
        _ => {
            object.insert("escalation".to_string(), value);
        }
    }
    Ok(())
}

/// `JSON.stringify(value)` für Meldungen und Fingerabdrücke; `None` ergibt wie in
/// JavaScript den Text `undefined`
pub fn js_stringify(value: Option<&Value>) -> String {
    match value {
        None => "undefined".to_string(),
        Some(value) => serde_json::to_string(value).unwrap_or_else(|_| "null".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::dispatch::config_normalizer::DispatchConfigNormalizer;

    /// Rohe Einstellung der TypeScript-Tests
    fn raw() -> Value {
        let mut raw = DispatchConfigNormalizer::new().example();
        let object = raw.as_object_mut().unwrap();
        object.insert(
            "profiles".to_string(),
            json!({
                "klein": { "model": "sonnet", "maxTurns": 20, "timeoutMinutes": 30 },
                "standard": { "model": "opus", "effort": "high", "maxTurns": 30, "timeoutMinutes": 60 },
            }),
        );
        object.insert("defaultProfile".to_string(), json!("standard"));
        object.insert("escalation".to_string(), json!(["klein", "standard"]));
        object.insert(
            "rules".to_string(),
            json!([{ "when": { "labels": ["doku"] }, "profile": "klein" }]),
        );
        object.insert("research".to_string(), json!({ "auto": false, "profile": null }));
        object.insert("planning".to_string(), json!({ "profile": null }));
        raw
    }

    /// Rohe und aufgelöste Einstellung
    fn fixture() -> (Value, DispatchConfig) {
        let raw = raw();
        let config = DispatchConfigNormalizer::new().normalize(&raw).unwrap();
        (raw, config)
    }

    /// Eine Änderung
    fn op(path: &str, from: Value, to: Value) -> ConfigPatchOp {
        ConfigPatchOp {
            path: path.to_string(),
            from: Some(from),
            to,
        }
    }

    #[test]
    fn setzt_profilwerte_ohne_andere_felder_oder_die_vorlage_zu_verandern() {
        let (raw, config) = fixture();
        let next = ConfigPatcher::new()
            .apply(&raw, &config, &[op("profiles.standard.maxTurns", json!(30), json!(45))])
            .unwrap();
        assert_eq!(
            next["profiles"]["standard"],
            json!({ "model": "opus", "effort": "high", "maxTurns": 45, "timeoutMinutes": 60 })
        );
        assert_eq!(raw["profiles"]["standard"]["maxTurns"], json!(30));
        let normalizer = DispatchConfigNormalizer::new();
        assert_eq!(
            normalizer.normalize(&next).unwrap().profiles["standard"].max_turns,
            45
        );
    }

    #[test]
    fn setzt_standardprofil_profil_einer_regel_und_leiter() {
        let (raw, config) = fixture();
        let next = ConfigPatcher::new()
            .apply(
                &raw,
                &config,
                &[
                    op("defaultProfile", json!("standard"), json!("klein")),
                    op("rules.0.profile", json!("klein"), json!("standard")),
                    op(
                        "escalation",
                        json!(["klein", "standard"]),
                        json!(["standard", "klein"]),
                    ),
                ],
            )
            .unwrap();
        assert_eq!(next["defaultProfile"], json!("klein"));
        assert_eq!(next["escalation"], json!(["standard", "klein"]));
        assert_eq!(
            next["rules"][0],
            json!({ "when": { "labels": ["doku"] }, "profile": "standard" })
        );
    }

    #[test]
    fn lehnt_veraltete_ausgangswerte_als_konflikt_ab() {
        let (raw, config) = fixture();
        let error = ConfigPatcher::new()
            .apply(&raw, &config, &[op("profiles.standard.maxTurns", json!(25), json!(45))])
            .unwrap_err();
        assert_eq!(error.code(), Some("conflict"));
        assert_eq!(
            error.harness().unwrap().details,
            Some(json!({ "reason": "patch-stale" }))
        );
    }

    #[test]
    fn lehnt_fremde_pfade_und_fehlende_regeln_ab() {
        let (raw, config) = fixture();
        let patcher = ConfigPatcher::new();
        assert!(!patcher.allowed("claude.command"));
        let fremd = patcher
            .apply(&raw, &config, &[op("claude.command", json!("claude"), json!("x"))])
            .unwrap_err();
        assert_eq!(fremd.code(), Some("input-invalid"));
        // `from` fehlt wie `undefined` in TypeScript: der Ausgangswert stimmt, erst das Setzen
        // scheitert an der fehlenden Regel
        let fehlend = patcher
            .apply(
                &raw,
                &config,
                &[ConfigPatchOp {
                    path: "rules.5.profile".to_string(),
                    from: None,
                    to: json!("klein"),
                }],
            )
            .unwrap_err();
        assert_eq!(fehlend.code(), Some("input-invalid"));
        assert!(fehlend.to_string().contains("Regel 6"), "{fehlend}");
    }
}
