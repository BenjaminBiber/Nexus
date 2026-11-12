//! Einstellung des Dispatchers (`~/.nexus/dispatch.yaml`), vollständig aufgelöst;
//! portiert aus `packages/core/src/dispatch/DispatchConfig.ts`.
//!
//! Die Tool-Listen, Marker und Gründe sind Vertrag zwischen Dispatcher und Agent und stehen
//! deshalb wortgleich zu TypeScript hier – auch die Befehle `nexus board …` und `harness board …`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::board::DispatchProfileReason;
use crate::dispatch::markers;
use crate::dispatch::prompt_check::PromptKind;
use crate::dispatch::state::UsageSnapshot;

pub use crate::dispatch::merge_strategy::MergeStrategy;

/// Zeitfenster, in dem neue Läufe starten dürfen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct TimeWindow {
    /// HH:MM Ortszeit, einschließlich
    pub from: String,
    /// HH:MM Ortszeit, ausschließlich; liegt `to` vor `from`, geht das Fenster über Mitternacht
    pub to: String,
}

/// Aufruf und Grenzen von Claude Code
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ClaudeSettings {
    /// Aufruf von Claude Code – `claude` oder ein voller Pfad
    pub command: String,
    /// Vorgabe für Profile ohne `model`; null = Einstellung von Claude Code
    pub model: Option<String>,
    /// Vorgabe für Profile ohne `effort`; null = Standard des Modells
    pub effort: Option<String>,
    pub max_turns: i64,
    pub timeout_minutes: i64,
    pub permission_mode: Option<String>,
    /// Was der Agent ohne Rückfrage darf (`--allowedTools`)
    pub allowed_tools: Vec<String>,
    /// true = API-Key-Variablen aus der Umgebung des Workers entfernen
    pub use_subscription: bool,
    pub extra_args: Vec<String>,
    /// Pfad der bash.exe von Git for Windows; null = automatisch suchen (nur Windows)
    pub git_bash_path: Option<String>,
}

/// Modell, Effort und Grenzen eines Profils
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct Profile {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub max_turns: i64,
    pub timeout_minutes: i64,
    /// Claude-Accounts (IDs aus `accounts.json`), die das Profil nutzen darf, in dieser
    /// Reihenfolge; fehlt = jeder aktive Account
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub accounts: Option<Vec<String>>,
}

/// Bedingung einer Regel; nur genannte Felder zählen
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct RuleCondition {
    /// Das Ticket trägt eines dieser Labels
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub labels: Option<Vec<String>>,
    /// Die Priorität ist eine davon
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub priority: Option<Vec<String>>,
    /// Mindestens so viele Zeilen in der Tabelle unter `## Integration` im Plan
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub files_min: Option<i64>,
    /// Der Plan hat mindestens so viele Zeichen
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub plan_min_chars: Option<i64>,
}

impl RuleCondition {
    /// Keine einzige Bedingung genannt
    pub fn is_empty(&self) -> bool {
        self.labels.is_none()
            && self.priority.is_none()
            && self.files_min.is_none()
            && self.plan_min_chars.is_none()
    }
}

/// Eine Regel: trifft `when` zu, gilt `profile`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct Rule {
    pub when: RuleCondition,
    pub profile: String,
    /// Überschreibt die Accounts des Profils, wenn die Regel greift; fehlt = wie das Profil
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub accounts: Option<Vec<String>>,
}

/// Wie unter den erlaubten Accounts gewählt wird: meiste freie Kapazität oder erster freier
/// der Liste
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "dispatch.ts")]
pub enum AccountStrategy {
    /// Der Account mit der geringsten bekannten Auslastung
    #[default]
    MostFree,
    /// Der erste freie Account der Liste
    Order,
}

impl AccountStrategy {
    /// Erlaubte Werte in der Reihenfolge von TypeScript (`ACCOUNT_STRATEGIES`)
    pub const ALL: [AccountStrategy; 2] = [AccountStrategy::MostFree, AccountStrategy::Order];

