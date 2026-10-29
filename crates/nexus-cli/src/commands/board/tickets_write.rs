//! `nexus board create|set|move|delete`: Tickets schreiben.
//! Portiert aus `packages/cli/src/commands/board/BoardTicketWriteCommands.ts`.

use clap::{ArgAction, Args};
use nexus_core::board::basics::Priority;
use nexus_core::board::ticket_service::{MoveOptions, NewTicketInput, TicketPatch, WriteOptions};
use nexus_core::board::TicketStep;
use nexus_core::text::TextSource;
use nexus_core::{HarnessError, Result};

use crate::board::{BoardOptionParser, BoardSelection};
use crate::context::CliContext;

/// Hilfe der Option `--target-branch`
pub const TARGET_BRANCH_HELP: &str = "Zielbranch, in den der Dispatcher mergt (fehlt er, wird er angelegt; ohne: ausgecheckter Branch des Hauptordners)";

/// Hilfe der Option `--target-branch` von `set`
const SET_TARGET_BRANCH_HELP: &str = "Zielbranch, in den der Dispatcher mergt (fehlt er, wird er angelegt; ohne: ausgecheckter Branch des Hauptordners); „none“ entfernt ihn (nur vor dem Start)";

/// Beschreibung von `create`
pub const CREATE_ABOUT: &str = "Legt ein Ticket auf dem Board dieses Repos an, sonst auf dem aktiven Board (Standard: Backlog, normal, braucht Planung).";

/// Beispiele unter der Hilfe von `create`
pub const CREATE_EXAMPLES: &str = "\nBeispiele:\n  harness board create \"Login bauen\" --desc \"Kontext\" --priority high --label backend --by claude\n  harness board create \"Export bauen\" --step \"Test schreiben\" --step \"Umsetzen\"";

/// Beschreibung von `set`
pub const SET_ABOUT: &str = "Ändert Felder eines Tickets. Texte nur ins Feld – Status, Planungsbedarf und Zuweisung bleiben, wenn nicht angegeben.";

/// Beispiele unter der Hilfe von `set`
pub const SET_EXAMPLES: &str = "\nBeispiele:\n  harness board set ARC-0007 --priority high --by claude\n  harness board set ARC-0007 --plan-file plan.md\n  harness board set ARC-0007 --status blockiert --blocked-reason \"Zugang fehlt\"\n  harness board set ARC-0007 --clear-branch";

/// Beschreibung von `move`
pub const MOVE_ABOUT: &str = "Verschiebt in eine Spalte. Erledigt braucht eine Abnahme; In Arbeit/Erledigt keine offene Voraussetzung.";

/// Beispiele unter der Hilfe von `move`
pub const MOVE_EXAMPLES: &str = "\nBeispiele:\n  harness board move ARC-0007 erledigt --acceptance \"Test deckt den Fall ab\" --by claude\n  harness board move ARC-0007 geplant";

/// Beschreibung von `delete`
pub const DELETE_ABOUT: &str =
    "Löscht ein Ticket endgültig (wird protokolliert). Nur auf ausdrückliche Anweisung.";

/// Beispiel unter der Hilfe von `delete`
pub const DELETE_EXAMPLE: &str = "\nBeispiel:\n  harness board delete ARC-0007";

