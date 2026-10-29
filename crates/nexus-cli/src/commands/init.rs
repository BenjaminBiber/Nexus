//! `nexus init`: richtet das Projekt einmalig ein (Token, Zähler, Plugin, Rules, Freigabe)
//! und nennt das Board des Repos (wiederholbar). Portiert aus
//! `packages/cli/src/commands/InitCommand.ts`.

use nexus_core::install::{
    ApprovalResult, ApprovalStatus, ClaudeImportApproval, ProjectPluginSwitch, ProjectRules,
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
}

/// Legt Ordner, Zähler und Token an, schaltet das Plugin für dieses Projekt ein, legt die Rules
/// für Doku- und Board-Regeln an (fehlende werden bei Wiederholung ergänzt), gibt deren Import in
/// Claude Code frei und nennt Nexus-Ordner und Board
pub fn run(context: &CliContext) -> Result<u8> {
    let project = HarnessProject::init(context.project_dir())?;
    let root = Path::new(&project.root);
    let switched = ProjectPluginSwitch::new().enable(root)?;
    let rules = ProjectRules::new(context.home());
    let written = rules.ensure(root)?;
    let approval = ClaudeImportApproval::from_process_env().approve(&project.root)?;
    let info = find_board(&context.home(), &project.root);

    let mut lines = vec![format!(
        "Nexus eingerichtet: {}",
        project.layout.dir().display()
    )];
    if switched {
        lines.push("Plugin für dieses Projekt eingeschaltet. Claude Code neu starten oder /reload-plugins eingeben, dann laufen die Hooks.".to_string());
    }
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
        plugin_enabled: switched,
        docs_rule: rules.docs.file(root).to_string_lossy().into_owned(),
        board_rule: rules.board.file(root).to_string_lossy().into_owned(),
        import_approval: approval,
        board: info.board,
        board_hint: info.hint,
    };
    let lines: Vec<String> = lines.into_iter().filter(|line| !line.is_empty()).collect();
    context.printer().result(&result, &lines);
    Ok(0)
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
}
