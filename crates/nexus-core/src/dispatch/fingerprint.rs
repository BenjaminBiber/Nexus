//! Hash der wirksamen Dispatcher-Einstellung; portiert aus
//! `packages/core/src/dispatch/DispatchConfigFingerprint.ts`.
//!
//! Der Hash benennt die Fassung, unter der ein Lauf gestartet ist. Nur Werte, die das Ergebnis
//! beeinflussen – Fenster, WIP oder Boards ändern ihn nicht.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::dispatch::config::DispatchConfig;

/// Teile der Einstellung, die in den Hash gehören; alles andere (Fenster, WIP, Boards, Namen)
/// ändert das Ergebnis eines Laufs nicht
const KEYS: [&str; 14] = [
    "profiles",
    "defaultProfile",
    "rules",
    "escalate",
    "ladder",
    "maxAttempts",
    "allowFable",
    "promptExtra",
    "prompts",
    "mergeStrategy",
    "reviewBeforeMerge",
    "joinWorktrees",
    "worktreeSetup",
    "worktreeSetupTimeoutMinutes",
];

/// Teile von `claude`, die ein Lauf tatsächlich spürt (command, useSubscription und gitBashPath
/// gehören zur Maschine, nicht zum Lauf)
const CLAUDE_KEYS: [&str; 7] = [
    "model",
    "effort",
    "maxTurns",
    "timeoutMinutes",
    "permissionMode",
    "allowedTools",
    "extraArgs",
];

/// Hash der wirksamen Dispatcher-Einstellung.
#[derive(Debug, Clone, Copy, Default)]
pub struct DispatchConfigFingerprint;

impl DispatchConfigFingerprint {
    /// Neuer Fingerabdruck (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Die beachteten Teile als JSON mit sortierten Schlüsseln; Arrays behalten ihre Reihenfolge
    /// (rules und ladder sind geordnet)
    pub fn content(&self, cfg: &DispatchConfig) -> String {
        let full = serde_json::to_value(cfg).expect("Einstellung ist serialisierbar");
        let mut subset = Map::new();
        for key in KEYS {
            subset.insert(key.to_string(), full[key].clone());
        }
        let mut claude = Map::new();
        for key in CLAUDE_KEYS {
            claude.insert(key.to_string(), full["claude"][key].clone());
        }
        subset.insert("claude".to_string(), Value::Object(claude));
        subset.insert(
            "research".to_string(),
            json!({ "profile": cfg.research.profile }),
        );
        subset.insert(
            "planning".to_string(),
            json!({ "profile": cfg.planning.profile }),
        );
        // `JSON.stringify` ohne Leerzeichen – serde_json schreibt genauso kompakt
        serde_json::to_string(&sorted(Value::Object(subset))).expect("JSON ist schreibbar")
    }

    /// Die ersten 16 Hex-Zeichen des SHA-256 über `content` – kurz genug für Anzeige und Filter,
    /// lang genug gegen Kollisionen
    pub fn hash(&self, cfg: &DispatchConfig) -> String {
        let digest = Sha256::digest(self.content(cfg).as_bytes());
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        hex[..16].to_string()
    }
}