/// Optionen von `board create`
#[derive(Debug, Args)]
pub struct CreateArgs {
    pub titel: String,
    #[arg(long, value_name = "text", help = "Beschreibung")]
    pub desc: Option<String>,
    #[arg(
        long = "desc-file",
        value_name = "datei",
        help = "Beschreibung aus Datei („-“ = Standardeingabe)"
    )]
    pub desc_file: Option<String>,
    #[arg(long, value_name = "status", help = "Startspalte")]
    pub status: Option<String>,
    #[arg(long, value_name = "prio", help = "low, normal, high oder urgent")]
    pub priority: Option<String>,
    #[arg(long, value_name = "label", action = ArgAction::Append, help = "Label (wiederholbar oder kommagetrennt)")]
    pub label: Vec<String>,
    #[arg(long, help = "braucht keine Planung")]
    pub planned: bool,
    #[arg(
        long,
        value_name = "text",
        help = "Abnahme: woran man sieht, dass es fertig ist"
    )]
    pub acceptance: Option<String>,
    #[arg(
        long = "acceptance-file",
        value_name = "datei",
        help = "Abnahme aus Datei"
    )]
    pub acceptance_file: Option<String>,
    #[arg(
        long = "depends-on",
        value_name = "ids",
        help = "Voraussetzungen, kommagetrennt"
    )]
    pub depends_on: Option<String>,
    #[arg(long, value_name = "text", action = ArgAction::Append, help = "Arbeitsschritt (wiederholbar, in dieser Reihenfolge; Kommas bleiben erhalten)")]
    pub step: Vec<String>,
    #[arg(long = "target-branch", value_name = "name", help = TARGET_BRANCH_HELP)]
    pub target_branch: Option<String>,
    #[arg(
        long,
        value_name = "id",
        help = "Board (Standard: Board dieses Repos, sonst aktives Board)"
    )]
    pub board: Option<String>,
    #[arg(
        long,
        help = "trotz offenem Ticket mit gleichem Titel anlegen; Sperren für Voraussetzungen und „nicht mehr nötig“ übergehen"
    )]
    pub force: bool,
    #[arg(long, value_name = "name", help = "wer anlegt (z. B. claude)")]
    pub by: Option<String>,
}

/// Optionen von `board set`
#[derive(Debug, Args)]
pub struct SetArgs {
    pub id: String,
    #[arg(long, value_name = "text", help = "Titel")]
    pub title: Option<String>,
    #[arg(long, value_name = "text", help = "Beschreibung")]
    pub desc: Option<String>,
    #[arg(
        long = "desc-file",
        value_name = "datei",
        help = "Beschreibung aus Datei"
    )]
    pub desc_file: Option<String>,
    #[arg(long, value_name = "text", help = "Plan")]
    pub plan: Option<String>,
    #[arg(long = "plan-file", value_name = "datei", help = "Plan aus Datei")]
    pub plan_file: Option<String>,
    #[arg(long, value_name = "text", help = "Handbuch")]
    pub handbook: Option<String>,
    #[arg(
        long = "handbook-file",
        value_name = "datei",
        help = "Handbuch aus Datei"
    )]
    pub handbook_file: Option<String>,
    #[arg(long, value_name = "text", help = "Mermaid-Diagramm")]
    pub diagram: Option<String>,
    #[arg(
        long = "diagram-file",
        value_name = "datei",
        help = "Diagramm aus Datei"
    )]
    pub diagram_file: Option<String>,
    #[arg(
        long,
        value_name = "status",
        help = "Status (gleiche Sperren wie move)"
    )]
    pub status: Option<String>,
    #[arg(long, value_name = "prio", help = "low, normal, high oder urgent")]
    pub priority: Option<String>,
    #[arg(long, value_name = "name", help = "Zuweisung; „none“ entfernt sie")]
    pub assignee: Option<String>,
    #[arg(long, value_name = "label", action = ArgAction::Append, help = "ersetzt die Labels (wiederholbar)")]
    pub label: Vec<String>,
    #[arg(
        long = "blocked-reason",
        value_name = "text",
        help = "Grund für Blockiert"
    )]
    pub blocked_reason: Option<String>,
    #[arg(long, value_name = "text", help = "Abnahme")]
    pub acceptance: Option<String>,
    #[arg(
        long = "acceptance-file",
        value_name = "datei",
        help = "Abnahme aus Datei"
    )]
    pub acceptance_file: Option<String>,
    #[arg(
        long = "needs-planning",
        value_name = "true|false",
        help = "Planungsbedarf"
    )]
    pub needs_planning: Option<String>,
    #[arg(
        long = "depends-on",
        value_name = "ids",
        help = "Voraussetzungen ersetzen; „none“ leert"
    )]
    pub depends_on: Option<String>,
    #[arg(long = "depends-on-add", value_name = "id", action = ArgAction::Append, help = "Voraussetzung ergänzen (wiederholbar)")]
    pub depends_on_add: Vec<String>,
    #[arg(long = "depends-on-rm", value_name = "id", action = ArgAction::Append, help = "Voraussetzung entfernen (wiederholbar)")]
    pub depends_on_rm: Vec<String>,
    #[arg(long = "target-branch", value_name = "name", help = SET_TARGET_BRANCH_HELP)]
    pub target_branch: Option<String>,
    #[arg(
        long = "to-board",
        value_name = "id",
        help = "auf ein anderes Board desselben Projekts verschieben"
    )]
    pub to_board: Option<String>,
    #[arg(
        long = "clear-branch",
        help = "Branch und Worktree am Ticket leeren (z. B. wenn sie zu einem anderen oder aufgeräumten Worktree gehören); Branch und Ordner selbst bleiben"
    )]
    pub clear_branch: bool,
    #[arg(
        long,
        value_name = "n",
        help = "nur schreiben, wenn das Ticket noch diese Version hat"
    )]
    pub rev: Option<String>,
    #[arg(
        long,
        help = "Sperren für Voraussetzungen und „nicht mehr nötig“ übergehen"
    )]
    pub force: bool,
    #[arg(long, value_name = "name", help = "wer ändert")]
    pub by: Option<String>,
}