    /// Schreibweise in JSON und in der Einstellung
    pub fn as_str(&self) -> &'static str {
        match self {
            AccountStrategy::MostFree => "most-free",
            AccountStrategy::Order => "order",
        }
    }
}

/// Erlaubte Werte von `accountStrategy`
pub const ACCOUNT_STRATEGIES: [AccountStrategy; 2] = AccountStrategy::ALL;

/// Einstellungen eines Laufs samt Begründung für den Start-Kommentar
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct RunSettings {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub max_turns: i64,
    pub timeout_minutes: i64,
    pub profile: String,
    pub why: String,
    /// Warum das Basisprofil gewählt wurde (vor einer Eskalation); fehlt bei Läufen älterer Stände
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reason: Option<DispatchProfileReason>,
    /// Hash der Dispatcher-Einstellung, die beim Start galt; fehlt bei Läufen älterer Stände
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub config_hash: Option<String>,
    /// 1-basierte Nummer der Regel, die gegriffen hat – nur bei `reason: 'rule'`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rule_index: Option<i64>,
    /// Basisprofil vor der Eskalation; fehlt ohne Eskalation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub escalated_from: Option<String>,
    /// Accounts, aus denen gewählt wird (Label `account:`, Regel, Profil);
    /// fehlt = jeder aktive Account
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub allowed_accounts: Option<Vec<String>>,
    /// Herkunft von `allowedAccounts` für die Begründung, z. B. „Profil schwer“
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub accounts_from: Option<String>,
    /// Gewählter Account des Laufs; fehlt bei Läufen älterer Stände (= `standard`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub account: Option<String>,
    /// Begründung der Account-Wahl
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub account_why: Option<String>,
    /// Auslastung des gewählten Accounts beim Start; fehlt bei Läufen älterer Stände
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub usage_at_start: Option<UsageSnapshot>,
}

/// Recherche-Läufe zu offenen Fragen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ResearchSettings {
    /// true = neue offene Fragen ohne Recherche, die den eigenen Benutzer betreffen, automatisch
    /// recherchieren lassen (je Frage einmal; nur Fragen, die nach dem Einschalten entstanden sind)
    pub auto: bool,
    /// Höchstens so viele Recherche-Läufe gleichzeitig (1–5, zusätzlich zum WIP der Tickets)
    pub wip: i64,
    /// Höchstens so viele automatische Recherchen je Kalendertag (Ortszeit)
    pub max_per_day: i64,
    /// Profil der Recherche; null = Profil „research“, falls vorhanden, sonst defaultProfile
    pub profile: Option<String>,
}

/// Planungsläufe an Tickets („Planen“ im Ticket-Panel)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct PlanningSettings {
    /// true = Tickets mit Planungsbedarf auf eigenen Boards automatisch planen lassen, solange im
    /// Dispatcher Platz ist (je Ticket einmal; nur Tickets, die nach dem Einschalten dazukamen)
    pub auto: bool,
    /// Höchstens so viele Planungsläufe gleichzeitig (1–5, zusätzlich zum WIP der Tickets)
    pub wip: i64,
    /// true = die Planung füllt auch die Abnahme des Tickets (woran man sieht, dass es fertig ist)
    pub acceptance: bool,
    /// Profil der Planung; null = Profil „planning“, falls vorhanden, sonst defaultProfile
    pub profile: Option<String>,
}

/// Autopilot des Dispatchers (`autopilot_for` entscheidet je Ticket)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct AutopilotSettings {
    /// true = keine Freigaben, Fragen entscheidet die Recherche, Merge-Konflikte löst Claude
    pub enabled: bool,
    /// Tickets mit diesem Label laufen ohne Autopilot; null = keine Ausnahme
    pub except_label: Option<String>,
    /// Höchstens so viele Merge-Sessions je Konflikt (1–5)
    pub max_merge_sessions: i64,
}

