//! Prüft die rohe Einstellung und füllt sie mit Vorgaben auf; portiert aus
//! `packages/core/src/dispatch/DispatchConfigNormalizer.ts`.
//!
//! Die rohe Einstellung bleibt ein `serde_json::Value`: `GET /api/dispatch/config` gibt sie
//! unverändert weiter, deshalb darf die Reihenfolge der Schlüssel nicht verloren gehen.

use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;
use serde_json::{json, Value};

use crate::accounts::is_account_id;
use crate::dispatch::config::{
    AccountStrategy, ClaudeSettings, DispatchConfig, JoinWorktrees, MergeStrategy,
    PlanningSettings, Profile, PromptProblem, PromptTemplates, ResearchSettings, Rule,
    RuleCondition, TimeWindow, WorktreeSetupStep, ACCOUNT_STRATEGIES, DEFAULT_ALLOWED_TOOLS,
    EFFORT_LEVELS, JOIN_WORKTREES, MERGE_STRATEGIES, MODEL_ALIASES, PROMPT_KINDS,
};
use crate::dispatch::js_value::{field, field_or_null, is_integer, js_number, js_string};
use crate::dispatch::prompt_check::{PromptKind, PromptTemplateCheck};
use crate::dispatch::prompt_defaults;
use crate::dispatch::prompt_template::PromptTemplate;
use crate::error::{Error, HarnessError, Result};

/// Obergrenze gleichzeitiger Recherche-Läufe
const MAX_RESEARCH_WIP: i64 = 5;
/// Obergrenze gleichzeitiger Planungsläufe
const MAX_PLANNING_WIP: i64 = 5;

static HHMM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([01]\d|2[0-3]):([0-5]\d)$").unwrap());

/// Voller Modellname statt eines Alias
static FULL_MODEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^claude-[a-z0-9.-]+(\[1m\])?$").unwrap());

/// Erkennt Fable (auch in vollen Namen), unabhängig von Groß- und Kleinschreibung
static FABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)fable").unwrap());

/// Absoluter Pfad: Laufwerksbuchstabe oder führender Schrägstrich
static ABSOLUTE_DIR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([a-zA-Z]:|/)").unwrap());

/// Vorgaben des Abschnitts `claude`
fn claude_defaults() -> Value {
    json!({
        "command": "claude",
        "model": "sonnet",
        "effort": null,
        "maxTurns": 80,
        "timeoutMinutes": 60,
        "permissionMode": "acceptEdits",
        "allowedTools": DEFAULT_ALLOWED_TOOLS,
        "useSubscription": true,
        "extraArgs": [],
        "gitBashPath": null,
    })
}

/// Prüft die rohe Einstellung und füllt sie mit Vorgaben auf.
#[derive(Debug, Clone, Copy, Default)]
pub struct DispatchConfigNormalizer;

impl DispatchConfigNormalizer {
    /// Neuer Normalizer (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Vollständig aufgelöste Einstellung oder `HarnessError` „dispatch-config-invalid“
    pub fn normalize(&self, raw: &Value) -> Result<DispatchConfig> {
        let input = if raw.is_object() { raw } else { &Value::Null };
        let claude = self.claude(input)?;
        let allow_fable = field_or_null(input, "allowFable") == &Value::Bool(true);
        if let Some(model) = &claude.model {
            check(self.model_problem(model, allow_fable), "claude.model")?;
        }
        if let Some(effort) = &claude.effort {
            check(self.effort_problem(effort), "claude.effort")?;
        }

        let profiles = self.profiles(input, &claude, allow_fable)?;
        let names: Vec<String> = profiles.keys().cloned().collect();
        let default_profile = {
            let wanted = text(field_or_null(input, "defaultProfile")).unwrap_or_else(|| {
                if profiles.contains_key("standard") {
                    "standard".to_string()
                } else {
                    names.first().cloned().unwrap_or_default()
                }
            });
            known(&profiles, &wanted, "defaultProfile")?
        };
        let ladder = match field_or_null(input, "escalation").as_array() {
            Some(items) => {
                let mut ladder = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    ladder.push(known(
                        &profiles,
                        &js_string(item),
                        &format!("escalation[{index}]"),
                    )?);
                }
                ladder
            }
            None => names.clone(),
        };
        let rules = self.rules(input, &profiles)?;
        let research = self.research(field_or_null(input, "research"), &profiles)?;
        let planning = self.planning(field_or_null(input, "planning"), &profiles)?;

        // Ein laufender Worker darf nie als verwaist gelten: Schwelle immer über dem längsten Zeitlimit
        let longest = profiles
            .values()
            .map(|profile| profile.timeout_minutes)
            .fold(claude.timeout_minutes, i64::max);
        let stale_raw = match field(input, "staleMinutes") {
            None => 0,
            Some(Value::Null) => 0,
            Some(value) => int(value, "staleMinutes", 1, None)?,
        };
        let stale_minutes = if stale_raw >= longest + 10 {
            stale_raw
        } else {
            longest + 30
        };