/// Optionen von `board move`
#[derive(Debug, Args)]
pub struct MoveArgs {
    pub id: String,
    pub status: String,
    #[arg(long, value_name = "text", help = "Abnahme im selben Schritt setzen")]
    pub acceptance: Option<String>,
    #[arg(long, value_name = "n", help = "erwartete Version")]
    pub rev: Option<String>,
    #[arg(
        long,
        help = "Sperren für Voraussetzungen und „nicht mehr nötig“ übergehen"
    )]
    pub force: bool,
    #[arg(long, value_name = "name", help = "wer verschiebt")]
    pub by: Option<String>,
}

/// Optionen von `board delete`
#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub id: String,
    #[arg(long, value_name = "name", help = "wer löscht")]
    pub by: Option<String>,
}

/// Führt `board create` aus
pub fn create(context: &CliContext, args: &CreateArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let board_id = BoardSelection
            .one(
                &rt.services,
                &rt.config,
                args.board.as_deref(),
                Some(context.project_dir()),
            )
            .await?;
        let input = NewTicketInput {
            title: args.titel.clone(),
            description: TextSource::read(args.desc.as_deref(), args.desc_file.as_deref())?,
            status: match &args.status {
                Some(status) => Some(BoardOptionParser.status(status)?),
                None => None,
            },
            priority: parse_priority(args.priority.as_deref())?,
            labels: Some(args.label.clone()),
            needs_planning: if args.planned { Some(false) } else { None },
            acceptance: TextSource::read(
                args.acceptance.as_deref(),
                args.acceptance_file.as_deref(),
            )?,
            depends_on: args
                .depends_on
                .as_deref()
                .map(|value| BoardOptionParser.depends_on(value)),
            steps: if args.step.is_empty() {
                None
            } else {
                Some(
                    args.step
                        .iter()
                        .map(|text| TicketStep {
                            text: text.clone(),
                            done: false,
                            detail: None,
                        })
                        .collect(),
                )
            },
            target_branch: args.target_branch.clone(),
            ..NewTicketInput::default()
        };
        let created = rt
            .services
            .tickets
            .create(
                board_id,
                &input,
                &rt.actor(args.by.as_deref()),
                &WriteOptions {
                    force: args.force,
                    allow_duplicate: args.force,
                    ..WriteOptions::default()
                },
            )
            .await?;
        context.printer().result(
            &created,
            &[format!(
                "{} angelegt: {}",
                created.ticket.id, created.ticket.title
            )],
        );
        Ok(0)
    })
}