/// Erlaubte Werte von mergeStrategy
pub const MERGE_STRATEGIES: [MergeStrategy; 2] = MergeStrategy::ALL;

/// Einrichtungsbefehl für einen neu angelegten Worktree (z. B. npm ci);
/// cwd relativ zum Worktree, „.“ = Worktree selbst
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct WorktreeSetupStep {
    pub cwd: String,
    pub run: String,
}

/// Worktrees des Dispatchers: ein Worktree je Ticket oder Tickets einer Spec in einem
/// gemeinsamen Worktree (alt)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub enum JoinWorktrees {
    /// Jedes Ticket bekommt einen eigenen Worktree und wird einzeln gemergt
    #[default]
    None,
    /// Tickets einer Spec teilen sich einen Worktree
    SameSpec,
}

impl JoinWorktrees {
    /// Erlaubte Werte in der Reihenfolge von TypeScript (`JOIN_WORKTREES`)
    pub const ALL: [JoinWorktrees; 2] = [JoinWorktrees::None, JoinWorktrees::SameSpec];

    /// Schreibweise in JSON und in der Einstellung
    pub fn as_str(&self) -> &'static str {
        match self {
            JoinWorktrees::None => "none",
            JoinWorktrees::SameSpec => "sameSpec",
        }
    }
}

/// Erlaubte Werte von joinWorktrees
pub const JOIN_WORKTREES: [JoinWorktrees; 2] = JoinWorktrees::ALL;

/// Eigene Prompt-Vorlagen je Laufart; null oder leer = Standardvorlage
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct PromptTemplates {
    pub work: Option<String>,
    pub plan: Option<String>,
    pub research: Option<String>,
    pub merge: Option<String>,
}

impl PromptTemplates {
    /// Die eigene Vorlage dieser Laufart
    pub fn get(&self, kind: PromptKind) -> Option<&str> {
        match kind {
            PromptKind::Work => self.work.as_deref(),
            PromptKind::Plan => self.plan.as_deref(),
            PromptKind::Research => self.research.as_deref(),
            PromptKind::Merge => self.merge.as_deref(),
        }
    }

    /// Setzt die eigene Vorlage dieser Laufart
    pub fn set(&mut self, kind: PromptKind, template: Option<String>) {
        match kind {
            PromptKind::Work => self.work = template,
            PromptKind::Plan => self.plan = template,
            PromptKind::Research => self.research = template,
            PromptKind::Merge => self.merge = template,
        }
    }
}

/// Erlaubte Laufarten einer Vorlage, in der Reihenfolge der Oberfläche
pub const PROMPT_KINDS: [PromptKind; 4] = PromptKind::ALL;

/// Befund zu einer eigenen Vorlage: unbekannte Platzhalter und fehlende Pflichtteile
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct PromptProblem {
    pub kind: PromptKind,
    pub unknown_placeholders: Vec<String>,
    /// Beschriftungen der fehlenden Pflichtteile (`PromptTemplateCheck`)
    pub missing_parts: Vec<String>,
}

