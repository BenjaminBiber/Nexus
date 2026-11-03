//! Wählt Modell, Effort, Schrittzahl und Zeitlimit eines Laufs; portiert aus
//! `packages/core/src/dispatch/ProfileChooser.ts`.
//!
//! Reihenfolge: Label `profil:`, Aufwand im Plan, Regeln, Standardprofil; danach Eskalation je
//! Abbruch und die Labels `model:` und `effort:`.

use std::sync::LazyLock;

use regex::Regex;

use crate::accounts::is_account_id;
use crate::board::{DispatchProfileReason, Priority, Ticket};
use crate::dispatch::config::{DispatchConfig, Rule, RunSettings};
use crate::dispatch::config_normalizer::DispatchConfigNormalizer;
use crate::dispatch::fingerprint::DispatchConfigFingerprint;

/// Überschrift „## Integration“ im Plan
static INTEGRATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^##\s+Integration\b").unwrap());
/// Irgendeine Überschrift der zweiten Ebene
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^##\s").unwrap());
/// Trennzeile einer Markdown-Tabelle
static TABLE_SEPARATOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\|[\s:|-]+\|?$").unwrap());
/// Zeile „Aufwand: <profil>“ (auch fett)
static ESTIMATE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^[ \t]*\**Aufwand\**[ \t]*:\**[ \t]*\**([\p{L}\w-]+)").unwrap()
});
/// Abschnitt „## Aufwand“ mit dem Wort darunter
static ESTIMATE_SECTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?im)^##[ \t]+Aufwand[ \t]*\r?\n\s*\**([\p{L}\w-]+)").unwrap());
/// Haiku kennt keinen Effort
static HAIKU: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(^|-)haiku").unwrap());

/// Die Felder eines Tickets, die für die Profilwahl zählen
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileTicket {
    pub labels: Vec<String>,
    pub priority: Priority,
    pub plan: String,
}

impl ProfileTicket {
    /// Ticket mit Labels, Priorität und Plan
    pub fn new(labels: Vec<String>, priority: Priority, plan: impl Into<String>) -> Self {
        Self {
            labels,
            priority,
            plan: plan.into(),
        }
    }
}

impl From<&Ticket> for ProfileTicket {
    fn from(ticket: &Ticket) -> Self {
        Self {
            labels: ticket.labels.clone(),
            priority: ticket.priority,
            plan: ticket.plan.clone(),
        }
    }
}

/// Die Labels `profil:`, `model:`, `effort:` und `account:` eines Tickets – jeweils das erste
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DispatchLabels {
    pub profile: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub account: Option<String>,
}

/// Wählt die Einstellungen eines Laufs aus Einstellung und Ticket.
#[derive(Debug, Clone)]
pub struct ProfileChooser<'a> {
    cfg: &'a DispatchConfig,
    /// Hash der Einstellung dieses Choosers, einmal berechnet: jeder Lauf trägt die Fassung,
    /// die bei seiner Wahl galt
    config_hash: String,
}

impl<'a> ProfileChooser<'a> {
    /// Chooser für diese Einstellung
    pub fn new(cfg: &'a DispatchConfig) -> Self {
        Self {
            config_hash: DispatchConfigFingerprint::new().hash(cfg),
            cfg,
        }
    }

    /// Hash der Einstellung, unter der dieser Chooser wählt (Version fürs Dashboard)
    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    /// Labels `profil:`, `model:`, `effort:` und `account:` – jeweils das erste
    pub fn labels(&self, labels: &[String]) -> DispatchLabels {
        let value = |prefix: &str| {
            labels
                .iter()
                .find(|label| label.starts_with(prefix))
                .map(|label| label[prefix.len()..].trim().to_string())
                .filter(|value| !value.is_empty())
        };
        DispatchLabels {
            profile: value("profil:"),
            model: value("model:"),
            effort: value("effort:"),
            account: value("account:"),
        }
    }