/// Führt `board set` aus
pub fn set(context: &CliContext, args: &SetArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let patch = patch(args)?;
        let updated = rt
            .services
            .tickets
            .update(
                &args.id,
                &patch,
                &rt.actor(args.by.as_deref()),
                &WriteOptions {
                    rev: BoardOptionParser.rev(args.rev.as_deref())?,
                    force: args.force,
                    ..WriteOptions::default()
                },
            )
            .await?;
        context.printer().result(
            &updated,
            &[format!(
                "{} geändert (rev {})",
                updated.ticket.id, updated.ticket.rev
            )],
        );
        Ok(0)
    })
}

/// Führt `board move` aus
pub fn move_to(context: &CliContext, args: &MoveArgs) -> Result<u8> {
    let status = BoardOptionParser.status(&args.status)?;
    let rev = BoardOptionParser.rev(args.rev.as_deref())?;
    context.with_board(async |rt| {
        let moved = rt
            .services
            .tickets
            .move_to(
                &args.id,
                status,
                &rt.actor(args.by.as_deref()),
                &MoveOptions {
                    rev,
                    force: args.force,
                    by_user: false,
                    acceptance: args.acceptance.clone(),
                },
            )
            .await?;
        context.printer().result(
            &moved,
            &[format!("{} → {}", moved.ticket.id, moved.ticket.status)],
        );
        Ok(0)
    })
}

/// Führt `board delete` aus
pub fn delete(context: &CliContext, args: &DeleteArgs) -> Result<u8> {
    context.with_board(async |rt| {
        rt.services
            .tickets
            .delete(&args.id, &rt.actor(args.by.as_deref()))
            .await?;
        context.printer().result(
            &serde_json::json!({ "deleted": args.id }),
            &[format!("{} gelöscht.", args.id)],
        );
        Ok(0)
    })
}

/// Baut die Änderung aus den Optionen
fn patch(o: &SetArgs) -> Result<TicketPatch> {
    if o.depends_on.is_some() && !(o.depends_on_add.is_empty() && o.depends_on_rm.is_empty()) {
        return Err(HarnessError::new(
            "usage",
            "--depends-on lässt sich nicht mit --depends-on-add/--depends-on-rm kombinieren.",
        )
        .into());
    }
    Ok(TicketPatch {
        title: o.title.clone(),
        description: TextSource::read(o.desc.as_deref(), o.desc_file.as_deref())?,
        plan: TextSource::read(o.plan.as_deref(), o.plan_file.as_deref())?,
        handbook: TextSource::read(o.handbook.as_deref(), o.handbook_file.as_deref())?,
        diagram: TextSource::read(o.diagram.as_deref(), o.diagram_file.as_deref())?,
        status: match &o.status {
            Some(status) => Some(BoardOptionParser.status(status)?),
            None => None,
        },
        priority: parse_priority(o.priority.as_deref())?,
        assignee: o.assignee.as_deref().map(none_or),
        labels: if o.label.is_empty() {
            None
        } else {
            Some(o.label.clone())
        },
        blocked_reason: o.blocked_reason.clone().map(Some),
        acceptance: TextSource::read(o.acceptance.as_deref(), o.acceptance_file.as_deref())?,
        needs_planning: o
            .needs_planning
            .as_deref()
            .map(|value| BoardOptionParser.bool(value)),
        depends_on: o
            .depends_on
            .as_deref()
            .map(|value| BoardOptionParser.depends_on(value)),
        depends_on_add: if o.depends_on_add.is_empty() {
            None
        } else {
            Some(o.depends_on_add.clone())
        },
        depends_on_remove: if o.depends_on_rm.is_empty() {
            None
        } else {
            Some(o.depends_on_rm.clone())
        },
        target_branch: o.target_branch.as_deref().map(none_or),
        board_id: match &o.to_board {
            Some(raw) => Some(BoardSelection.parse_id(raw)?),
            None => None,
        },
        // --clear-branch leert beides; ohne die Option bleiben sie unangetastet
        branch: o.clear_branch.then_some(None),
        worktree: o.clear_branch.then_some(None),
        ..TicketPatch::default()
    })
}