/// Die vollständig aufgelöste Einstellung des Dispatchers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct DispatchConfig {
    /// false = nichts Neues starten (Läufe werden trotzdem ausgewertet)
    pub enabled: bool,
    /// Board-IDs; null = aktives Board
    pub boards: Option<Vec<i64>>,
    pub wip: i64,
    pub window: Option<TimeWindow>,
    pub opt_in_label: Option<String>,
    pub opt_out_label: Option<String>,
    pub agent_prefix: String,
    pub dispatcher_name: String,
    pub stale_minutes: i64,
    pub max_attempts: i64,
    pub pause_minutes_on_limit: i64,
    pub prompt_extra: String,
    /// Eigene Vorlagen je Laufart; null/leer = Standardvorlage
    pub prompts: PromptTemplates,
    /// Zurückmergen der Ticket-Branches: Merge-Commit (alle Commits bleiben) oder
    /// Squash (ein Commit je Branch)
    pub merge_strategy: MergeStrategy,
    /// true = ein vom Lauf erledigtes Ticket wartet vor dem Merge auf Freigabe
    /// (Blockiert, `REVIEW_REASON`)
    pub review_before_merge: bool,
    /// none = jedes Ticket bekommt einen eigenen Worktree und wird einzeln gemergt;
    /// sameSpec = Tickets einer Spec teilen einen Worktree
    pub join_worktrees: JoinWorktrees,
    /// Befehle, die der Worker in einem neu angelegten Worktree vor Claude ausführt
    /// (nacheinander; leer = keine)
    pub worktree_setup: Vec<WorktreeSetupStep>,
    /// Zeitlimit je Einrichtungsbefehl in Minuten
    pub worktree_setup_timeout_minutes: i64,
    pub claude: ClaudeSettings,
    /// Profile in der Reihenfolge der Datei – `defaultProfile` und die Leiter hängen daran
    #[ts(type = "Record<string, Profile>")]
    pub profiles: IndexMap<String, Profile>,
    pub default_profile: String,
    pub rules: Vec<Rule>,
    pub escalate: bool,
    /// Profilleiter von günstig nach teuer
    pub ladder: Vec<String>,
    pub allow_fable: bool,
    pub research: ResearchSettings,
    pub planning: PlanningSettings,
    /// Autopilot: keine Freigaben, Fragen entscheidet die Recherche, Merge-Konflikte mehrfach
    /// per Session
    pub autopilot: AutopilotSettings,
    /// Wahl unter mehreren erlaubten Accounts
    pub account_strategy: AccountStrategy,
    /// Ab dieser Auslastung in Prozent (5 Stunden oder Woche) gilt ein Account als voll (50–100)
    pub account_limit_percent: i64,
    /// Fester Puffer in Prozentpunkten fürs 5-Stunden-Fenster, solange es zu wenige Läufe für
    /// eine Schätzung gibt (0–50)
    pub account_reserve_five_hour: i64,
    /// Fester Puffer in Prozentpunkten für die Woche (0–50)
    pub account_reserve_week: i64,
    /// Setzt sich das 5-Stunden-Fenster innerhalb dieser Minuten zurück, zählt die Reserve dort
    /// nicht (0–300)
    pub account_reset_grace_minutes: i64,
}

/// Werkzeuge, die ein Umsetzungslauf ohne Rückfrage benutzen darf
pub const DEFAULT_ALLOWED_TOOLS: [&str; 16] = [
    "Read",
    "Edit",
    "Write",
    "Glob",
    "Grep",
    // Board-CLI: `nexus` und der alte Name `harness`
    "Bash(harness:*)",
    "Bash(nexus:*)",
    // node deckt Tests und Skripte des Projekts ab
    "Bash(node:*)",
    "Bash(npm test:*)",
    "Bash(npm run:*)",
    "Bash(git status:*)",
    "Bash(git diff:*)",
    "Bash(git log:*)",
    "Bash(git branch:*)",
    "Bash(git add:*)",
    "Bash(git commit:*)",
];

/// Zusätzlich erlaubt nur in der Merge-Session
pub const MERGE_SESSION_TOOLS: [&str; 2] = ["Bash(git merge:*)", "Bash(git checkout:*)"];