    /// Ein Label, mit dem nicht gestartet werden kann – lieber stehen lassen als raten
    pub fn label_problem(&self, labels: &[String]) -> Option<String> {
        let normalizer = DispatchConfigNormalizer::new();
        let l = self.labels(labels);
        if let Some(profile) = &l.profile {
            if !self.cfg.profiles.contains_key(profile) {
                return Some(format!(
                    "Label profil:{profile} – dieses Profil gibt es nicht ({})",
                    self.cfg
                        .profiles
                        .keys()
                        .cloned()
                        .collect::<Vec<String>>()
                        .join(", ")
                ));
            }
        }
        if let Some(model) = &l.model {
            if let Some(problem) = normalizer.model_problem(model, self.cfg.allow_fable) {
                return Some(format!("Label model:{model} – {problem}"));
            }
        }
        if let Some(effort) = &l.effort {
            if let Some(problem) = normalizer.effort_problem(effort) {
                return Some(format!("Label effort:{effort} – {problem}"));
            }
        }
        if let Some(account) = &l.account {
            if !is_account_id(account) {
                return Some(format!(
                    "Label account:{account} – keine gültige Account-ID"
                ));
            }
        }
        None
    }

    /// Zeilen der Tabelle unter „## Integration“ (ohne Kopf und Trenner)
    pub fn plan_file_count(&self, plan: &str) -> i64 {
        let lines: Vec<&str> = plan
            .split('\n')
            .map(|line| line.trim_end_matches('\r'))
            .collect();
        let Some(start) = lines
            .iter()
            .position(|line| INTEGRATION.is_match(line.trim()))
        else {
            return 0;
        };
        let mut rows = 0;
        let mut separator = false;
        for line in &lines[start + 1..] {
            let t = line.trim();
            if HEADING.is_match(t) {
                break;
            }
            if !t.starts_with('|') {
                continue;
            }
            if TABLE_SEPARATOR.is_match(t) {
                separator = true;
                continue;
            }
            if separator {
                rows += 1;
            }
        }
        rows
    }

    /// Einschätzung des Planners: Zeile „Aufwand: <profil>“ (auch fett) oder Abschnitt
    /// „## Aufwand“; klein geschrieben
    pub fn plan_estimate(&self, plan: &str) -> Option<String> {
        if let Some(line) = ESTIMATE_LINE.captures(plan) {
            return Some(line[1].to_lowercase());
        }
        ESTIMATE_SECTION
            .captures(plan)
            .map(|section| section[1].to_lowercase())
    }

