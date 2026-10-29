//! `nexus board plan|plan-json|next-planning`: Planungslauf.
//! Portiert aus `packages/cli/src/commands/board/BoardPlanningCommands.ts`.

use clap::Args;
use nexus_core::board::planning_service::PlanInput;
use nexus_core::text::TextSource;
use nexus_core::{HarnessError, Result};

use crate::board::{BoardOptionParser, BoardSelection};
use crate::context::CliContext;

/// Beschreibung von `plan`
pub const PLAN_ABOUT: &str =
    "Planungslauf: schreibt Plan/Handbuch/Diagramm, setzt Geplant und löscht den Planungsbedarf.";

/// Beispiel unter der Hilfe von `plan`
pub const PLAN_EXAMPLE: &str = "\nBeispiel:\n  harness board plan ARC-0007 --plan-file plan.md --handbook-file hb.md --by claude";

/// Beschreibung von `plan-json`
pub const PLAN_JSON_ABOUT: &str =
    "Planungslauf aus JSON {plan, handbook, diagram, acceptance?, status?, by?} in einem Aufruf.";

/// Beispiel unter der Hilfe von `plan-json`
pub const PLAN_JSON_EXAMPLE: &str =
    "\nBeispiel:\n  harness board plan-json ARC-0007 --file payload.json --by claude";

/// Beschreibung von `next-planning`
pub const NEXT_ABOUT: &str = "Nächstes Ticket mit Planungsbedarf; mit --claim für 15 Minuten beansprucht. Überspringt In Planung.";

/// Beispiel unter der Hilfe von `next-planning`
pub const NEXT_EXAMPLE: &str = "\nBeispiel:\n  harness --json board next-planning --claim claude";

/// Die Felder, die in der Eingabe von `plan-json` Texte sein müssen
const JSON_FIELDS: [&str; 6] = ["plan", "handbook", "diagram", "acceptance", "status", "by"];

/// Optionen von `board plan`
#[derive(Debug, Args)]
pub struct PlanArgs {
    pub id: String,
    #[arg(long, value_name = "text", help = "Plan")]
    pub plan: Option<String>,
    #[arg(
        long = "plan-file",
        value_name = "datei",
        help = "Plan aus Datei („-“ = Standardeingabe)"
    )]
    pub plan_file: Option<String>,
    #[arg(long, value_name = "text", help = "Handbuch")]
    pub handbook: Option<String>,
    #[arg(
        long = "handbook-file",
        value_name = "datei",
        help = "Handbuch aus Datei"
    )]
    pub handbook_file: Option<String>,
    #[arg(long, value_name = "text", help = "Mermaid ohne Code-Fence")]
    pub diagram: Option<String>,
    #[arg(
        long = "diagram-file",
        value_name = "datei",
        help = "Diagramm aus Datei"
    )]
    pub diagram_file: Option<String>,
    #[arg(long, value_name = "status", help = "Zielstatus (Standard: planned)")]
    pub status: Option<String>,
    #[arg(long, value_name = "name", help = "wer plant (Standard: Anzeigename)")]
    pub by: Option<String>,
}

/// Optionen von `board plan-json`
#[derive(Debug, Args)]
pub struct PlanJsonArgs {
    pub id: String,
    #[arg(
        long,
        required = true,
        value_name = "datei",
        help = "JSON-Datei („-“ = Standardeingabe)"
    )]
    pub file: String,
    #[arg(long, value_name = "status", help = "Zielstatus, falls nicht im JSON")]
    pub status: Option<String>,
    #[arg(long, value_name = "name", help = "wer plant, falls nicht im JSON")]
    pub by: Option<String>,
}

/// Optionen von `board next-planning`
#[derive(Debug, Args)]
pub struct NextPlanningArgs {
    #[arg(
        long,
        value_name = "agent",
        help = "Ticket für diesen Agenten beanspruchen"
    )]
    pub claim: Option<String>,
    #[arg(
        long,
        value_name = "id|all",
        help = "Board (Standard: Board dieses Repos, sonst aktives Board)"
    )]
    pub board: Option<String>,
}

/// Führt `board plan` aus
pub fn plan(context: &CliContext, args: &PlanArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let input = PlanInput {
            plan: TextSource::read(args.plan.as_deref(), args.plan_file.as_deref())?,
            handbook: TextSource::read(args.handbook.as_deref(), args.handbook_file.as_deref())?,
            diagram: TextSource::read(args.diagram.as_deref(), args.diagram_file.as_deref())?,
            acceptance: None,
            status: match &args.status {
                Some(status) => Some(BoardOptionParser.status(status)?),
                None => None,
            },
        };
        let planned = rt
            .services
            .planning
            .plan(&args.id, &input, &rt.actor(args.by.as_deref()))
            .await?;
        context.printer().result(
            &planned,
            &[format!(
                "{} geplant ({})",
                planned.ticket.id, planned.ticket.status
            )],
        );
        Ok(0)
    })
}

