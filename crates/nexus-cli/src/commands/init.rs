//! `nexus init`: richtet das Projekt einmalig ein (Token, Zähler, Plugin, Rules, Freigabe)
//! und nennt das Board des Repos (wiederholbar). Portiert aus
//! `packages/cli/src/commands/InitCommand.ts`.

use nexus_core::install::{
    ApprovalResult, ApprovalStatus, ClaudeImportApproval, ProjectPluginSwitch, ProjectRules,
    RulesResult, SwitchResult, LEGACY_PLUGIN_ID,
};
use nexus_core::{HarnessProject, Result};
use serde::Serialize;
use std::path::Path;

use crate::board_lookup::{find_board, BoardName};
use crate::context::CliContext;

/// Ergebnis von `init` als JSON; Schlüssel wie in TypeScript
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InitResult {
    dir: String,
    plugin_enabled: bool,
    docs_rule: String,
    board_rule: String,
    import_approval: ApprovalResult,
    board: Option<BoardName>,
    board_hint: Option<String>,
    /// Kennung des alten Plugins, wenn das Projekt davon umgestellt wurde
    #[serde(skip_serializing_if = "Option::is_none")]
    migrated_from: Option<String>,
    /// Alte Rules, die abgelöst wurden
    #[serde(skip_serializing_if = "Vec::is_empty")]
    removed_rules: Vec<String>,
    /// Alte Rules, die von Hand geändert wurden und deshalb liegen bleiben
    #[serde(skip_serializing_if = "Vec::is_empty")]
    kept_rules: Vec<String>,
}

/// Legt Ordner, Zähler und Token an, schaltet das Plugin für dieses Projekt ein, legt die Rules
/// für Doku- und Board-Regeln an (fehlende werden bei Wiederholung ergänzt), gibt deren Import in
/// Claude Code frei und nennt Nexus-Ordner und Board
pub fn run(context: &CliContext) -> Result<u8> {
    let project = HarnessProject::init(context.project_dir())?;
    let root = Path::new(&project.root);
    let switched = ProjectPluginSwitch::new().enable(root)?;
    let rules = ProjectRules::new(context.home());
    let ensured = rules.ensure(root)?;
    let written = &ensured.written;
    let approval = ClaudeImportApproval::from_process_env().approve(&project.root)?;
    let info = find_board(&context.home(), &project.root);

    // Unter Windows hängt der Ordner mit `\` an einen Pfad mit `/`; einheitlich ausgeben
    let mut lines = vec![format!(
        "Nexus eingerichtet: {}",
        project.layout.dir().to_string_lossy().replace('\\', "/")
    )];
    if switched.enabled {
        lines.push("Plugin für dieses Projekt eingeschaltet. Claude Code neu starten oder /reload-plugins eingeben, dann laufen die Hooks.".to_string());
    }
    lines.extend(migration_lines(&switched, &ensured));
    if written.iter().any(|path| path == rules.docs.path()) {
        lines.push(format!(
            "Doku-Regeln eingebunden: {} (nicht versioniert) importiert {}.",
            rules.docs.path(),
            rules.docs.import_target()
        ));
    }
    if written.iter().any(|path| path == rules.board.path()) {
        lines.push(format!(
            "Board-Regel eingebunden: {} (nicht versioniert, immer geladen) importiert {}.",
            rules.board.path(),
            rules.board.import_target()
        ));
    }
    lines.extend(approval_lines(&approval));
    match &info.board {
        Some(board) => lines.push(format!("Board dieses Repos: {} {}", board.id, board.name)),
        None => lines.push(info.hint.clone().unwrap_or_default()),
    }

    let result = InitResult {
        dir: project.layout.dir().to_string_lossy().into_owned(),
        plugin_enabled: switched.enabled,
        docs_rule: rules.docs.file(root).to_string_lossy().into_owned(),
        board_rule: rules.board.file(root).to_string_lossy().into_owned(),
        import_approval: approval,
        board: info.board,
        board_hint: info.hint,
        migrated_from: switched.migrated.then(|| LEGACY_PLUGIN_ID.to_string()),
        removed_rules: ensured.removed.clone(),
        kept_rules: ensured.kept.clone(),
    };
    let lines: Vec<String> = lines.into_iter().filter(|line| !line.is_empty()).collect();
    context.printer().result(&result, &lines);
    Ok(0)
}

