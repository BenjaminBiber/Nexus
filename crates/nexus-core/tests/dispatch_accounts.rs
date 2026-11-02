//! Accounts in der Dispatcher-Einstellung: Prüfung, Profilwahl, Umgebung des Workers und
//! Verweise. Nachgebildet aus `packages/core/tests/unit/dispatch/DispatchAccounts.test.ts`
//! im Branch `claude-accounts-ts`.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use nexus_core::board::Priority;
use nexus_core::dispatch::{
    limit_account, limit_pause, AccountReferences, AccountStrategy, AttemptCounter,
    ClaudeInvocation, DispatchConfig, DispatchConfigFingerprint, DispatchConfigNormalizer,
    ProfileChooser, ProfileTicket, RunSettings,
};

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

/// Die aufgelöste Einstellung
fn cfg() -> DispatchConfig {
    DispatchConfigNormalizer::new().normalize(&raw()).unwrap()
}

/// Ein Ticket mit diesen Labels
fn ticket(labels: &[&str]) -> ProfileTicket {
    ProfileTicket::new(
        labels.iter().map(ToString::to_string).collect(),
        Priority::Normal,
        "",
    )
}

/// Die erlaubten Accounts und ihre Herkunft
fn allowed(run: &RunSettings) -> (Option<Vec<String>>, Option<String>) {
    (run.allowed_accounts.clone(), run.accounts_from.clone())
}

/// Die Meldung des Prüffehlers
fn problem(raw: &Value) -> String {
    DispatchConfigNormalizer::new()
        .normalize(raw)
        .unwrap_err()
        .harness()
        .expect("Prüffehler")
        .message
        .clone()
}

#[test]
fn liest_accounts_je_profil_und_regel_ohne_doppelte() {
    let cfg = cfg();
    assert_eq!(
        cfg.profiles["klein"].accounts.as_deref(),
        Some(["pro-1".to_string(), "pro-2".to_string()].as_slice())
    );
    assert_eq!(cfg.profiles["standard"].accounts, None);
    assert_eq!(
        cfg.rules[0].accounts.as_deref(),
        Some(["firma".to_string()].as_slice())
    );
    assert_eq!(cfg.rules[1].accounts, None);
    assert_eq!(cfg.account_strategy, AccountStrategy::MostFree);
    assert_eq!(cfg.account_limit_percent, 90);
}

#[test]
fn lehnt_ungueltige_ids_strategien_und_grenzen_ab() {
    assert!(
        problem(&json!({ "profiles": { "a": { "accounts": ["Böse"] } } }))
            .contains("keine Account-ID")
    );
    assert!(problem(&json!({ "accountStrategy": "zufall" })).contains("accountStrategy"));
    assert!(problem(&json!({ "accountLimitPercent": 20 })).contains("accountLimitPercent"));
}

#[test]
fn aendert_den_confighash_einer_einstellung_ohne_accounts_nicht() {
    let plain = DispatchConfigNormalizer::new()
        .normalize(&json!({ "profiles": { "standard": { "model": "opus" } } }))
        .unwrap();
    assert!(!DispatchConfigFingerprint::new()
        .content(&plain)
        .contains("accounts"));
}

#[test]
fn ein_profil_ohne_accounts_erlaubt_jeden_aktiven_account() {
    let cfg = cfg();
    assert_eq!(
        allowed(&ProfileChooser::new(&cfg).choose(&ticket(&[]), 1)),
        (None, None)
    );
}

#[test]
fn profil_regel_und_label_bestimmen_die_erlaubten_accounts() {
    let cfg = cfg();
    let chooser = ProfileChooser::new(&cfg);
    assert_eq!(
        allowed(&chooser.choose(&ticket(&["profil:schwer"]), 1)),
        (
            Some(vec!["max".to_string()]),
            Some("Profil schwer".to_string())
        )
    );
    let regel = chooser.choose(&ticket(&["firma"]), 1);
    assert_eq!(regel.profile, "standard");
    assert_eq!(
        allowed(&regel),
        (Some(vec!["firma".to_string()]), Some("Regel 1".to_string()))
    );
    assert_eq!(
        allowed(&chooser.choose(&ticket(&["firma", "account:pro-2"]), 1)),
        (
            Some(vec!["pro-2".to_string()]),
            Some("Label account:pro-2".to_string())
        )
    );
}