        Ok(DispatchConfig {
            enabled: field_or_null(input, "enabled") == &Value::Bool(true),
            boards: self.boards(field(input, "boards"))?,
            wip: int(
                coalesce(field_or_null(input, "wip"), &json!(1)),
                "wip",
                0,
                None,
            )?,
            window: self.window(field(input, "window"))?,
            opt_in_label: match field(input, "optInLabel") {
                None => Some("auto".to_string()),
                Some(value) => text(value),
            },
            opt_out_label: match field(input, "optOutLabel") {
                None => Some("manuell".to_string()),
                Some(value) => text(value),
            },
            agent_prefix: text(field_or_null(input, "agentPrefix"))
                .unwrap_or_else(|| "junior-".to_string()),
            dispatcher_name: text(field_or_null(input, "dispatcherName"))
                .unwrap_or_else(|| "dispatcher".to_string()),
            stale_minutes,
            max_attempts: int(
                coalesce(field_or_null(input, "maxAttempts"), &json!(2)),
                "maxAttempts",
                1,
                None,
            )?,
            pause_minutes_on_limit: int(
                coalesce(field_or_null(input, "pauseMinutesOnLimit"), &json!(60)),
                "pauseMinutesOnLimit",
                1,
                None,
            )?,
            prompt_extra: field_or_null(input, "promptExtra")
                .as_str()
                .unwrap_or_default()
                .to_string(),
            prompts: self.prompts(field_or_null(input, "prompts"))?,
            merge_strategy: self.merge_strategy(field_or_null(input, "mergeStrategy"))?,
            review_before_merge: self
                .review_before_merge(field_or_null(input, "reviewBeforeMerge"))?,
            join_worktrees: self.join_worktrees(field_or_null(input, "joinWorktrees"))?,
            worktree_setup: self.worktree_setup(field_or_null(input, "worktreeSetup"))?,
            worktree_setup_timeout_minutes: int(
                coalesce(
                    field_or_null(input, "worktreeSetupTimeoutMinutes"),
                    &json!(15),
                ),
                "worktreeSetupTimeoutMinutes",
                1,
                None,
            )?,
            claude,
            profiles,
            default_profile,
            rules,
            escalate: field_or_null(input, "escalate") != &Value::Bool(false),
            ladder,
            allow_fable,
            research,
            planning,
            account_strategy: self.account_strategy(field_or_null(input, "accountStrategy"))?,
            account_limit_percent: int(
                coalesce(field_or_null(input, "accountLimitPercent"), &json!(90)),
                "accountLimitPercent",
                50,
                Some(100),
            )?,
            account_reserve_five_hour: int(
                coalesce(field_or_null(input, "accountReserveFiveHour"), &json!(10)),
                "accountReserveFiveHour",
                0,
                Some(50),
            )?,
            account_reserve_week: int(
                coalesce(field_or_null(input, "accountReserveWeek"), &json!(2)),
                "accountReserveWeek",
                0,
                Some(50),
            )?,
            account_reset_grace_minutes: int(
                coalesce(field_or_null(input, "accountResetGraceMinutes"), &json!(30)),
                "accountResetGraceMinutes",
                0,
                Some(300),
            )?,
        })
    }

    /// Beispiel-Einstellung wie `dispatch.js init` des alten Boards (Dispatcher aus, Opt-in `auto`)
    pub fn example(&self) -> Value {
        json!({
            "enabled": false,
            "boards": null,
            "wip": 1,
            "window": null,
            "optInLabel": "auto",
            "optOutLabel": "manuell",
            "maxAttempts": 2,
            "pauseMinutesOnLimit": 60,
            "mergeStrategy": "merge",
            "reviewBeforeMerge": true,
            "joinWorktrees": "none",
            // Beispiel: [{ cwd: 'BackofficeNextGen', run: 'npm ci --prefer-offline --no-audit' }]
            "worktreeSetup": [],
            "worktreeSetupTimeoutMinutes": 15,
            "profiles": {
                "klein": { "model": "sonnet", "effort": "high", "maxTurns": 40, "timeoutMinutes": 45 },
                "standard": { "model": "opus", "effort": "high", "maxTurns": 80, "timeoutMinutes": 90 },
                "schwer": { "model": "opus", "effort": "max", "maxTurns": 150, "timeoutMinutes": 120 },
            },
            "defaultProfile": "standard",
            "rules": [
                { "when": { "labels": ["doku", "tippfehler", "klein"] }, "profile": "klein" },
                { "when": { "labels": ["architektur", "refactoring"] }, "profile": "schwer" },
                { "when": { "priority": ["urgent"] }, "profile": "schwer" },
                { "when": { "filesMin": 6 }, "profile": "schwer" },
            ],
            "escalate": true,
            "allowFable": false,
            // Mehrere Claude-Accounts (Einstellungen → Accounts): Profile und Regeln können sie
            // mit accounts: [id, …] festlegen
            "accountStrategy": "most-free",
            "accountLimitPercent": 90,
            "accountReserveFiveHour": 10,
            "accountReserveWeek": 2,
            "accountResetGraceMinutes": 30,
            "research": { "auto": false, "wip": 1, "maxPerDay": 10 },
            "planning": { "auto": false, "wip": 1, "acceptance": true, "profile": null },
            "promptExtra": "",
            // Eigene Vorlagen; null = die Standardvorlage der jeweiligen Laufart
            "prompts": { "work": null, "plan": null, "research": null, "merge": null },
            "claude": {
                "command": "claude",
                "permissionMode": "acceptEdits",
                "useSubscription": true,
                "allowedTools": DEFAULT_ALLOWED_TOOLS,
                "extraArgs": [],
            },
        })
    }

    /// Befunde zu den eigenen Vorlagen der rohen Einstellung; leer = alles in Ordnung.
    /// Für das Speichern gedacht: `normalize` verwirft eine fehlerhafte Vorlage still,
    /// `write` lehnt sie ab.
    pub fn prompt_problems(&self, raw: &Value) -> Result<Vec<PromptProblem>> {
        let mut problems = Vec::new();
        for (kind, text) in self.prompt_entries(field_or_null(raw, "prompts"))? {
            if let Some(problem) = self.template_problem(kind, &text) {
                problems.push(problem);
            }
        }
        Ok(problems)
    }

    /// Der Fehler zu Befunden an eigenen Vorlagen – derselbe beim Speichern wie in der Vorschau.
    /// `details` nennt die erste betroffene Laufart samt fehlenden und unbekannten Teilen, damit
    /// die Oberfläche sie anzeigen kann, ohne die Meldung zu zerlegen.
    pub fn prompt_error(problems: &[PromptProblem]) -> HarnessError {
        let first = &problems[0];
        let message = problems
            .iter()
            .map(|problem| {
                let mut teile: Vec<String> = Vec::new();
                if !problem.unknown_placeholders.is_empty() {
                    let names: Vec<String> = problem
                        .unknown_placeholders
                        .iter()
                        .map(|name| format!("{{{{{name}}}}}"))
                        .collect();
                    teile.push(format!("unbekannte Platzhalter {}", names.join(", ")));
                }
                if !problem.missing_parts.is_empty() {
                    teile.push(format!(
                        "fehlende Pflichtteile: {}",
                        problem.missing_parts.join("; ")
                    ));
                }
                format!("Vorlage „{}“: {}.", problem.kind, teile.join("; "))
            })
            .collect::<Vec<String>>()
            .join("\n");
        HarnessError::new("dispatch-prompt-invalid", message).with_details(json!({
            "kind": first.kind,
            "missing": first.missing_parts,
            "unknown": first.unknown_placeholders,
            "problems": problems,
        }))
    }

    /// Warum dieser Modellname nicht geht – oder `None`
    pub fn model_problem(&self, model: &str, allow_fable: bool) -> Option<String> {
        let known = MODEL_ALIASES.contains(&model) || FULL_MODEL.is_match(model);
        if !known {
            return Some(format!(
                "Unbekanntes Modell „{model}“ (Alias wie sonnet/opus/haiku oder voller Name claude-…)"
            ));
        }
        if (FABLE.is_match(model) || model == "best") && !allow_fable {
            return Some(format!(
                "Fable („{model}“) ist nicht freigegeben – im Headless-Modus kann es ohne Rückfrage über Usage-Credits abrechnen. Freigeben mit allowFable: true"
            ));
        }
        None
    }

    /// Warum diese Effort-Stufe nicht geht – oder `None`
    pub fn effort_problem(&self, effort: &str) -> Option<String> {
        if EFFORT_LEVELS.contains(&effort) {
            None
        } else {
            Some(format!(
                "Unbekannte effort-Stufe „{effort}“ ({})",
                EFFORT_LEVELS.join(", ")
            ))
        }
    }

    /// Abschnitt `claude` aus Vorgaben und Angaben der Datei
    fn claude(&self, input: &Value) -> Result<ClaudeSettings> {
        let mut merged = claude_defaults();
        if let (Some(target), Some(source)) = (
            merged.as_object_mut(),
            field_or_null(input, "claude").as_object(),
        ) {
            for (key, value) in source {
                target.insert(key.clone(), value.clone());
            }
        }
        let c = &merged;
        Ok(ClaudeSettings {
            command: text(field_or_null(c, "command")).unwrap_or_else(|| "claude".to_string()),
            model: text(field_or_null(c, "model")),
            effort: text(field_or_null(c, "effort")),
            max_turns: int(field_or_null(c, "maxTurns"), "claude.maxTurns", 1, None)?,
            timeout_minutes: int(
                field_or_null(c, "timeoutMinutes"),
                "claude.timeoutMinutes",
                1,
                None,
            )?,
            permission_mode: text(field_or_null(c, "permissionMode")),
            allowed_tools: match field_or_null(c, "allowedTools").as_array() {
                Some(items) => items
                    .iter()
                    .map(|item| js_string(item).trim().to_string())
                    .filter(|tool| !tool.is_empty())
                    .collect(),
                None => DEFAULT_ALLOWED_TOOLS
                    .iter()
                    .map(|t| t.to_string())
                    .collect(),
            },
            use_subscription: field_or_null(c, "useSubscription") != &Value::Bool(false),
            extra_args: match field_or_null(c, "extraArgs").as_array() {
                Some(items) => items.iter().map(js_string).collect(),
                None => Vec::new(),
            },
            git_bash_path: text(field_or_null(c, "gitBashPath")),
        })
    }

    /// Profile; ohne Angabe genau eines („standard“) aus den claude-Vorgaben
    fn profiles(
        &self,
        input: &Value,
        claude: &ClaudeSettings,
        allow_fable: bool,
    ) -> Result<IndexMap<String, Profile>> {
        let fallback = json!({ "standard": {} });
        let raw = match field(input, "profiles").and_then(Value::as_object) {
            Some(object) if !object.is_empty() => object,
            _ => fallback.as_object().expect("Objekt"),
        };
        let mut profiles = IndexMap::new();
        for (name, p) in raw {
            let where_ = format!("profiles.{name}");
            let model = match field(p, "model") {
                None => claude.model.clone(),
                Some(value) => text(value),
            };
            let effort = match field(p, "effort") {
                None => claude.effort.clone(),
                Some(value) => text(value),
            };
            if let Some(model) = &model {
                check(self.model_problem(model, allow_fable), &where_)?;
            }
            if let Some(effort) = &effort {
                check(self.effort_problem(effort), &where_)?;
            }
            let accounts = self.accounts(field(p, "accounts"), &format!("{where_}.accounts"))?;
            profiles.insert(
                name.clone(),
                Profile {
                    accounts,
                    model,
                    effort,
                    max_turns: match field(p, "maxTurns") {
                        None | Some(Value::Null) => claude.max_turns,
                        Some(value) => int(value, &format!("{where_}.maxTurns"), 1, None)?,
                    },
                    timeout_minutes: match field(p, "timeoutMinutes") {
                        None | Some(Value::Null) => claude.timeout_minutes,
                        Some(value) => int(value, &format!("{where_}.timeoutMinutes"), 1, None)?,
                    },
                },
            );
        }
        Ok(profiles)
    }

    /// Regeln; jede braucht mindestens eine Bedingung und ein bekanntes Profil
    fn rules(&self, input: &Value, profiles: &IndexMap<String, Profile>) -> Result<Vec<Rule>> {
        let Some(items) = field_or_null(input, "rules").as_array() else {
            return Ok(Vec::new());
        };
        let mut rules = Vec::with_capacity(items.len());
        for (index, raw) in items.iter().enumerate() {
            let where_ = format!("rules[{index}]");
            let when = field_or_null(raw, "when");
            let mut cond = RuleCondition::default();
            if let Some(labels) = field(when, "labels").filter(|value| !value.is_null()) {
                cond.labels = Some(string_list(labels));
            }
            if let Some(priority) = field(when, "priority").filter(|value| !value.is_null()) {
                cond.priority = Some(string_list(priority));
            }
            if let Some(files_min) = field(when, "filesMin").filter(|value| !value.is_null()) {
                cond.files_min = Some(int(files_min, &format!("{where_}.when.filesMin"), 1, None)?);
            }
            if let Some(plan_min) = field(when, "planMinChars").filter(|value| !value.is_null()) {
                cond.plan_min_chars = Some(int(
                    plan_min,
                    &format!("{where_}.when.planMinChars"),
                    1,
                    None,
                )?);
            }
            if cond.is_empty() {
                return Err(fail(format!(
                    "{where_}: „when“ braucht mindestens eine Bedingung."
                )));
            }
            let profile = match field(raw, "profile") {
                None | Some(Value::Null) => String::new(),
                Some(value) => js_string(value),
            };
            let accounts = self.accounts(field(raw, "accounts"), &format!("{where_}.accounts"))?;
            rules.push(Rule {
                when: cond,
                profile: known(profiles, &profile, &where_)?,
                accounts,
            });
        }
        Ok(rules)
    }

    /// Recherchen: Automatik (Vorgabe aus), gleichzeitige Läufe (1–5, Vorgabe 1), automatische
    /// Läufe je Tag (Vorgabe 10), Profil (muss es geben, wenn genannt)
    fn research(
        &self,
        value: &Value,
        profiles: &IndexMap<String, Profile>,
    ) -> Result<ResearchSettings> {
        if !value.is_null() && !value.is_object() {
            return Err(fail("research muss ein Objekt sein (auto, wip, profile)."));
        }
        let profile = text(field_or_null(value, "profile"));
        Ok(ResearchSettings {
            auto: field_or_null(value, "auto") == &Value::Bool(true),
            wip: int(
                coalesce(field_or_null(value, "wip"), &json!(1)),
                "research.wip",
                1,
                Some(MAX_RESEARCH_WIP),
            )?,
            max_per_day: int(
                coalesce(field_or_null(value, "maxPerDay"), &json!(10)),
                "research.maxPerDay",
                1,
                None,
            )?,
            profile: match profile {
                Some(name) => Some(known(profiles, &name, "research.profile")?),
                None => None,
            },
        })
    }

    /// Planungen: Automatik (Vorgabe aus), gleichzeitige Läufe (1–5, Vorgabe 1), Abnahme mit
    /// füllen (Vorgabe an), Profil (muss es geben, wenn genannt)
    fn planning(
        &self,
        value: &Value,
        profiles: &IndexMap<String, Profile>,
    ) -> Result<PlanningSettings> {
        if !value.is_null() && !value.is_object() {
            return Err(fail(
                "planning muss ein Objekt sein (auto, wip, acceptance, profile).",
            ));
        }
        let profile = text(field_or_null(value, "profile"));
        Ok(PlanningSettings {
            auto: field_or_null(value, "auto") == &Value::Bool(true),
            wip: int(
                coalesce(field_or_null(value, "wip"), &json!(1)),
                "planning.wip",
                1,
                Some(MAX_PLANNING_WIP),
            )?,
            acceptance: field_or_null(value, "acceptance") != &Value::Bool(false),
            profile: match profile {
                Some(name) => Some(known(profiles, &name, "planning.profile")?),
                None => None,
            },
        })
    }

    /// Board-IDs als Zahlen; leer oder fehlend = aktives Board
    fn boards(&self, value: Option<&Value>) -> Result<Option<Vec<i64>>> {
        let Some(value) = value.filter(|value| !value.is_null()) else {
            return Ok(None);
        };
        let Some(items) = value.as_array() else {
            return Err(fail("boards muss eine Liste von Board-IDs sein."));
        };
        if items.is_empty() {
            return Ok(None);
        }
        let mut boards = Vec::with_capacity(items.len());
        for item in items {
            match js_number(item) {
                Some(number) if is_integer(number) && number >= 1.0 => boards.push(number as i64),
                _ => {
                    return Err(fail(format!(
                        "boards: „{}“ ist keine Board-ID.",
                        js_string(item)
                    )))
                }
            }
        }
        Ok(Some(boards))
    }

    /// Zeitfenster HH:MM–HH:MM oder kein Fenster
    fn window(&self, value: Option<&Value>) -> Result<Option<TimeWindow>> {
        let Some(value) = value.filter(|value| !value.is_null()) else {
            return Ok(None);
        };
        let from = field_or_null(value, "from")
            .as_str()
            .unwrap_or_default()
            .to_string();
        let to = field_or_null(value, "to")
            .as_str()
            .unwrap_or_default()
            .to_string();
        if !HHMM.is_match(&from) || !HHMM.is_match(&to) {
            return Err(fail(
                "window braucht from und to im Format HH:MM, z. B. from: \"18:00\", to: \"07:00\".",
            ));
        }
        Ok(Some(TimeWindow { from, to }))
    }

    /// Account-IDs eines Profils oder einer Regel (ohne doppelte); leer oder fehlend = `None`
    /// (jeder aktive bzw. wie das Profil). Ob es die Accounts gibt, prüft erst die Wahl –
    /// `accounts.json` kann sich unabhängig von `dispatch.yaml` ändern
    fn accounts(&self, value: Option<&Value>, where_: &str) -> Result<Option<Vec<String>>> {
        let Some(value) = value.filter(|value| !value.is_null()) else {
            return Ok(None);
        };
        let Some(items) = value.as_array() else {
            return Err(fail(format!(
                "{where_} muss eine Liste von Account-IDs sein."
            )));
        };
        let mut ids: Vec<String> = Vec::new();
        for raw in items {
            let id = js_string(raw).trim().to_string();
            if !is_account_id(&id) {
                return Err(fail(format!("{where_}: „{id}“ ist keine Account-ID.")));
            }
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        Ok(if ids.is_empty() { None } else { Some(ids) })
    }

    /// Wahl unter mehreren Accounts; fehlend = meiste freie Kapazität
    fn account_strategy(&self, value: &Value) -> Result<AccountStrategy> {
        if value.is_null() {
            return Ok(AccountStrategy::MostFree);
        }
        let wanted = value.as_str().unwrap_or_default().trim();
        ACCOUNT_STRATEGIES
            .into_iter()
            .find(|strategy| strategy.as_str() == wanted)
            .ok_or_else(|| {
                fail(format!(
                    "accountStrategy muss {} sein (nicht „{}“).",
                    ACCOUNT_STRATEGIES
                        .map(|strategy| strategy.as_str())
                        .join(" oder "),
                    js_string(value)
                ))
            })
    }

    /// Merge-Strategie; fehlend = Merge-Commit
    fn merge_strategy(&self, value: &Value) -> Result<MergeStrategy> {
        if value.is_null() {
            return Ok(MergeStrategy::Merge);
        }
        let wanted = value.as_str().unwrap_or_default().trim();
        MERGE_STRATEGIES
            .into_iter()
            .find(|strategy| strategy.as_str() == wanted)
            .ok_or_else(|| {
                fail(format!(
                    "mergeStrategy muss {} sein (nicht „{}“).",
                    MERGE_STRATEGIES
                        .map(|strategy| strategy.as_str())
                        .join(" oder "),
                    js_string(value)
                ))
            })
    }

    /// Freigabe vor dem Merge; fehlend = warten
    fn review_before_merge(&self, value: &Value) -> Result<bool> {
        match value {
            Value::Null => Ok(true),
            Value::Bool(flag) => Ok(*flag),
            other => Err(fail(format!(
                "reviewBeforeMerge muss true oder false sein (nicht „{}“).",
                js_string(other)
            ))),
        }
    }

    /// Einrichtungsbefehle: run nicht leer, cwd relativ zum Worktree und darin (kein absoluter
    /// Pfad, kein „..“); fehlend = keine
    fn worktree_setup(&self, value: &Value) -> Result<Vec<WorktreeSetupStep>> {
        if value.is_null() {
            return Ok(Vec::new());
        }
        let Some(items) = value.as_array() else {
            return Err(fail("worktreeSetup muss eine Liste von { cwd, run } sein."));
        };
        let mut steps = Vec::with_capacity(items.len());
        for (index, raw) in items.iter().enumerate() {
            let where_ = format!("worktreeSetup[{index}]");
            if !raw.is_object() {
                return Err(fail(format!(
                    "{where_} muss ein Objekt {{ cwd, run }} sein."
                )));
            }
            let Some(run) = text(field_or_null(raw, "run")) else {
                return Err(fail(format!("{where_}: run (Befehl) fehlt.")));
            };
            let cwd = field_or_null(raw, "cwd");
            if !cwd.is_null() && !cwd.is_string() {
                return Err(fail(format!(
                    "{where_}: cwd muss ein Pfad relativ zum Worktree sein."
                )));
            }
            steps.push(WorktreeSetupStep {
                cwd: relative_dir(cwd.as_str().unwrap_or_default(), &where_)?,
                run,
            });
        }
        Ok(steps)
    }

    /// Worktrees bündeln; fehlend = ein Worktree je Ticket
    fn join_worktrees(&self, value: &Value) -> Result<JoinWorktrees> {
        if value.is_null() {
            return Ok(JoinWorktrees::None);
        }
        let wanted = value.as_str().unwrap_or_default().trim();
        JOIN_WORKTREES
            .into_iter()
            .find(|join| join.as_str() == wanted)
            .ok_or_else(|| {
                fail(format!(
                    "joinWorktrees muss {} sein (nicht „{}“).",
                    JOIN_WORKTREES.map(|join| join.as_str()).join(" oder "),
                    js_string(value)
                ))
            })
    }

    /// Die angegebenen Vorlagen als Paare (Laufart, Text); leere und nicht gesetzte fallen weg.
    /// Struktur-Fehler (kein Objekt, unbekannte Laufart) bleiben hart, nur der Inhalt einer
    /// Vorlage ist weich.
    fn prompt_entries(&self, value: &Value) -> Result<Vec<(PromptKind, String)>> {
        if value.is_null() {
            return Ok(Vec::new());
        }
        let Some(object) = value.as_object() else {
            return Err(fail(
                "prompts muss ein Objekt sein (work, plan, research, merge).",
            ));
        };
        let mut entries = Vec::new();
        for (key, text) in object {
            let Some(kind) = PromptKind::parse(key) else {
                return Err(fail(format!(
                    "prompts: „{key}“ ist keine Laufart ({}).",
                    PROMPT_KINDS.map(|kind| kind.as_str()).join(", ")
                )));
            };
            let Some(text) = text.as_str().filter(|text| !text.trim().is_empty()) else {
                continue;
            };
            // Zeilenenden vereinheitlichen, damit eine Datei von Windows den configHash nicht verschiebt
            entries.push((kind, text.replace("\r\n", "\n")));
        }
        Ok(entries)
    }

    /// Eigene Vorlagen; was die Prüfung nicht besteht, wird `None` (= Standardvorlage)
    fn prompts(&self, value: &Value) -> Result<PromptTemplates> {
        let mut prompts = PromptTemplates::default();
        for (kind, text) in self.prompt_entries(value)? {
            if self.template_problem(kind, &text).is_none() {
                prompts.set(kind, Some(text));
            }
        }
        Ok(prompts)
    }

    /// Was an dieser Vorlage nicht stimmt – oder `None`
    fn template_problem(&self, kind: PromptKind, text: &str) -> Option<PromptProblem> {
        let allowed: Vec<String> = prompt_defaults::placeholders(kind)
            .into_iter()
            .map(|placeholder| placeholder.name)
            .collect();
        let unknown_placeholders: Vec<String> = PromptTemplate::new()
            .names(text)
            .into_iter()
            .filter(|name| !allowed.contains(name))
            .collect();
        let missing_parts: Vec<String> = PromptTemplateCheck::new()
            .missing(kind, text)
            .into_iter()
            .map(|part| part.label)
            .collect();
        if unknown_placeholders.is_empty() && missing_parts.is_empty() {
            return None;
        }
        Some(PromptProblem {
            kind,
            unknown_placeholders,
            missing_parts,
        })
    }
}