/// „none“ leert das Feld, jeder andere Text setzt es
fn none_or(value: &str) -> Option<String> {
    if value.trim().to_lowercase() == "none" {
        None
    } else {
        Some(value.to_string())
    }
}

/// Priorität aus dem Text der Option; wie in TypeScript gibt der Dienst die Meldung
fn parse_priority(value: Option<&str>) -> Result<Option<Priority>> {
    match value {
        Some(value) => Ok(Some(value.parse()?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Optionen von `set`, die nur die ID nennen
    fn set_args(id: &str) -> SetArgs {
        SetArgs {
            id: id.to_string(),
            title: None,
            desc: None,
            desc_file: None,
            plan: None,
            plan_file: None,
            handbook: None,
            handbook_file: None,
            diagram: None,
            diagram_file: None,
            status: None,
            priority: None,
            assignee: None,
            label: Vec::new(),
            blocked_reason: None,
            acceptance: None,
            acceptance_file: None,
            needs_planning: None,
            depends_on: None,
            depends_on_add: Vec::new(),
            depends_on_rm: Vec::new(),
            target_branch: None,
            to_board: None,
            clear_branch: false,
            rev: None,
            force: false,
            by: None,
        }
    }

    #[test]
    fn ohne_optionen_bleibt_die_anderung_leer() {
        assert!(patch(&set_args("ARC-0001")).unwrap().is_empty());
    }

    #[test]
    fn depends_on_lasst_sich_nicht_mit_add_oder_rm_verbinden() {
        let args = SetArgs {
            depends_on: Some("ARC-0001".into()),
            depends_on_add: vec!["ARC-0002".into()],
            ..set_args("ARC-0003")
        };
        let error = patch(&args).unwrap_err();
        assert_eq!(error.code(), Some("usage"));
        assert_eq!(
            error.to_string(),
            "--depends-on lässt sich nicht mit --depends-on-add/--depends-on-rm kombinieren."
        );

        let args = SetArgs {
            depends_on: Some("none".into()),
            depends_on_rm: vec!["ARC-0002".into()],
            ..set_args("ARC-0003")
        };
        assert_eq!(patch(&args).unwrap_err().code(), Some("usage"));
    }

    #[test]
    fn none_leert_zuweisung_und_zielbranch() {
        let args = SetArgs {
            assignee: Some(" None ".into()),
            target_branch: Some("none".into()),
            ..set_args("ARC-0001")
        };
        let patch = patch(&args).unwrap();
        assert_eq!(patch.assignee, Some(None));
        assert_eq!(patch.target_branch, Some(None));
    }

    #[test]
    fn ein_name_setzt_zuweisung_und_zielbranch() {
        let args = SetArgs {
            assignee: Some("claude".into()),
            target_branch: Some("release/1".into()),
            ..set_args("ARC-0001")
        };
        let patch = patch(&args).unwrap();
        assert_eq!(patch.assignee, Some(Some("claude".into())));
        assert_eq!(patch.target_branch, Some(Some("release/1".into())));
    }

    #[test]
    fn clear_branch_leert_branch_und_worktree() {
        let args = SetArgs {
            clear_branch: true,
            ..set_args("ARC-0001")
        };
        let patch = patch(&args).unwrap();
        assert_eq!(patch.branch, Some(None));
        assert_eq!(patch.worktree, Some(None));

        let ohne = patch_of(&set_args("ARC-0001"));
        assert_eq!(ohne.branch, None);
        assert_eq!(ohne.worktree, None);
    }

    #[test]
    fn eine_unbekannte_prioritat_wird_abgelehnt() {
        let args = SetArgs {
            priority: Some("quatsch".into()),
            ..set_args("ARC-0001")
        };
        assert_eq!(patch(&args).unwrap_err().code(), Some("priority-invalid"));
    }

    /// Kurz für eine Änderung, die gelingen muss
    fn patch_of(args: &SetArgs) -> TicketPatch {
        patch(args).expect("die Änderung ist gültig")
    }
}