#[test]
fn bei_der_eskalation_bringt_das_groessere_profil_seine_accounts_mit() {
    let cfg = cfg();
    let chooser = ProfileChooser::new(&cfg);
    let aus_doku = chooser.choose(&ticket(&["doku"]), 3);
    assert_eq!(aus_doku.profile, "schwer");
    assert_eq!(aus_doku.allowed_accounts, Some(vec!["max".to_string()]));
    let aus_firma = chooser.choose(&ticket(&["firma"]), 2);
    assert_eq!(aus_firma.profile, "schwer");
    assert_eq!(aus_firma.allowed_accounts, Some(vec!["max".to_string()]));
}

#[test]
fn planung_und_recherche_uebernehmen_die_accounts_ihres_profils() {
    let mut raw = raw();
    raw["planning"] = json!({ "profile": "schwer" });
    let cfg = DispatchConfigNormalizer::new().normalize(&raw).unwrap();
    let chooser = ProfileChooser::new(&cfg);
    assert_eq!(
        chooser.planning().allowed_accounts,
        Some(vec!["max".to_string()])
    );
    assert_eq!(chooser.research().allowed_accounts, None);
}

#[test]
fn meldet_ein_ungueltiges_label_account() {
    let cfg = cfg();
    let problem = ProfileChooser::new(&cfg)
        .label_problem(&["account:Böse ID".to_string()])
        .expect("Befund");
    assert!(problem.contains("keine gültige Account-ID"));
}

#[test]
fn worker_umgebung_bekommt_das_claude_config_dir_des_accounts() {
    let cfg = cfg();
    let invocation = ClaudeInvocation::new();
    let env: BTreeMap<String, String> =
        BTreeMap::from([("CLAUDE_CONFIG_DIR".to_string(), "/server".to_string())]);
    assert_eq!(
        invocation.env(&env, &cfg, None, None, Some("/a/max"))["CLAUDE_CONFIG_DIR"],
        "/a/max"
    );
    assert_eq!(
        invocation.env(&env, &cfg, None, None, None)["CLAUDE_CONFIG_DIR"],
        "/server"
    );
    assert!(!invocation
        .env(&BTreeMap::new(), &cfg, None, None, None)
        .contains_key("CLAUDE_CONFIG_DIR"));
}

#[test]
fn zaehlt_limit_pausen_je_account() {
    let cfg = cfg();
    let counter = AttemptCounter::new(&cfg);
    let pause = limit_pause();
    let von = |body: String| ("dispatcher".to_string(), body);
    let comments = vec![
        von(format!(
            "{pause} von Account max erreicht, Ticket zurück nach Geplant."
        )),
        von("[dispatch] Start: an junior-ARC-1".to_string()),
        von(format!(
            "{pause} von Account pro-1 erreicht, Ticket zurück nach Geplant."
        )),
        von(format!(
            "{pause} von Account max erreicht, Ticket zurück nach Geplant."
        )),
    ];
    assert_eq!(counter.limit_pauses_in_row(&comments, Some("max")), 2);
    assert_eq!(counter.limit_pauses_in_row(&comments, Some("pro-1")), 1);
    assert_eq!(counter.limit_pauses_in_row(&comments, None), 3);
    assert_eq!(limit_account(&format!("{pause} erreicht")), None);
}

#[test]
fn findet_und_entfernt_verweise_auf_einen_account() {
    let refs = AccountReferences::new();
    let gefunden = refs.find(&raw());
    assert_eq!(
        gefunden["pro-1"],
        vec!["Profil klein".to_string(), "Profil klein".to_string()]
    );
    assert_eq!(gefunden["pro-2"], vec!["Profil klein".to_string()]);
    assert_eq!(gefunden["max"], vec!["Profil schwer".to_string()]);
    assert_eq!(gefunden["firma"], vec!["Regel 1".to_string()]);

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