/// Führt `board plan-json` aus
pub fn plan_json(context: &CliContext, args: &PlanJsonArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let input = read_json(&args.file)?;
        let status = field(&input, "status").or(args.status.clone());
        let plan = PlanInput {
            plan: field(&input, "plan"),
            handbook: field(&input, "handbook"),
            diagram: field(&input, "diagram"),
            acceptance: field(&input, "acceptance"),
            status: match &status {
                Some(status) => Some(BoardOptionParser.status(status)?),
                None => None,
            },
        };
        let by = field(&input, "by").or(args.by.clone());
        let planned = rt
            .services
            .planning
            .plan(&args.id, &plan, &rt.actor(by.as_deref()))
            .await?;
        context.printer().result(
            &planned,
            &[format!(
                "{} geplant ({})",
                planned.ticket.id, planned.ticket.status
            )],
        );
        Ok(0)
    })
}

/// Führt `board next-planning` aus
pub fn next_planning(context: &CliContext, args: &NextPlanningArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let board_ids = BoardSelection
            .many(
                &rt.services,
                &rt.config,
                args.board.as_deref(),
                Some(context.project_dir()),
            )
            .await?;
        let ticket = rt
            .services
            .planning
            .next(
                &board_ids,
                args.claim.as_deref(),
                &rt.actor(args.claim.as_deref()),
            )
            .await?;
        let line = match &ticket {
            Some(ticket) => format!("{}  {}", ticket.ticket.id, ticket.ticket.title),
            None => "Kein Ticket mit Planungsbedarf.".to_string(),
        };
        context
            .printer()
            .result(&serde_json::json!({ "ticket": ticket }), &[line]);
        Ok(0)
    })
}

/// Liest und prüft die JSON-Eingabe
fn read_json(file: &str) -> Result<serde_json::Map<String, serde_json::Value>> {
    let raw = TextSource::read(None, Some(file))?.unwrap_or_default();
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Err(HarnessError::new(
            "input-invalid",
            "Die Eingabe für plan-json ist kein gültiges JSON.",
        )
        .into());
    };
    let Some(object) = parsed.as_object() else {
        return Err(HarnessError::new(
            "input-invalid",
            "Die Eingabe für plan-json muss ein JSON-Objekt sein.",
        )
        .into());
    };
    for key in JSON_FIELDS {
        if object.get(key).is_some_and(|value| !value.is_string()) {
            return Err(HarnessError::new(
                "input-invalid",
                format!("Das Feld „{key}“ in der Eingabe für plan-json muss ein Text sein."),
            )
            .into());
        }
    }
    Ok(object.clone())
}

/// Ein Textfeld der JSON-Eingabe
fn field(input: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Schreibt die Eingabe in eine Datei und liest sie
    fn lesen(text: &str) -> Result<serde_json::Map<String, serde_json::Value>> {
        let dir = tempfile::tempdir().expect("Temp-Ordner");
        let file = dir.path().join("payload.json");
        std::fs::write(&file, text).expect("Datei");
        read_json(&file.display().to_string())
    }

    #[test]
    fn liest_die_felder_der_eingabe() {
        let input = lesen(r#"{"plan":"P","handbook":"H","by":"claude"}"#).unwrap();
        assert_eq!(field(&input, "plan").as_deref(), Some("P"));
        assert_eq!(field(&input, "by").as_deref(), Some("claude"));
        assert_eq!(field(&input, "diagram"), None);
    }

    #[test]
    fn kaputtes_json_wird_mit_input_invalid_abgelehnt() {
        let error = lesen("{kaputt").unwrap_err();
        assert_eq!(error.code(), Some("input-invalid"));
        assert_eq!(
            error.to_string(),
            "Die Eingabe für plan-json ist kein gültiges JSON."
        );
    }

    #[test]
    fn eine_liste_ist_kein_objekt() {
        assert_eq!(
            lesen("[]").unwrap_err().to_string(),
            "Die Eingabe für plan-json muss ein JSON-Objekt sein."
        );
    }

    #[test]
    fn ein_feld_mit_falschem_typ_wird_abgelehnt() {
        assert_eq!(
            lesen(r#"{"plan":7}"#).unwrap_err().to_string(),
            "Das Feld „plan“ in der Eingabe für plan-json muss ein Text sein."
        );
    }
}