    /// Einstellungen für Versuch `attempt` (ab 1)
    pub fn choose(&self, ticket: &ProfileTicket, attempt: i64) -> RunSettings {
        let normalizer = DispatchConfigNormalizer::new();
        let labels = self.labels(&ticket.labels);
        let mut profile = self.cfg.default_profile.clone();
        let mut why = format!("Standardprofil {profile}");
        // Grund der Wahl des Basisprofils fürs Dashboard; Eskalation und Labels model:/effort:
        // ändern ihn nicht
        let mut reason = DispatchProfileReason::Default;
        let mut rule_index: Option<i64> = None;
        let estimate = self.plan_estimate(&ticket.plan);
        let label_profile = labels
            .profile
            .as_ref()
            .filter(|name| self.cfg.profiles.contains_key(name.as_str()));
        let estimate_profile = estimate
            .as_ref()
            .filter(|name| self.cfg.profiles.contains_key(name.as_str()));
        if let Some(name) = label_profile {
            profile = name.clone();
            why = format!("Label profil:{profile}");
            reason = DispatchProfileReason::Label;
        } else if let Some(name) = estimate_profile {
            // Der Planner hat den Code gerade gelesen – seine Einschätzung schlägt jede Regel
            profile = name.clone();
            why = format!("Aufwand laut Plan: {profile}");
            reason = DispatchProfileReason::Plan;
        } else {
            if let Some(index) = self
                .cfg
                .rules
                .iter()
                .position(|rule| self.matches(rule, ticket))
            {
                profile = self.cfg.rules[index].profile.clone();
                why = format!("Regel {} → {profile}", index + 1);
                reason = DispatchProfileReason::Rule;
                rule_index = Some(index as i64 + 1);
            }
            if let Some(estimate) = &estimate {
                why.push_str(&format!(" (Aufwand „{estimate}“ im Plan ist kein Profil)"));
            }
        }
        let mut escalated_from: Option<String> = None;
        if self.cfg.escalate && attempt > 1 {
            if let Some(at) = self.cfg.ladder.iter().position(|name| *name == profile) {
                let index = (at + attempt as usize - 1).min(self.cfg.ladder.len() - 1);
                let target = self.cfg.ladder[index].clone();
                if target != profile {
                    why.push_str(&format!(", Versuch {attempt}: eskaliert auf {target}"));
                    escalated_from = Some(profile);
                    profile = target;
                }
            }
        }
        let base = &self.cfg.profiles[&profile];
        let mut run = RunSettings {
            model: base.model.clone(),
            effort: base.effort.clone(),
            max_turns: base.max_turns,
            timeout_minutes: base.timeout_minutes,
            profile,
            why,
            reason: Some(reason),
            config_hash: Some(self.config_hash.clone()),
            rule_index,
            escalated_from,
            allowed_accounts: None,
            accounts_from: None,
            account: None,
            account_why: None,
            usage_at_start: None,
        };
        if let Some(model) = &labels.model {
            if normalizer
                .model_problem(model, self.cfg.allow_fable)
                .is_none()
            {
                run.model = Some(model.clone());
                run.why.push_str(&format!(", Label model:{model}"));
            }
        }
        if let Some(effort) = &labels.effort {
            if normalizer.effort_problem(effort).is_none() {
                run.effort = Some(effort.clone());
                run.why.push_str(&format!(", Label effort:{effort}"));
            }
        }
        // Accounts: Label account: vor der Regel (nur ohne Eskalation – das eskalierte Profil
        // bringt seine eigenen mit) vor dem Profil
        let rule = run
            .rule_index
            .filter(|_| run.escalated_from.is_none())
            .and_then(|index| self.cfg.rules.get(index as usize - 1));
        let label_account = labels.account.as_ref().filter(|id| is_account_id(id));
        if let Some(account) = label_account {
            let from = format!("Label account:{account}");
            allow(&mut run, Some(std::slice::from_ref(account)), &from);
        } else if let Some(accounts) = rule.and_then(|rule| rule.accounts.as_deref()) {
            let from = format!("Regel {}", run.rule_index.unwrap_or_default());
            allow(&mut run, Some(accounts), &from);
        } else {
            let from = format!("Profil {}", run.profile);
            let accounts = self.cfg.profiles[&run.profile].accounts.clone();
            allow(&mut run, accounts.as_deref(), &from);
        }
        // Haiku kennt keinen Effort
        if run.model.as_deref().is_some_and(|m| HAIKU.is_match(m)) {
            run.effort = None;
        }
        run
    }

    /// Einstellungen eines Recherche-Laufs: `research.profile`, sonst Profil „research“,
    /// sonst Standardprofil
    pub fn research(&self) -> RunSettings {
        let (profile, why, reason) = match &self.cfg.research.profile {
            Some(profile) => (
                profile.clone(),
                format!("Einstellung research.profile: {profile}"),
                DispatchProfileReason::Setting,
            ),
            None if self.cfg.profiles.contains_key("research") => (
                "research".to_string(),
                "Profil research".to_string(),
                DispatchProfileReason::Setting,
            ),
            None => (
                self.cfg.default_profile.clone(),
                format!("Standardprofil {}", self.cfg.default_profile),
                DispatchProfileReason::Default,
            ),
        };
        self.run_of(profile, why, reason)
    }