/// Der Wert, falls er gesetzt ist – sonst der Ersatz (`??` aus JavaScript)
fn coalesce<'a>(value: &'a Value, fallback: &'a Value) -> &'a Value {
    if value.is_null() {
        fallback
    } else {
        value
    }
}

/// Die Liste als Texte; alles andere ergibt eine leere Liste
fn string_list(value: &Value) -> Vec<String> {
    match value.as_array() {
        Some(items) => items.iter().map(js_string).collect(),
        None => Vec::new(),
    }
}

/// Das Profil gibt es – sonst ein Prüffehler, der alle bekannten nennt
fn known(profiles: &IndexMap<String, Profile>, name: &str, where_: &str) -> Result<String> {
    if !profiles.contains_key(name) {
        return Err(fail(format!(
            "{where_}: Profil „{name}“ gibt es nicht ({}).",
            profiles.keys().cloned().collect::<Vec<String>>().join(", ")
        )));
    }
    Ok(name.to_string())
}

/// Ordner relativ zum Worktree mit „/“, ohne „./“ am Anfang und „/“ am Ende; leer = „.“;
/// absolut oder mit „..“ → Fehler
fn relative_dir(value: &str, where_: &str) -> Result<String> {
    let text = value.trim().replace('\\', "/");
    if ABSOLUTE_DIR.is_match(&text) {
        return Err(fail(format!(
            "{where_}: cwd „{value}“ ist absolut – erlaubt ist nur ein Pfad relativ zum Worktree."
        )));
    }
    let parts: Vec<&str> = text
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    if parts.contains(&"..") {
        return Err(fail(format!(
            "{where_}: cwd „{value}“ verlässt den Worktree („..“ ist nicht erlaubt)."
        )));
    }
    let joined = parts.join("/");
    Ok(if joined.is_empty() {
        ".".to_string()
    } else {
        joined
    })
}