/// Einzige erlaubte Tools einer Recherche: lesen, lesende Board-Befehle, das Schreiben genau einer
/// Datei research.json im Arbeitsordner (Heredocs mit JSON lehnt Claude Code bei Präfix-Regeln ab),
/// das Speichern der Recherche aus dieser Datei und lesende Git-Befehle; ersetzt
/// `claude.allowedTools`. Kein `git grep` (`-O` startet ein Programm) – dafür gibt es Grep
pub const RESEARCH_TOOLS: [&str; 16] = [
    "Read",
    "Glob",
    "Grep",
    "Write(./research.json)",
    "Edit(./research.json)",
    "Bash(harness board question show:*)",
    "Bash(nexus board question show:*)",
    "Bash(harness board question research:*)",
    "Bash(nexus board question research:*)",
    "Bash(harness board show:*)",
    "Bash(nexus board show:*)",
    "Bash(harness board list:*)",
    "Bash(nexus board list:*)",
    "Bash(git log:*)",
    "Bash(git show:*)",
    "Bash(git diff:*)",
];

/// Für eine Recherche ausdrücklich verboten (`--disallowedTools`): Verbote schlagen Erlaubnisse,
/// auch solche aus `~/.claude/settings.json` und den Einstellungen des Repos
pub const RESEARCH_DENIED_TOOLS: [&str; 22] = [
    "MultiEdit",
    "NotebookEdit",
    "Bash(git commit:*)",
    "Bash(git push:*)",
    "Bash(git checkout:*)",
    "Bash(git reset:*)",
    "Bash(git switch:*)",
    "Bash(git merge:*)",
    "Bash(git rebase:*)",
    "Bash(npm:*)",
    "Bash(node:*)",
    "Bash(harness board dispatch:*)",
    "Bash(nexus board dispatch:*)",
    "Bash(harness board question research-start:*)",
    "Bash(nexus board question research-start:*)",
    "Bash(harness board question answer:*)",
    "Bash(nexus board question answer:*)",
    "Bash(harness board question discard:*)",
    "Bash(nexus board question discard:*)",
    "Bash(git diff --output:*)",
    "Bash(git log --output:*)",
    "Bash(git show --output:*)",
];

/// Einzige erlaubte Tools einer Planung: lesen, lesende Board-Befehle, das Schreiben genau einer
/// Datei plan.json im Arbeitsordner, das Speichern des Plans aus dieser Datei, eine Frage oder
/// einen Kommentar am Ticket anlegen und lesende Git-Befehle; ersetzt `claude.allowedTools`
pub const PLANNING_TOOLS: [&str; 24] = [
    "Read",
    "Glob",
    "Grep",
    "Write(./plan.json)",
    "Edit(./plan.json)",
    "Bash(harness board show:*)",
    "Bash(nexus board show:*)",
    "Bash(harness board list:*)",
    "Bash(nexus board list:*)",
    "Bash(harness board plan-json:*)",
    "Bash(nexus board plan-json:*)",
    "Bash(harness board comment:*)",
    "Bash(nexus board comment:*)",
    "Bash(harness board note list:*)",
    "Bash(nexus board note list:*)",
    "Bash(harness board question show:*)",
    "Bash(nexus board question show:*)",
    "Bash(harness board question list:*)",
    "Bash(nexus board question list:*)",
    "Bash(harness board question add:*)",
    "Bash(nexus board question add:*)",
    "Bash(git log:*)",
    "Bash(git show:*)",
    "Bash(git diff:*)",
];

/// Nur für eine Planung zusätzlich verboten: alles, was Status, Zuständigkeit oder Felder des
/// Tickets von Hand ändert (das erledigt plan-json)
pub const PLANNING_ONLY_DENIED_TOOLS: [&str; 14] = [
    "Bash(harness board move:*)",
    "Bash(nexus board move:*)",
    "Bash(harness board set:*)",
    "Bash(nexus board set:*)",
    "Bash(harness board claim:*)",
    "Bash(nexus board claim:*)",
    "Bash(harness board start:*)",
    "Bash(nexus board start:*)",
    "Bash(harness board done:*)",
    "Bash(nexus board done:*)",
    "Bash(harness board delete:*)",
    "Bash(nexus board delete:*)",
    "Bash(harness board plan-start:*)",
    "Bash(nexus board plan-start:*)",
];