    /// Einstellungen eines Planungslaufs: `planning.profile`, sonst Profil „planning“,
    /// sonst Standardprofil
    pub fn planning(&self) -> RunSettings {
        let (profile, why, reason) = match &self.cfg.planning.profile {
            Some(profile) => (
                profile.clone(),
                format!("Einstellung planning.profile: {profile}"),
                DispatchProfileReason::Setting,
            ),
            None if self.cfg.profiles.contains_key("planning") => (
                "planning".to_string(),
                "Profil planning".to_string(),
                DispatchProfileReason::Setting,
            ),
            None => (
                self.cfg.default_profile.clone(),
                format!("Standardprofil {}", self.cfg.default_profile),
                DispatchProfileReason::Default,
            ),
        };
        self.run_of(profile, why, reason)
    }

    /// Lauf-Einstellungen aus einem Profil samt Begründung
    fn run_of(&self, profile: String, why: String, reason: DispatchProfileReason) -> RunSettings {
        let base = &self.cfg.profiles[&profile];
        let mut run = RunSettings {
            model: base.model.clone(),
            effort: base.effort.clone(),
            max_turns: base.max_turns,
            timeout_minutes: base.timeout_minutes,
            profile,
            why,
            reason: Some(reason),
            config_hash: Some(self.config_hash.clone()),
            rule_index: None,
            escalated_from: None,
            allowed_accounts: None,
            accounts_from: None,
            account: None,
            account_why: None,
            usage_at_start: None,
        };
        let from = format!("Profil {}", run.profile);
        allow(&mut run, base.accounts.as_deref(), &from);
        if run.model.as_deref().is_some_and(|m| HAIKU.is_match(m)) {
            run.effort = None;
        }
        run
    }

    /// Trifft die Regel zu? Alle genannten Bedingungen müssen passen
    fn matches(&self, rule: &Rule, ticket: &ProfileTicket) -> bool {
        let w = &rule.when;
        if let Some(labels) = &w.labels {
            if !labels.iter().any(|label| ticket.labels.contains(label)) {
                return false;
            }
        }
        if let Some(priority) = &w.priority {
            if !priority.iter().any(|p| p == ticket.priority.as_str()) {
                return false;
            }
        }
        if let Some(files_min) = w.files_min {
            if self.plan_file_count(&ticket.plan) < files_min {
                return false;
            }
        }
        if let Some(plan_min_chars) = w.plan_min_chars {
            // JavaScript zählt UTF-16-Einheiten; hier zählen Zeichen – die Grenze dient nur der
            // groben Abschätzung der Plangröße
            if (ticket.plan.chars().count() as i64) < plan_min_chars {
                return false;
            }
        }
        true
    }
}