/// Ausgabe zur Umstellung eines Projekts, das noch am alten Plugin hing: eine Zeile für den
/// Plugin-Eintrag, eine für die abgelösten Rules und eine für die, die liegen bleiben
fn migration_lines(switched: &SwitchResult, rules: &RulesResult) -> Vec<String> {
    let mut lines = Vec::new();
    if switched.migrated {
        lines.push(format!(
            "Vom alten Plugin {LEGACY_PLUGIN_ID} umgestellt: der alte Eintrag ist aus den Einstellungen dieses Projekts entfernt."
        ));
    }
    if !rules.removed.is_empty() {
        lines.push(format!(
            "Alte Rules abgelöst: {}.",
            rules.removed.join(", ")
        ));
    }
    if !rules.kept.is_empty() {
        lines.push(format!(
            "Von Hand geänderte alte Rules bleiben liegen: {}. Bitte selbst prüfen und löschen.",
            rules.kept.join(", ")
        ));
    }
    lines
}

/// Ausgabe zur Freigabe des Imports; bei unveränderter Freigabe nichts
fn approval_lines(approval: &ApprovalResult) -> Vec<String> {
    match approval.status {
        ApprovalStatus::Approved => vec![format!(
            "Import der Doku- und Board-Regeln in Claude Code freigegeben: {}",
            approval.file
        )],
        ApprovalStatus::NoConfig => vec![format!(
            "Claude Code lief auf diesem Rechner noch nie ({} fehlt). Nach dem ersten Start bitte `nexus init` wiederholen, sonst laden die Doku- und Board-Regeln nicht.",
            approval.file
        )],
        ApprovalStatus::Unchanged => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_freigabe_wird_nur_bei_anderung_gemeldet() {
        let result = |status| ApprovalResult {
            status,
            file: "/h/.claude.json".to_string(),
            keys: Vec::new(),
        };
        assert_eq!(
            approval_lines(&result(ApprovalStatus::Approved)),
            ["Import der Doku- und Board-Regeln in Claude Code freigegeben: /h/.claude.json"]
        );
        assert!(approval_lines(&result(ApprovalStatus::Unchanged)).is_empty());
        let no_config = approval_lines(&result(ApprovalStatus::NoConfig));
        assert!(
            no_config[0].starts_with(
                "Claude Code lief auf diesem Rechner noch nie (/h/.claude.json fehlt)."
            ),
            "{no_config:?}"
        );
    }

    #[test]
    fn ohne_umstellung_steht_keine_zeile_dazu() {
        let lines = migration_lines(
            &SwitchResult {
                enabled: true,
                migrated: false,
            },
            &RulesResult::default(),
        );
        assert!(lines.is_empty(), "{lines:?}");
    }

    #[test]
    fn meldet_das_alte_plugin_die_abgelosten_und_die_liegenden_rules() {
        let lines = migration_lines(
            &SwitchResult {
                enabled: true,
                migrated: true,
            },
            &RulesResult {
                written: Vec::new(),
                removed: vec![".claude/rules/ai-harness-docs.md".to_string()],
                kept: vec![".claude/rules/ai-harness-board.md".to_string()],
            },
        );
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(
            lines[0].contains("ai-harness@ai-harness-local"),
            "{lines:?}"
        );
        assert!(
            lines[1] == "Alte Rules abgelöst: .claude/rules/ai-harness-docs.md.",
            "{lines:?}"
        );
        assert!(lines[2].contains("bleiben liegen"), "{lines:?}");
    }
}