/// Ganze Zahl ab `min` (und höchstens `max`, wenn angegeben)
fn int(value: &Value, name: &str, min: i64, max: Option<i64>) -> Result<i64> {
    let number = js_number(value);
    if let Some(max) = max {
        let ok = number
            .map(|n| is_integer(n) && n >= min as f64 && n <= max as f64)
            .unwrap_or(false);
        if !ok {
            return Err(fail(format!(
                "{name} muss eine ganze Zahl von {min} bis {max} sein."
            )));
        }
    }
    match number {
        Some(number) if is_integer(number) && number >= min as f64 => Ok(number as i64),
        _ => Err(fail(format!("{name} muss eine ganze Zahl ab {min} sein."))),
    }
}

/// Getrimmter Text oder `None`
fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
}

/// Wirft bei einem Problem
fn check(problem: Option<String>, where_: &str) -> Result<()> {
    match problem {
        Some(problem) => Err(fail(format!("{where_}: {problem}"))),
        None => Ok(()),
    }
}

/// Prüffehler der Einstellung
fn fail(message: impl Into<String>) -> Error {
    HarnessError::new("dispatch-config-invalid", message).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::config::Profile;

    /// Erwartet einen Prüffehler, dessen Meldung den Text enthält
    fn rejects(raw: Value, text: &str) {
        let error = DispatchConfigNormalizer::new()
            .normalize(&raw)
            .expect_err("kein Fehler");
        assert_eq!(error.code(), Some("dispatch-config-invalid"), "{raw}");
        assert!(
            error.to_string().contains(text),
            "„{text}“ fehlt in „{error}“"
        );
    }

    /// Die aufgelöste Einstellung aus der rohen Angabe
    fn normalize(raw: Value) -> DispatchConfig {
        DispatchConfigNormalizer::new().normalize(&raw).unwrap()
    }

    /// Die aufgelöste Beispiel-Einstellung
    fn example() -> DispatchConfig {
        let normalizer = DispatchConfigNormalizer::new();
        normalizer.normalize(&normalizer.example()).unwrap()
    }

    #[test]
    fn fullt_leere_einstellung_mit_vorgaben() {
        let cfg = normalize(json!({}));
        assert!(!cfg.enabled);
        assert_eq!(cfg.boards, None);
        assert_eq!(cfg.wip, 1);
        assert_eq!(cfg.window, None);
        assert_eq!(cfg.opt_in_label.as_deref(), Some("auto"));
        assert_eq!(cfg.opt_out_label.as_deref(), Some("manuell"));
        assert_eq!(cfg.agent_prefix, "junior-");
        assert_eq!(cfg.dispatcher_name, "dispatcher");
        assert_eq!(cfg.max_attempts, 2);
        assert_eq!(cfg.pause_minutes_on_limit, 60);
        assert_eq!(cfg.prompt_extra, "");
        assert!(cfg.escalate);
        assert!(!cfg.allow_fable);
        assert_eq!(cfg.default_profile, "standard");
        assert_eq!(cfg.claude.command, "claude");
        assert_eq!(cfg.claude.model.as_deref(), Some("sonnet"));
        assert_eq!(cfg.claude.effort, None);
        assert_eq!(cfg.claude.max_turns, 80);
        assert_eq!(cfg.claude.timeout_minutes, 60);
        assert_eq!(cfg.claude.permission_mode.as_deref(), Some("acceptEdits"));
        assert!(cfg.claude.use_subscription);
        assert_eq!(cfg.claude.extra_args, Vec::<String>::new());
        assert_eq!(
            cfg.claude.allowed_tools,
            DEFAULT_ALLOWED_TOOLS.map(String::from)
        );
        assert_eq!(
            cfg.profiles.get("standard"),
            Some(&Profile {
                model: Some("sonnet".to_string()),
                effort: None,
                max_turns: 80,
                timeout_minutes: 60,
                accounts: None,
            })
        );
        assert_eq!(cfg.profiles.len(), 1);
        assert_eq!(cfg.ladder, ["standard"]);
        assert_eq!(cfg.stale_minutes, 90);
    }

    #[test]
    fn ubernimmt_profile_regeln_leiter_und_boards() {
        let cfg = example();
        assert_eq!(
            cfg.profiles.keys().collect::<Vec<&String>>(),
            ["klein", "standard", "schwer"]
        );
        assert_eq!(
            cfg.profiles.get("schwer"),
            Some(&Profile {
                model: Some("opus".to_string()),
                effort: Some("max".to_string()),
                max_turns: 150,
                timeout_minutes: 120,
                accounts: None,
            })
        );
        assert_eq!(
            cfg.rules[0],
            Rule {
                when: RuleCondition {
                    labels: Some(vec![
                        "doku".to_string(),
                        "tippfehler".to_string(),
                        "klein".to_string()
                    ]),
                    ..RuleCondition::default()
                },
                profile: "klein".to_string(),
                accounts: None,
            }
        );
        assert_eq!(cfg.ladder, ["klein", "standard", "schwer"]);
        assert_eq!(cfg.stale_minutes, 150);
        assert!(!cfg.enabled);
        assert_eq!(
            normalize(json!({ "boards": ["3", 5] })).boards,
            Some(vec![3, 5])
        );
        assert_eq!(normalize(json!({ "boards": [] })).boards, None);
        assert_eq!(
            normalize(json!({ "escalation": ["standard"] })).ladder,
            ["standard"]
        );
    }

    #[test]
    fn nimmt_stale_minutes_nur_uber_dem_langsten_zeitlimit() {
        assert_eq!(normalize(json!({ "staleMinutes": 65 })).stale_minutes, 90);
        assert_eq!(normalize(json!({ "staleMinutes": 200 })).stale_minutes, 200);
    }

    #[test]
    fn leeres_opt_in_label_heisst_alle_tickets() {
        assert_eq!(normalize(json!({ "optInLabel": "" })).opt_in_label, None);
        assert_eq!(normalize(json!({ "optInLabel": null })).opt_in_label, None);
        assert_eq!(
            normalize(json!({ "optOutLabel": null })).opt_out_label,
            None
        );
    }

    #[test]
    fn ubergeht_die_fruhere_einstellung_merge_ignore() {
        // Unbekannte Schlüssel der Datei werden einfach übergangen
        let cfg = normalize(json!({ "mergeIgnore": ["*.yaml"] }));
        assert_eq!(cfg.merge_strategy, MergeStrategy::Merge);
    }

    #[test]
    fn merge_strategie_vorgabe_merge_squash_wird_ubernommen() {
        assert_eq!(normalize(json!({})).merge_strategy, MergeStrategy::Merge);
        assert_eq!(
            normalize(json!({ "mergeStrategy": null })).merge_strategy,
            MergeStrategy::Merge
        );
        assert_eq!(
            normalize(json!({ "mergeStrategy": "squash" })).merge_strategy,
            MergeStrategy::Squash
        );
        assert_eq!(
            normalize(json!({ "mergeStrategy": " squash " })).merge_strategy,
            MergeStrategy::Squash
        );
        assert_eq!(
            DispatchConfigNormalizer::new().example()["mergeStrategy"],
            json!("merge")
        );
        rejects(json!({ "mergeStrategy": "rebase" }), "mergeStrategy");
        rejects(json!({ "mergeStrategy": 1 }), "mergeStrategy");
    }

    #[test]
    fn freigabe_vor_dem_merge_vorgabe_true() {
        assert!(normalize(json!({})).review_before_merge);
        assert!(normalize(json!({ "reviewBeforeMerge": null })).review_before_merge);
        assert!(normalize(json!({ "reviewBeforeMerge": true })).review_before_merge);
        assert!(!normalize(json!({ "reviewBeforeMerge": false })).review_before_merge);
        assert_eq!(
            DispatchConfigNormalizer::new().example()["reviewBeforeMerge"],
            json!(true)
        );
        rejects(
            json!({ "reviewBeforeMerge": "ja" }),
            "reviewBeforeMerge muss true oder false sein",
        );
    }

    #[test]
    fn einrichtung_neuer_worktrees_cwd_relativ_und_innerhalb() {
        let leer = normalize(json!({}));
        assert_eq!(leer.worktree_setup, Vec::new());
        assert_eq!(leer.worktree_setup_timeout_minutes, 15);
        let beispiel = example();
        assert_eq!(beispiel.worktree_setup, Vec::new());
        assert_eq!(beispiel.worktree_setup_timeout_minutes, 15);

        let cfg = normalize(json!({
            "worktreeSetup": [
                { "cwd": " BackofficeNextGen\\ ", "run": " npm ci --prefer-offline --no-audit " },
                { "run": "npm run build" },
                { "cwd": "./a/./b/", "run": "x" },
            ],
            "worktreeSetupTimeoutMinutes": 5,
        }));
        assert_eq!(
            cfg.worktree_setup,
            [
                WorktreeSetupStep {
                    cwd: "BackofficeNextGen".to_string(),
                    run: "npm ci --prefer-offline --no-audit".to_string()
                },
                WorktreeSetupStep {
                    cwd: ".".to_string(),
                    run: "npm run build".to_string()
                },
                WorktreeSetupStep {
                    cwd: "a/b".to_string(),
                    run: "x".to_string()
                },
            ]
        );
        assert_eq!(cfg.worktree_setup_timeout_minutes, 5);

        rejects(
            json!({ "worktreeSetup": [{ "cwd": "../Haupt", "run": "npm ci" }] }),
            "worktreeSetup[0]",
        );
        rejects(
            json!({ "worktreeSetup": [{ "cwd": "a/../../b", "run": "npm ci" }] }),
            "verlässt",
        );
        rejects(
            json!({ "worktreeSetup": [{ "cwd": "C:\\Projekte\\x", "run": "npm ci" }] }),
            "absolut",
        );
        rejects(
            json!({ "worktreeSetup": [{ "cwd": "/tmp", "run": "npm ci" }] }),
            "absolut",
        );
        rejects(
            json!({ "worktreeSetup": [{ "cwd": "a", "run": " " }] }),
            "run",
        );
        rejects(json!({ "worktreeSetup": "npm ci" }), "worktreeSetup");
        rejects(
            json!({ "worktreeSetupTimeoutMinutes": 0 }),
            "worktreeSetupTimeoutMinutes",
        );
    }

    #[test]
    fn worktrees_bundeln_vorgabe_none() {
        assert_eq!(normalize(json!({})).join_worktrees, JoinWorktrees::None);
        assert_eq!(
            normalize(json!({ "joinWorktrees": null })).join_worktrees,
            JoinWorktrees::None
        );
        assert_eq!(
            normalize(json!({ "joinWorktrees": "sameSpec" })).join_worktrees,
            JoinWorktrees::SameSpec
        );
        assert_eq!(
            normalize(json!({ "joinWorktrees": " sameSpec " })).join_worktrees,
            JoinWorktrees::SameSpec
        );
        assert_eq!(
            DispatchConfigNormalizer::new().example()["joinWorktrees"],
            json!("none")
        );
        rejects(json!({ "joinWorktrees": "alle" }), "joinWorktrees");
        rejects(json!({ "joinWorktrees": true }), "joinWorktrees");
    }

    #[test]
    fn lehnt_ungultiges_mit_hinweis_ab() {
        rejects(
            json!({ "window": { "from": "25:00", "to": "07:00" } }),
            "window",
        );
        rejects(json!({ "wip": -1 }), "wip");
        rejects(json!({ "boards": ["x"] }), "boards");
        rejects(
            json!({ "profiles": { "a": { "model": "gpt" } } }),
            "Unbekanntes Modell",
        );
        rejects(
            json!({ "profiles": { "a": { "effort": "ultra" } } }),
            "effort",
        );
        rejects(
            json!({ "profiles": { "a": { "model": "fable" } } }),
            "Fable",
        );
        rejects(json!({ "defaultProfile": "nix" }), "Profil „nix“");
        rejects(
            json!({ "rules": [{ "when": {}, "profile": "standard" }] }),
            "Bedingung",
        );
        rejects(
            json!({ "rules": [{ "when": { "labels": ["x"] }, "profile": "nix" }] }),
            "Profil „nix“",
        );
        rejects(json!({ "escalation": ["nix"] }), "escalation");
    }

    #[test]
    fn planungen_vorgaben_profil_und_grenzen() {
        let vorgabe = PlanningSettings {
            auto: false,
            wip: 1,
            acceptance: true,
            profile: None,
        };
        assert_eq!(normalize(json!({})).planning, vorgabe);
        assert_eq!(
            normalize(json!({
                "planning": { "auto": true, "wip": 3, "acceptance": false, "profile": "standard" }
            }))
            .planning,
            PlanningSettings {
                auto: true,
                wip: 3,
                acceptance: false,
                profile: Some("standard".to_string()),
            }
        );
        assert_eq!(
            normalize(json!({ "planning": { "wip": 5 } })).planning.wip,
            5
        );
        assert_eq!(
            normalize(json!({ "planning": { "profile": " " } }))
                .planning
                .profile,
            None
        );
        assert_eq!(normalize(json!({ "planning": null })).planning, vorgabe);
        // Das frühere maxPerDay der Planung wird übergangen
        assert_eq!(
            normalize(json!({ "planning": { "maxPerDay": 0 } })).planning,
            vorgabe
        );
        assert_eq!(example().planning, vorgabe);
        rejects(
            json!({ "planning": { "profile": "nix" } }),
            "planning.profile: Profil „nix“",
        );
        rejects(
            json!({ "planning": { "wip": 0 } }),
            "planning.wip muss eine ganze Zahl von 1 bis 5",
        );
        rejects(
            json!({ "planning": { "wip": 6 } }),
            "planning.wip muss eine ganze Zahl von 1 bis 5",
        );
        rejects(
            json!({ "planning": "schwer" }),
            "planning muss ein Objekt sein (auto, wip, acceptance, profile)",
        );
    }

    #[test]
    fn recherchen_vorgaben_profil_und_grenzen() {
        assert_eq!(
            normalize(json!({})).research,
            ResearchSettings {
                auto: false,
                wip: 1,
                max_per_day: 10,
                profile: None,
            }
        );
        assert_eq!(
            normalize(json!({
                "research": { "auto": true, "wip": 2, "maxPerDay": 3, "profile": "standard" }
            }))
            .research,
            ResearchSettings {
                auto: true,
                wip: 2,
                max_per_day: 3,
                profile: Some("standard".to_string()),
            }
        );
        assert_eq!(
            normalize(json!({ "research": { "wip": 5 } })).research.wip,
            5
        );
        assert_eq!(
            normalize(json!({ "research": { "profile": "" } }))
                .research
                .profile,
            None
        );
        assert_eq!(
            DispatchConfigNormalizer::new().example()["research"],
            json!({ "auto": false, "wip": 1, "maxPerDay": 10 })
        );
        rejects(
            json!({ "research": { "profile": "nix" } }),
            "research.profile: Profil „nix“",
        );
        rejects(
            json!({ "research": { "wip": 0 } }),
            "research.wip muss eine ganze Zahl von 1 bis 5",
        );
        rejects(
            json!({ "research": { "wip": 6 } }),
            "research.wip muss eine ganze Zahl von 1 bis 5",
        );
        rejects(
            json!({ "research": { "maxPerDay": 0 } }),
            "research.maxPerDay",
        );
        rejects(json!({ "research": "an" }), "research muss ein Objekt");
    }

    #[test]
    fn erlaubt_fable_nur_mit_allow_fable() {
        let normalizer = DispatchConfigNormalizer::new();
        let cfg = normalize(json!({
            "allowFable": true,
            "profiles": { "f": { "model": "claude-fable-5-1" } }
        }));
        assert_eq!(cfg.profiles["f"].model.as_deref(), Some("claude-fable-5-1"));
        assert!(normalizer
            .model_problem("best", false)
            .unwrap()
            .contains("Fable"));
        assert_eq!(normalizer.model_problem("claude-opus-5-5", false), None);
        assert_eq!(normalizer.effort_problem("xhigh"), None);
    }

    /// Knappe eigene Vorlage für work, die alle Pflichtteile erfüllt
    fn work_template() -> String {
        [
            "{{agent}} setzt {{ticketId}} („{{title}}“) um.",
            "Fertig: harness board done {{ticketId}} --by {{agent}}",
            "Blockiert: harness board set {{ticketId}} --status blocked",
            "Frage: harness board question add \"…\" --ticket {{ticketId}}",
        ]
        .join("\n")
    }

    /// Knappe eigene Vorlage für plan
    fn plan_template() -> String {
        "{{agent}} plant {{ticketId}}: plan.json schreiben, dann harness board plan-json {{ticketId}}"
            .to_string()
    }

    #[test]
    fn lasst_ohne_angabe_alle_vier_laufarten_auf_der_standardvorlage() {
        assert_eq!(normalize(json!({})).prompts, PromptTemplates::default());
        assert_eq!(normalize(json!({ "prompts": null })).prompts.work, None);
    }

    #[test]
    fn ubernimmt_eine_gultige_vorlage_wortlich() {
        let template = work_template();
        let prompts = normalize(json!({ "prompts": { "work": template } })).prompts;
        assert_eq!(prompts.work.as_deref(), Some(template.as_str()));
        assert_eq!(prompts.plan, None);
        assert_eq!(prompts.research, None);
        assert_eq!(prompts.merge, None);
    }

    #[test]
    fn behandelt_leere_vorlagen_wie_nicht_gesetzt() {
        assert_eq!(
            normalize(json!({ "prompts": { "work": "", "plan": "   ", "research": null } }))
                .prompts,
            PromptTemplates::default()
        );
    }

    #[test]
    fn vereinheitlicht_zeilenenden_auf_lf() {
        let template = plan_template().replacen(": ", ":\r\n", 1);
        let prompts = normalize(json!({ "prompts": { "plan": template } })).prompts;
        let plan = prompts.plan.unwrap();
        assert!(!plan.contains('\r'));
        assert!(plan.contains('\n'));
    }

    #[test]
    fn fallt_bei_unbekanntem_platzhalter_oder_fehlendem_pflichtteil_still_auf_den_standard() {
        let mit_unbekanntem = format!("{}\n{{{{unbekannt}}}}", work_template());
        assert_eq!(
            normalize(json!({ "prompts": { "work": mit_unbekanntem } }))
                .prompts
                .work,
            None
        );
        assert_eq!(
            normalize(json!({ "prompts": { "work": "nur {{agent}} und {{ticketId}}" } }))
                .prompts
                .work,
            None
        );
    }

    #[test]
    fn nennt_die_befunde_uber_prompt_problems() {
        let normalizer = DispatchConfigNormalizer::new();
        let mit_unbekanntem = format!("{}\n{{{{unbekannt}}}}", work_template());
        let problems = normalizer
            .prompt_problems(&json!({
                "prompts": { "work": mit_unbekanntem, "plan": "{{agent}} ohne alles" }
            }))
            .unwrap();
        assert_eq!(
            problems
                .iter()
                .map(|problem| problem.kind.as_str())
                .collect::<Vec<&str>>(),
            ["work", "plan"]
        );
        assert_eq!(problems[0].unknown_placeholders, ["unbekannt"]);
        assert_eq!(problems[0].missing_parts, Vec::<String>::new());
        assert!(problems[1].missing_parts.join(" ").contains("plan.json"));
        assert_eq!(
            normalizer
                .prompt_problems(
                    &json!({ "prompts": { "work": work_template(), "plan": plan_template() } })
                )
                .unwrap(),
            vec![]
        );
    }

    #[test]
    fn lehnt_eine_falsche_struktur_hart_ab() {
        rejects(json!({ "prompts": "text" }), "prompts muss ein Objekt sein");
        rejects(
            json!({ "prompts": { "arbeit": "x" } }),
            "„arbeit“ ist keine Laufart",
        );
        let error = DispatchConfigNormalizer::new()
            .prompt_problems(&json!({ "prompts": "text" }))
            .unwrap_err();
        assert_eq!(error.code(), Some("dispatch-config-invalid"));
    }
}