/// Objekte mit sortierten Schlüsseln neu aufbauen, damit die Reihenfolge in der Datei den Hash
/// nicht ändert
fn sorted(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        Value::Object(object) => {
            let mut keys: Vec<String> = object.keys().cloned().collect();
            keys.sort();
            let mut out = Map::new();
            for key in keys {
                let item = object.get(&key).cloned().unwrap_or(Value::Null);
                out.insert(key, sorted(item));
            }
            Value::Object(out)
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::config_normalizer::DispatchConfigNormalizer;

    /// Die aufgelöste Beispiel-Einstellung
    fn base() -> DispatchConfig {
        let normalizer = DispatchConfigNormalizer::new();
        normalizer.normalize(&normalizer.example()).unwrap()
    }

    /// Die Beispiel-Einstellung mit einer Änderung
    fn with_change(change: impl FnOnce(&mut DispatchConfig)) -> DispatchConfig {
        let mut cfg = base();
        change(&mut cfg);
        cfg
    }

    #[test]
    fn gibt_16_hex_zeichen_zuruck_und_bleibt_fur_dieselbe_einstellung_gleich() {
        let fingerprint = DispatchConfigFingerprint::new();
        let hash = fingerprint.hash(&base());
        assert_eq!(hash.len(), 16);
        assert!(hash
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        assert_eq!(fingerprint.hash(&base()), hash);
    }

    #[test]
    fn ist_unabhangig_von_der_reihenfolge_der_schlussel() {
        let fingerprint = DispatchConfigFingerprint::new();
        let cfg = base();
        // Profile in umgekehrter Reihenfolge neu aufbauen: fachlich dieselbe Einstellung
        let umgekehrt = with_change(|cfg| {
            cfg.profiles = cfg
                .profiles
                .iter()
                .rev()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
        });
        assert_eq!(fingerprint.hash(&umgekehrt), fingerprint.hash(&cfg));
    }

    #[test]
    fn andert_sich_bei_profilen_regeln_leiter_und_claude_einstellungen() {
        let fingerprint = DispatchConfigFingerprint::new();
        let hash = fingerprint.hash(&base());
        let first = base().profiles.keys().next().unwrap().clone();
        assert_ne!(
            fingerprint.hash(&with_change(|c| c.profiles[&first].max_turns += 1)),
            hash
        );
        assert_ne!(fingerprint.hash(&with_change(|c| c.rules.reverse())), hash);
        assert_ne!(fingerprint.hash(&with_change(|c| c.ladder.reverse())), hash);
        assert_ne!(
            fingerprint.hash(&with_change(|c| c.max_attempts += 1)),
            hash
        );
        assert_ne!(
            fingerprint.hash(&with_change(|c| c
                .claude
                .allowed_tools
                .push("Bash".to_string()))),
            hash
        );
        assert_ne!(
            fingerprint.hash(&with_change(
                |c| c.research.profile = Some("klein".to_string())
            )),
            hash
        );
    }

    #[test]
    fn andert_sich_nicht_bei_fenster_wip_boards_und_namen() {
        let fingerprint = DispatchConfigFingerprint::new();
        let hash = fingerprint.hash(&base());
        assert_eq!(fingerprint.hash(&with_change(|c| c.wip += 3)), hash);
        assert_eq!(
            fingerprint.hash(&with_change(|c| c.window =
                Some(crate::dispatch::config::TimeWindow {
                    from: "08:00".to_string(),
                    to: "18:00".to_string(),
                }))),
            hash
        );
        assert_eq!(
            fingerprint.hash(&with_change(|c| c.boards = Some(vec![1, 2]))),
            hash
        );
        assert_eq!(
            fingerprint.hash(&with_change(|c| c.enabled = !c.enabled)),
            hash
        );
        assert_eq!(
            fingerprint.hash(&with_change(|c| c.dispatcher_name = "anders".to_string())),
            hash
        );
        assert_eq!(
            fingerprint.hash(&with_change(
                |c| c.claude.command = "C:/claude.exe".to_string()
            )),
            hash
        );
    }

    #[test]
    fn andert_sich_mit_einer_eigenen_prompt_vorlage() {
        let fingerprint = DispatchConfigFingerprint::new();
        let hash = fingerprint.hash(&base());
        assert_eq!(
            base().prompts,
            crate::dispatch::config::PromptTemplates::default()
        );
        assert_ne!(
            fingerprint.hash(&with_change(
                |c| c.prompts.work = Some("eigene Vorlage {{agent}}".to_string())
            )),
            hash
        );
        assert_ne!(
            fingerprint.hash(&with_change(
                |c| c.prompts.research = Some("andere Vorlage {{agent}}".to_string())
            )),
            hash
        );
        // Zwei verschiedene Vorlagen derselben Laufart ergeben verschiedene Hashes
        assert_ne!(
            fingerprint.hash(&with_change(
                |c| c.prompts.work = Some("A {{agent}}".to_string())
            )),
            fingerprint.hash(&with_change(
                |c| c.prompts.work = Some("B {{agent}}".to_string())
            ))
        );
    }

    #[test]
    fn liefert_den_inhalt_als_json_mit_sortierten_schlusseln() {
        let content: Value =
            serde_json::from_str(&DispatchConfigFingerprint::new().content(&base())).unwrap();
        let keys: Vec<&String> = content.as_object().unwrap().keys().collect();
        let mut sortiert = keys.clone();
        sortiert.sort();
        assert_eq!(keys, sortiert);
        let claude: Vec<&String> = content["claude"].as_object().unwrap().keys().collect();
        assert_eq!(
            claude,
            [
                "allowedTools",
                "effort",
                "extraArgs",
                "maxTurns",
                "model",
                "permissionMode",
                "timeoutMinutes"
            ]
        );
        assert!(content.get("wip").is_none());
        assert_eq!(content["defaultProfile"], json!(base().default_profile));
    }

    /// Referenzwerte aus dem TypeScript-Stand; erzeugt mit einem einmaligen tsx-Skript, das
    /// `DispatchConfigFingerprint.hash` über die drei Einstellungen laufen ließ
    /// (`normalize({})`, `normalize(example())` und das Beispiel mit eigener work-Vorlage und
    /// promptExtra).
    #[test]
    fn stimmt_mit_den_referenzwerten_aus_typescript_uberein() {
        let normalizer = DispatchConfigNormalizer::new();
        let fingerprint = DispatchConfigFingerprint::new();

        let leer = normalizer.normalize(&json!({})).unwrap();
        assert_eq!(fingerprint.hash(&leer), "9d2e3a0591b37576");

        let beispiel = normalizer.normalize(&normalizer.example()).unwrap();
        assert_eq!(fingerprint.hash(&beispiel), "2a279428522a285a");

        let mut raw = normalizer.example();
        raw["promptExtra"] = json!("Hausregel: kein any.");
        raw["prompts"] = json!({
            "work": crate::dispatch::prompt_defaults::WORK_TEMPLATE,
            "plan": null,
            "research": null,
            "merge": null,
        });
        let eigene = normalizer.normalize(&raw).unwrap();
        assert_eq!(fingerprint.hash(&eigene), "58dc3a07ba753e05");
    }

    /// Der gehashte Inhalt der leeren Einstellung, Zeichen für Zeichen wie in TypeScript
    #[test]
    fn inhalt_der_leeren_einstellung_ist_zeichengleich() {
        let normalizer = DispatchConfigNormalizer::new();
        let cfg = normalizer.normalize(&json!({})).unwrap();
        assert_eq!(
            DispatchConfigFingerprint::new().content(&cfg),
            r#"{"allowFable":false,"claude":{"allowedTools":["Read","Edit","Write","Glob","Grep","Bash(harness:*)","Bash(node:*)","Bash(npm test:*)","Bash(npm run:*)","Bash(git status:*)","Bash(git diff:*)","Bash(git log:*)","Bash(git branch:*)","Bash(git add:*)","Bash(git commit:*)"],"effort":null,"extraArgs":[],"maxTurns":80,"model":"sonnet","permissionMode":"acceptEdits","timeoutMinutes":60},"defaultProfile":"standard","escalate":true,"joinWorktrees":"none","ladder":["standard"],"maxAttempts":2,"mergeStrategy":"merge","planning":{"profile":null},"profiles":{"standard":{"effort":null,"maxTurns":80,"model":"sonnet","timeoutMinutes":60}},"promptExtra":"","prompts":{"merge":null,"plan":null,"research":null,"work":null},"research":{"profile":null},"reviewBeforeMerge":true,"rules":[],"worktreeSetup":[],"worktreeSetupTimeoutMinutes":15}"#
        );
    }
}
