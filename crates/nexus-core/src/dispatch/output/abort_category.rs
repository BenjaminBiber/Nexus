//! Die Kategorien der Abbruchgründe gehören zur Auswertung der Ausgabe, stehen aber schon
//! als [`crate::dispatch::abort_reason`] im Dispatcher – die Lauf-Statistik brauchte sie
//! vor dem Leser. Hier stehen sie als Teil von `dispatch::output` zur Verfügung, dazu die
//! Gegenprobe mit den Gründen, die der [`RunOutputReader`] wirklich formuliert.

pub use crate::dispatch::abort_reason::{AbortCategory, AbortReasonClassifier, ABORT_CATEGORIES};

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{AbortCategory, AbortReasonClassifier};
    use crate::dispatch::output::RunOutputReader;
    use crate::dispatch::{RunResult, RunSettings};

    fn run() -> RunSettings {
        RunSettings {
            model: Some("opus".to_string()),
            effort: Some("high".to_string()),
            max_turns: 80,
            timeout_minutes: 90,
            profile: "standard".to_string(),
            why: String::new(),
            reason: None,
            config_hash: None,
            rule_index: None,
            escalated_from: None,
            allowed_accounts: None,
            accounts_from: None,
            account: None,
            account_why: None,
            usage_at_start: None,
        }
    }

    /// Der Grund so, wie der `RunOutputReader` ihn formuliert (nicht abgetippt)
    fn category(code: Option<i64>, timed_out: bool, output: &str) -> AbortCategory {
        let reader = RunOutputReader::new();
        let result = RunResult {
            code,
            timed_out,
            output: output.to_string(),
            ..RunResult::default()
        };
        let summary = reader.summarize(&result.output);
        AbortReasonClassifier::category(&reader.abort_reason(&result, &summary, &run(), None))
    }

    #[test]
    fn ordnet_die_grunde_des_runoutputreader_genau_einer_kategorie_zu() {
        assert_eq!(category(None, true, ""), AbortCategory::Timeout);
        assert_eq!(
            category(
                Some(0),
                false,
                &json!({ "type": "result", "subtype": "error_max_turns", "is_error": true })
                    .to_string()
            ),
            AbortCategory::MaxTurns
        );
        assert_eq!(
            category(
                Some(1),
                false,
                &json!({ "type": "result", "subtype": "error_during_execution", "is_error": true, "result": "Prompt is too long" }).to_string()
            ),
            AbortCategory::Context
        );
        assert_eq!(category(Some(2), false, ""), AbortCategory::ExitCode);
        assert_eq!(
            category(
                Some(0),
                false,
                &json!({ "type": "result", "subtype": "error_during_execution", "is_error": true })
                    .to_string()
            ),
            AbortCategory::ClaudeError
        );
        assert_eq!(category(Some(0), false, ""), AbortCategory::NoOutcome);
    }

    #[test]
    fn erkennt_abgelehnte_werkzeugaufrufe_nicht_als_eigene_kategorie_sondern_am_grund_davor() {
        let out = json!({
            "type": "result", "subtype": "success", "is_error": false,
            "permission_denials": [{ "tool_name": "Bash" }],
        })
        .to_string();
        assert_eq!(category(Some(0), false, &out), AbortCategory::NoOutcome);
    }

    #[test]
    fn kennt_die_kategorie_kontext_voll() {
        assert!(AbortCategory::ALL.contains(&AbortCategory::Context));
        assert_eq!(
            AbortReasonClassifier::category("Kontext voll: Prompt is too long"),
            AbortCategory::Context
        );
        assert_eq!(AbortReasonClassifier::label("context"), "Kontext voll");
    }
}