/// Erlaubte Accounts samt Herkunft eintragen; ohne Liste bleibt jeder aktive Account erlaubt
fn allow(run: &mut RunSettings, accounts: Option<&[String]>, from: &str) {
    run.allowed_accounts = None;
    run.accounts_from = None;
    let Some(accounts) = accounts.filter(|accounts| !accounts.is_empty()) else {
        return;
    };
    run.allowed_accounts = Some(accounts.to_vec());
    run.accounts_from = Some(from.to_string());
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::dispatch::config_normalizer::DispatchConfigNormalizer;

    /// Die aufgelöste Beispiel-Einstellung
    fn example() -> DispatchConfig {
        let normalizer = DispatchConfigNormalizer::new();
        normalizer.normalize(&normalizer.example()).unwrap()
    }

    /// Ticket mit Standardwerten für die Profilwahl
    fn ticket(labels: &[&str], priority: Priority, plan: &str) -> ProfileTicket {
        ProfileTicket::new(
            labels.iter().map(|l| (*l).to_string()).collect(),
            priority,
            plan,
        )
    }

    /// Ticket ohne Labels, mit normaler Priorität und ohne Plan
    fn plain() -> ProfileTicket {
        ticket(&[], Priority::Normal, "")
    }

    /// Plan mit einer Integrationstabelle über `rows` Zeilen
    fn integration(rows: usize) -> String {
        let mut lines = vec![
            "## Integration".to_string(),
            "| Datei | Änderung |".to_string(),
            "|---|---|".to_string(),
        ];
        for index in 0..rows {
            lines.push(format!("| f{index}.ts | x |"));
        }
        lines.push("## Tests".to_string());
        lines.push("| a | b |".to_string());
        lines.join("\n")
    }

    #[test]
    fn nimmt_ohne_hinweise_das_standardprofil() {
        let cfg = example();
        let run = ProfileChooser::new(&cfg).choose(&plain(), 1);
        assert_eq!(run.profile, "standard");
        assert_eq!(run.model.as_deref(), Some("opus"));
        assert_eq!(run.effort.as_deref(), Some("high"));
        assert_eq!(run.max_turns, 80);
        assert_eq!(run.timeout_minutes, 90);
        assert_eq!(run.why, "Standardprofil standard");
    }

    #[test]
    fn label_profil_schlagt_aufwand_und_regeln() {
        let cfg = example();
        let run = ProfileChooser::new(&cfg).choose(
            &ticket(
                &["profil:klein", "architektur"],
                Priority::Normal,
                "Aufwand: schwer",
            ),
            1,
        );
        assert_eq!(run.profile, "klein");
    }

    #[test]
    fn liest_den_aufwand_des_planners_als_zeile_oder_abschnitt() {
        let cfg = example();
        let chooser = ProfileChooser::new(&cfg);
        assert_eq!(
            chooser.plan_estimate("**Aufwand:** Schwer – viele Dateien"),
            Some("schwer".to_string())
        );
        assert_eq!(
            chooser.plan_estimate("## Aufwand\n\nklein, nur Doku"),
            Some("klein".to_string())
        );
        assert_eq!(chooser.plan_estimate("kein Hinweis"), None);
        let run = chooser.choose(
            &ticket(&["architektur"], Priority::Normal, "## Aufwand\nklein"),
            1,
        );
        assert_eq!(run.profile, "klein");
        assert_eq!(run.why, "Aufwand laut Plan: klein");
    }

    #[test]
    fn nennt_einen_aufwand_der_kein_profil_ist_und_nimmt_die_regeln() {
        let cfg = example();
        let run = ProfileChooser::new(&cfg)
            .choose(&ticket(&["doku"], Priority::Normal, "Aufwand: riesig"), 1);
        assert_eq!(run.profile, "klein");
        assert_eq!(
            run.why,
            "Regel 1 → klein (Aufwand „riesig“ im Plan ist kein Profil)"
        );
    }

    #[test]
    fn wendet_regeln_nach_labels_prioritat_und_dateizahl_an() {
        let cfg = example();
        let chooser = ProfileChooser::new(&cfg);
        assert_eq!(
            chooser
                .choose(&ticket(&[], Priority::Urgent, ""), 1)
                .profile,
            "schwer"
        );
        assert_eq!(chooser.plan_file_count(&integration(6)), 6);
        assert_eq!(
            chooser
                .choose(&ticket(&[], Priority::Normal, &integration(6)), 1)
                .profile,
            "schwer"
        );
        assert_eq!(
            chooser
                .choose(&ticket(&[], Priority::Normal, &integration(5)), 1)
                .profile,
            "standard"
        );
    }

    #[test]
    fn eskaliert_je_abbruch_eine_stufe_hochstens_bis_zum_ende_der_leiter() {
        let cfg = example();
        let chooser = ProfileChooser::new(&cfg);
        let run = chooser.choose(&ticket(&["doku"], Priority::Normal, ""), 2);
        assert_eq!(run.profile, "standard");
        assert_eq!(
            run.why,
            "Regel 1 → klein, Versuch 2: eskaliert auf standard"
        );
        assert_eq!(
            chooser
                .choose(&ticket(&["doku"], Priority::Normal, ""), 5)
                .profile,
            "schwer"
        );
        let normalizer = DispatchConfigNormalizer::new();
        let mut raw = normalizer.example();
        raw["escalate"] = json!(false);
        let aus = normalizer.normalize(&raw).unwrap();
        assert_eq!(
            ProfileChooser::new(&aus)
                .choose(&ticket(&["doku"], Priority::Normal, ""), 3)
                .profile,
            "klein"
        );
    }

    #[test]
    fn merkt_sich_das_basisprofil_einer_eskalation() {
        let cfg = example();
        let chooser = ProfileChooser::new(&cfg);
        assert_eq!(
            chooser
                .choose(&ticket(&["doku"], Priority::Normal, ""), 2)
                .escalated_from
                .as_deref(),
            Some("klein")
        );
        assert_eq!(
            chooser
                .choose(&ticket(&["doku"], Priority::Normal, ""), 1)
                .escalated_from,
            None
        );
        // Am Ende der Leiter bleibt das Profil gleich – keine Eskalation
        assert_eq!(
            chooser
                .choose(&ticket(&["profil:schwer"], Priority::Normal, ""), 3)
                .escalated_from,
            None
        );
    }

    #[test]
    fn labels_model_und_effort_uberschreiben_einzelne_werte() {
        let cfg = example();
        let chooser = ProfileChooser::new(&cfg);
        let run = chooser.choose(
            &ticket(&["model:sonnet", "effort:xhigh"], Priority::Normal, ""),
            1,
        );
        assert_eq!(run.model.as_deref(), Some("sonnet"));
        assert_eq!(run.effort.as_deref(), Some("xhigh"));
        assert_eq!(
            run.why,
            "Standardprofil standard, Label model:sonnet, Label effort:xhigh"
        );
        assert_eq!(
            chooser
                .choose(&ticket(&["model:haiku"], Priority::Normal, ""), 1)
                .effort,
            None
        );
    }

    #[test]
    fn nennt_den_grund_der_profilwahl_und_bei_einer_regel_ihre_nummer() {
        let cfg = example();
        let chooser = ProfileChooser::new(&cfg);
        assert_eq!(
            chooser.choose(&plain(), 1).reason,
            Some(DispatchProfileReason::Default)
        );
        assert_eq!(chooser.choose(&plain(), 1).rule_index, None);
        assert_eq!(
            chooser
                .choose(&ticket(&["profil:klein"], Priority::Normal, ""), 1)
                .reason,
            Some(DispatchProfileReason::Label)
        );
        assert_eq!(
            chooser
                .choose(&ticket(&[], Priority::Normal, "## Aufwand\nklein"), 1)
                .reason,
            Some(DispatchProfileReason::Plan)
        );
        // Regel 3 der Beispiel-Einstellung greift bei Priorität urgent
        let urgent = chooser.choose(&ticket(&[], Priority::Urgent, ""), 1);
        assert_eq!(urgent.profile, "schwer");
        assert_eq!(urgent.why, "Regel 3 → schwer");
        assert_eq!(urgent.reason, Some(DispatchProfileReason::Rule));
        assert_eq!(urgent.rule_index, Some(3));
        let doku = chooser.choose(&ticket(&["doku"], Priority::Normal, ""), 1);
        assert_eq!(doku.reason, Some(DispatchProfileReason::Rule));
        assert_eq!(doku.rule_index, Some(1));
    }

    #[test]
    fn eskalation_und_labels_lassen_den_grund_des_basisprofils_stehen() {
        let cfg = example();
        let chooser = ProfileChooser::new(&cfg);
        let run = chooser.choose(&ticket(&["doku"], Priority::Normal, ""), 2);
        assert_eq!(run.profile, "standard");
        assert_eq!(run.escalated_from.as_deref(), Some("klein"));
        assert_eq!(run.reason, Some(DispatchProfileReason::Rule));
        assert_eq!(run.rule_index, Some(1));
        assert_eq!(
            chooser
                .choose(
                    &ticket(&["model:sonnet", "effort:xhigh"], Priority::Normal, ""),
                    1
                )
                .reason,
            Some(DispatchProfileReason::Default)
        );
    }

    #[test]
    fn meldet_unbrauchbare_labels() {
        let cfg = example();
        let chooser = ProfileChooser::new(&cfg);
        let problem = |label: &str| chooser.label_problem(&[label.to_string()]);
        assert!(problem("profil:gross").unwrap().contains("profil:gross"));
        assert!(problem("model:gpt").unwrap().contains("Unbekanntes Modell"));
        assert!(problem("model:fable").unwrap().contains("Fable"));
        assert!(problem("effort:mega").unwrap().contains("effort"));
        assert_eq!(problem("profil:klein"), None);
    }

    #[test]
    fn recherche_nimmt_profil_einstellung_oder_standard() {
        let normalizer = DispatchConfigNormalizer::new();
        let cfg = example();
        let chooser = ProfileChooser::new(&cfg);
        assert_eq!(chooser.research().profile, "standard");
        assert_eq!(chooser.research().why, "Standardprofil standard");
        assert_eq!(
            chooser.research().reason,
            Some(DispatchProfileReason::Default)
        );

        let mut raw = normalizer.example();
        raw["profiles"]["research"] = json!({ "model": "sonnet", "effort": "medium" });
        let mit_profil = normalizer.normalize(&raw).unwrap();
        let run = ProfileChooser::new(&mit_profil).research();
        assert_eq!(run.profile, "research");
        assert_eq!(run.model.as_deref(), Some("sonnet"));
        assert_eq!(run.effort.as_deref(), Some("medium"));
        assert_eq!(run.reason, Some(DispatchProfileReason::Setting));

        let mut raw = normalizer.example();
        raw["research"] = json!({ "profile": "klein" });
        let gewahlt = normalizer.normalize(&raw).unwrap();
        let run = ProfileChooser::new(&gewahlt).research();
        assert_eq!(run.profile, "klein");
        assert_eq!(run.why, "Einstellung research.profile: klein");
        assert_eq!(run.reason, Some(DispatchProfileReason::Setting));
    }

    #[test]
    fn planung_nimmt_profil_einstellung_oder_standard() {
        let normalizer = DispatchConfigNormalizer::new();
        let cfg = example();
        let chooser = ProfileChooser::new(&cfg);
        assert_eq!(chooser.planning().profile, "standard");
        assert_eq!(chooser.planning().why, "Standardprofil standard");
        assert_eq!(
            chooser.planning().reason,
            Some(DispatchProfileReason::Default)
        );

        let mut raw = normalizer.example();
        raw["profiles"]["planning"] = json!({ "model": "opus", "effort": "max" });
        let mit_profil = normalizer.normalize(&raw).unwrap();
        let run = ProfileChooser::new(&mit_profil).planning();
        assert_eq!(run.profile, "planning");
        assert_eq!(run.model.as_deref(), Some("opus"));
        assert_eq!(run.effort.as_deref(), Some("max"));
        assert_eq!(run.reason, Some(DispatchProfileReason::Setting));

        let mut raw = normalizer.example();
        raw["planning"] = json!({ "profile": "schwer" });
        let gewahlt = normalizer.normalize(&raw).unwrap();
        let run = ProfileChooser::new(&gewahlt).planning();
        assert_eq!(run.profile, "schwer");
        assert_eq!(run.why, "Einstellung planning.profile: schwer");
    }

    #[test]
    fn hangt_den_hash_der_einstellung_an_alle_laufe() {
        let cfg = example();
        let chooser = ProfileChooser::new(&cfg);
        let hash = chooser.config_hash().to_string();
        assert_eq!(hash.len(), 16);
        assert!(hash
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        assert_eq!(
            chooser.choose(&plain(), 1).config_hash.as_deref(),
            Some(hash.as_str())
        );
        assert_eq!(
            chooser.research().config_hash.as_deref(),
            Some(hash.as_str())
        );
        assert_eq!(
            chooser.planning().config_hash.as_deref(),
            Some(hash.as_str())
        );

        let normalizer = DispatchConfigNormalizer::new();
        let mut raw = normalizer.example();
        raw["defaultProfile"] = json!("klein");
        let anders = normalizer.normalize(&raw).unwrap();
        assert_ne!(ProfileChooser::new(&anders).config_hash(), hash);
    }
}