/// Für eine Planung ausdrücklich verboten: die Verbote der Recherche, dazu
/// [`PLANNING_ONLY_DENIED_TOOLS`] (in TypeScript `PLANNING_DENIED_TOOLS`)
pub fn planning_denied_tools() -> Vec<&'static str> {
    RESEARCH_DENIED_TOOLS
        .into_iter()
        .chain(PLANNING_ONLY_DENIED_TOOLS)
        .collect()
}

/// Rechte-Modus einer Recherche, unabhängig von `claude.permissionMode`: „default“ lässt ohne
/// Rückfrage nur die freigegebenen Tools zu (`claude -p` kann nicht fragen, alles andere wird
/// abgelehnt); „acceptEdits“ würde Dateiänderungen ohne Freigabe erlauben
pub const RESEARCH_PERMISSION_MODE: &str = "default";

/// Effort-Stufen von Claude Code; welche ein Modell kann, entscheidet Claude Code
pub const EFFORT_LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// Bekannte Modell-Aliase
pub const MODEL_ALIASES: [&str; 11] = [
    "default",
    "sonnet",
    "opus",
    "haiku",
    "fable",
    "best",
    "opusplan",
    "sonnet[1m]",
    "opus[1m]",
    "opusplan[1m]",
    "fable[1m]",
];

/// Blockiert-Grund eines Tickets, das nach dem Lauf auf Freigabe vor dem Merge wartet
pub const REVIEW_REASON: &str = "Human Interaction: Review vor dem Merge";

/// Vorsilbe des Blockiert-Grunds, wenn die Einrichtung eines neuen Worktrees scheiterte
pub const SETUP_REASON_PREFIX: &str = "Einrichtung fehlgeschlagen:";

/// Vorsilbe des Blockiert-Grunds, wenn der Merge scheiterte („Erneut mergen“ im Panel)
pub const MERGE_REASON_PREFIX: &str = "Merge:";

/// Vorsilbe von Blockiert-Gründen und Kommentaren des Autopiloten
pub const AUTOPILOT_PREFIX: &str = "Autopilot:";

/// Anfang des Kommentars bei einer Pause wegen Nutzungslimit
pub fn limit_pause() -> String {
    format!("{}: Nutzungslimit", markers::PAUSE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_listen_sind_wortgleich_zu_typescript() {
        assert_eq!(DEFAULT_ALLOWED_TOOLS[0], "Read");
        assert_eq!(DEFAULT_ALLOWED_TOOLS[5], "Bash(harness:*)");
        assert_eq!(DEFAULT_ALLOWED_TOOLS[6], "Bash(nexus:*)");
        assert_eq!(MERGE_SESSION_TOOLS.len(), 2);
        assert_eq!(RESEARCH_PERMISSION_MODE, "default");
        assert_eq!(EFFORT_LEVELS, ["low", "medium", "high", "xhigh", "max"]);
    }

    #[test]
    fn grunde_und_vorsilben_sind_wortgleich() {
        assert_eq!(REVIEW_REASON, "Human Interaction: Review vor dem Merge");
        assert_eq!(SETUP_REASON_PREFIX, "Einrichtung fehlgeschlagen:");
        assert_eq!(MERGE_REASON_PREFIX, "Merge:");
        assert_eq!(AUTOPILOT_PREFIX, "Autopilot:");
        assert_eq!(limit_pause(), "[dispatch] Pause: Nutzungslimit");
    }

    #[test]
    fn join_worktrees_schreibt_die_werte_wie_typescript() {
        assert_eq!(
            serde_json::to_string(&JoinWorktrees::SameSpec).unwrap(),
            "\"sameSpec\""
        );
        assert_eq!(JoinWorktrees::None.as_str(), "none");
        assert_eq!(
            serde_json::from_str::<JoinWorktrees>("\"none\"").unwrap(),
            JoinWorktrees::None
        );
        assert_eq!(JOIN_WORKTREES.len(), 2);
    }
}
