//! `nexus board question …`: Fragen auf der Board-Datenbank stellen, recherchieren, verknüpfen
//! und (nur Menschen) entscheiden.
//! Portiert aus `packages/cli/src/commands/board/BoardQuestionCommands.ts`.

use clap::{ArgAction, Args, Subcommand};
use nexus_core::board::question_service::{
    BoardQuestionListFilter, BoardQuestionWriteOptions, NewBoardQuestionInput,
};
use nexus_core::board::services::BoardServices;
use nexus_core::board::{BoardQuestionActor, QuestionIds, TicketIds};
use nexus_core::questions::{DecisionInput, QuestionState, ResearchInputReader};
use nexus_core::{HarnessError, Result};
use nexus_db::BoardRuntime;

use crate::board::BoardSelection;
use crate::context::CliContext;
use crate::output::QuestionFormatter;

/// Beschreibung der Gruppe `question`
pub const QUESTION_ABOUT: &str =
    "Fragen auf der Board-Datenbank: fragen, recherchieren, entscheiden (entscheiden nur Menschen).";

/// Hilfe der Option `--kind`
const KIND_HELP: &str = "Art des Autors: human, session oder subagent (Standard: session innerhalb einer Session, sonst human)";

/// Hilfe der Option `--author`
const AUTHOR_HELP: &str = "Name des Autors (Standard: Name aus board.yaml)";

/// Hilfe der Option `--session`
const SESSION_HELP: &str = "Kennung der Session (Standard: Umgebungsvariable HARNESS_SESSION)";

/// Hilfe der Option `--author` beim Entscheiden
const DECIDER_HELP: &str = "Name des Entscheidenden (Standard: Name aus board.yaml)";

/// Hilfe der Option `--project`
const PROJECT_HELP: &str =
    "Board-Projekt (Standard: Projekt des Boards dieses Repos, sonst des aktiven Boards)";

pub const ADD_ABOUT: &str =
    "Stellt eine Frage; betroffene Tickets warten, bis sie entschieden ist.";
pub const ADD_EXAMPLE: &str = "\nBeispiel:\n  harness board question add \"Welcher Speicher?\" --ticket ARC-0007 --occasion \"Planung ARC-0007\" --kind session --author claude";

pub const RESEARCH_ABOUT: &str =
    "Ergänzt eine Recherche mit Kontext und Optionen; frühere Recherchen bleiben erhalten.";
pub const RESEARCH_EXAMPLE: &str = "\nDatei: {\"context\": \"…\", \"options\": [{\"title\": \"…\", \"text\": \"…\", \"pros\": [\"…\"], \"cons\": [\"…\"], \"recommended\": true}]}\nBeispiel:\n  harness board question research ARC-F0001 --from recherche.json --kind session --author claude";

pub const COMMENT_ABOUT: &str =
    "Hängt einen Kommentar an eine Frage (Rückfrage, Zwischenstand, Hinweis).";
pub const COMMENT_EXAMPLE: &str = "\nBeispiel:\n  harness board question comment ARC-F0001 \"Neuer Befund: …\" --kind session --author claude";

pub const LIST_ABOUT: &str =
    "Listet Fragen eines Projekts (bereite zuerst, älteste zuerst); ohne --state nur offene.";
pub const LIST_EXAMPLES: &str = "\nBeispiele:\n  harness board question list --md\n  harness board question list --state alle --ticket ARC-0007";

pub const SHOW_ABOUT: &str =
    "Zeigt eine Frage mit Beschreibung, Tickets, Recherchen, Entscheidung, Kommentaren und Verlauf.";
pub const SHOW_EXAMPLE: &str = "\nBeispiel:\n  harness board question show ARC-F0001";

pub const ANSWER_ABOUT: &str = "Beantwortet eine Frage mit einer Option der neuesten Recherche oder eigenem Text; Begründung freiwillig. Nur Menschen.";
pub const ANSWER_EXAMPLE: &str = "\nBeispiel:\n  harness board question answer ARC-F0001 --option O2 --reason \"Passt zum Betrieb\"";

pub const DISCARD_ABOUT: &str = "Verwirft eine Frage mit Begründung. Nur Menschen.";
pub const DISCARD_EXAMPLE: &str = "\nBeispiel:\n  harness board question discard ARC-F0001 --reason \"erledigt sich mit ARC-F0002\"";

pub const REOPEN_ABOUT: &str =
    "Öffnet eine beantwortete oder verworfene Frage wieder. Nur Menschen.";
pub const REOPEN_EXAMPLE: &str =
    "\nBeispiel:\n  harness board question reopen ARC-F0001 --reason \"neue Lage\"";

pub const LINK_ABOUT: &str = "Fügt betroffene Tickets hinzu oder entfernt sie.";
pub const LINK_EXAMPLE: &str =
    "\nBeispiel:\n  harness board question link ARC-F0001 --add ARC-0008 --remove ARC-0007";

pub const DEPENDS_ABOUT: &str =
    "Fügt Abhängigkeiten (Fragen oder Tickets) hinzu oder entfernt sie; Kreise werden abgelehnt.";
pub const DEPENDS_EXAMPLE: &str =
    "\nBeispiel:\n  harness board question depends ARC-F0002 --add ARC-F0001";

/// Die Unterbefehle von `board question`. `research-start` läuft über den Server und kommt in
/// P3-12 dazu.
// Die Varianten tragen die Optionen von clap; sie dürfen unterschiedlich groß sein
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
pub enum QuestionCommands {
    #[command(name = "add", about = ADD_ABOUT, after_help = ADD_EXAMPLE, disable_help_flag = true)]
    Add(AddArgs),

    #[command(name = "research", about = RESEARCH_ABOUT, after_help = RESEARCH_EXAMPLE, disable_help_flag = true)]
    Research(ResearchArgs),

    #[command(name = "comment", about = COMMENT_ABOUT, after_help = COMMENT_EXAMPLE, disable_help_flag = true)]
    Comment(CommentArgs),

    #[command(name = "list", about = LIST_ABOUT, after_help = LIST_EXAMPLES, disable_help_flag = true)]
    List(ListArgs),

    #[command(name = "show", about = SHOW_ABOUT, after_help = SHOW_EXAMPLE, disable_help_flag = true)]
    Show(ShowArgs),

    #[command(name = "answer", about = ANSWER_ABOUT, after_help = ANSWER_EXAMPLE, disable_help_flag = true)]
    Answer(AnswerArgs),

    #[command(name = "discard", about = DISCARD_ABOUT, after_help = DISCARD_EXAMPLE, disable_help_flag = true)]
    Discard(DiscardArgs),

    #[command(name = "reopen", about = REOPEN_ABOUT, after_help = REOPEN_EXAMPLE, disable_help_flag = true)]
    Reopen(ReopenArgs),

    #[command(name = "link", about = LINK_ABOUT, after_help = LINK_EXAMPLE, disable_help_flag = true)]
    Link(LinkArgs),

    #[command(name = "depends", about = DEPENDS_ABOUT, after_help = DEPENDS_EXAMPLE, disable_help_flag = true)]
    Depends(DependsArgs),

    /// Alles, was kein angemeldeter Unterbefehl ist; wird mit `command-unknown` abgewiesen
    #[command(external_subcommand)]
    Unbekannt(Vec<String>),
}

/// Wer bei Agenten-Befehlen handelt
#[derive(Debug, Args)]
pub struct WhoArgs {
    #[arg(long, value_name = "art", help = KIND_HELP)]
    pub kind: Option<String>,
    #[arg(long, value_name = "name", help = AUTHOR_HELP)]
    pub author: Option<String>,
    #[arg(long, value_name = "kennung", help = SESSION_HELP)]
    pub session: Option<String>,
}

/// Optionen von `board question add`
#[derive(Debug, Args)]
pub struct AddArgs {
    pub titel: String,
    #[arg(
        long,
        value_name = "text",
        help = "Beschreibung (Markdown, auch Mermaid)"
    )]
    pub description: Option<String>,
    #[arg(long, value_name = "id", action = ArgAction::Append, help = "betroffenes Ticket (wiederholbar)")]
    pub ticket: Vec<String>,
    #[arg(long, value_name = "id", action = ArgAction::Append, help = "Frage oder Ticket, von dem die Klärung abhängt (wiederholbar)")]
    pub depends: Vec<String>,
    #[arg(long, value_name = "tag", action = ArgAction::Append, help = "Tag (wiederholbar)")]
    pub tag: Vec<String>,
    #[arg(long, value_name = "text", help = "Anlass, z. B. „Planung ARC-0007“")]
    pub occasion: Option<String>,
    #[arg(long, value_name = "key", help = PROJECT_HELP)]
    pub project: Option<String>,
    #[command(flatten)]
    pub who: WhoArgs,
}

/// Optionen von `board question research`
#[derive(Debug, Args)]
pub struct ResearchArgs {
    pub id: String,
    #[arg(
        long,
        value_name = "datei",
        help = "JSON-Datei mit context und options, „-“ liest die Standardeingabe"
    )]
    pub from: Option<String>,
    #[arg(
        long,
        value_name = "text",
        help = "Nur Kontext als Text (ohne Optionen)"
    )]
    pub context: Option<String>,
    #[command(flatten)]
    pub who: WhoArgs,
}

/// Optionen von `board question comment`
#[derive(Debug, Args)]
pub struct CommentArgs {
    pub id: String,
    pub text: String,
    #[command(flatten)]
    pub who: WhoArgs,
}

/// Optionen von `board question list`
#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(
        long,
        value_name = "zustand",
        help = "offen, beantwortet, verworfen oder alle"
    )]
    pub state: Option<String>,
    #[arg(
        long,
        value_name = "id",
        help = "nur Fragen, die dieses Ticket betreffen"
    )]
    pub ticket: Option<String>,
    #[arg(long, value_name = "key", help = PROJECT_HELP)]
    pub project: Option<String>,
    #[arg(long, help = "Ausgabe als Markdown")]
    pub md: bool,
}

/// Optionen von `board question show`
#[derive(Debug, Args)]
pub struct ShowArgs {
    pub id: String,
}

/// Optionen von `board question answer`
#[derive(Debug, Args)]
pub struct AnswerArgs {
    pub id: String,
    #[arg(
        long,
        value_name = "nummer",
        help = "gewählte Option der neuesten Recherche, z. B. O2"
    )]
    pub option: Option<String>,
    #[arg(long, value_name = "text", help = "eigene Antwort statt einer Option")]
    pub text: Option<String>,
    #[arg(
        long,
        value_name = "text",
        help = "Begründung der Entscheidung (freiwillig)"
    )]
    pub reason: Option<String>,
    #[arg(long, value_name = "name", help = DECIDER_HELP)]
    pub author: Option<String>,
}

/// Optionen von `board question discard`
#[derive(Debug, Args)]
pub struct DiscardArgs {
    pub id: String,
    #[arg(long, value_name = "text", help = "Begründung (Pflicht)")]
    pub reason: Option<String>,
    #[arg(long, value_name = "name", help = DECIDER_HELP)]
    pub author: Option<String>,
}

/// Optionen von `board question reopen`
#[derive(Debug, Args)]
pub struct ReopenArgs {
    pub id: String,
    #[arg(long, value_name = "text", help = "Grund (optional)")]
    pub reason: Option<String>,
    #[arg(
        long,
        value_name = "name",
        help = "Name (Standard: Name aus board.yaml)"
    )]
    pub author: Option<String>,
}

/// Optionen von `board question link`; „(default: [])“ steht wie bei commander in der Hilfe
#[derive(Debug, Args)]
pub struct LinkArgs {
    pub id: String,
    #[arg(long, value_name = "ticket", action = ArgAction::Append, help = "Ticket hinzufügen (wiederholbar) (default: [])")]
    pub add: Vec<String>,
    #[arg(long, value_name = "ticket", action = ArgAction::Append, help = "Ticket entfernen (wiederholbar) (default: [])")]
    pub remove: Vec<String>,
    #[command(flatten)]
    pub who: WhoArgs,
}

/// Optionen von `board question depends`
#[derive(Debug, Args)]
pub struct DependsArgs {
    pub id: String,
    #[arg(long, value_name = "id", action = ArgAction::Append, help = "Frage oder Ticket hinzufügen (wiederholbar) (default: [])")]
    pub add: Vec<String>,
    #[arg(long, value_name = "id", action = ArgAction::Append, help = "Abhängigkeit entfernen (wiederholbar) (default: [])")]
    pub remove: Vec<String>,
    #[command(flatten)]
    pub who: WhoArgs,
}

/// Führt einen Unterbefehl von `board question` aus
pub fn run(context: &CliContext, command: &QuestionCommands) -> Result<u8> {
    match command {
        QuestionCommands::Add(args) => add(context, args),
        QuestionCommands::Research(args) => research(context, args),
        QuestionCommands::Comment(args) => comment(context, args),
        QuestionCommands::List(args) => list(context, args),
        QuestionCommands::Show(args) => show(context, args),
        QuestionCommands::Answer(args) => answer(context, args),
        QuestionCommands::Discard(args) => discard(context, args),
        QuestionCommands::Reopen(args) => reopen(context, args),
        QuestionCommands::Link(args) => link(context, args),
        QuestionCommands::Depends(args) => depends(context, args),
        QuestionCommands::Unbekannt(words) => Err(super::unknown_question(words)),
    }
}

/// `board question add`: eine Frage stellen
fn add(context: &CliContext, args: &AddArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let project = project_key(context, rt, args.project.as_deref()).await?;
        let created = rt
            .services
            .questions
            .create(
                &project,
                &NewBoardQuestionInput {
                    title: args.titel.clone(),
                    description: args.description.clone(),
                    tags: Some(args.tag.clone()),
                    occasion: args.occasion.clone(),
                    ticket_ids: Some(args.ticket.clone()),
                    depends_on: Some(args.depends.clone()),
                },
                &actor(context, rt, &args.who)?,
            )
            .await?;
        let question = &created.view.question;
        context.printer().result(
            &created,
            &[format!(
                "Frage {} angelegt: {}",
                question.id, question.title
            )],
        );
        Ok(0)
    })
}

/// `board question research`: eine Recherche ergänzen
fn research(context: &CliContext, args: &ResearchArgs) -> Result<u8> {
    // Wie in TypeScript vor dem Öffnen der Datenbank
    let input = ResearchInputReader::read(args.from.as_deref(), args.context.as_deref())?;
    context.with_board(async |rt| {
        let updated = rt
            .services
            .questions
            .add_research(&args.id, &input, &actor(context, rt, &args.who)?)
            .await?;
        context.printer().result(
            &updated.research.last(),
            &[format!(
                "Recherche zu {} ergänzt: {}.",
                updated.view.question.id,
                last_history(&updated)
            )],
        );
        Ok(0)
    })
}

/// `board question comment`: einen Kommentar anhängen
fn comment(context: &CliContext, args: &CommentArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let updated = rt
            .services
            .questions
            .comment(&args.id, &args.text, &actor(context, rt, &args.who)?)
            .await?;
        context.printer().result(
            &updated,
            &[format!(
                "Kommentar an {} gespeichert.",
                updated.view.question.id
            )],
        );
        Ok(0)
    })
}

/// `board question list`: die Fragen eines Projekts
fn list(context: &CliContext, args: &ListArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let state = state(args.state.as_deref())?;
        let project = project_key(context, rt, args.project.as_deref()).await?;
        let result = rt
            .services
            .questions
            .list(
                &project,
                &BoardQuestionListFilter {
                    state,
                    ticket: args.ticket.clone(),
                    ..BoardQuestionListFilter::default()
                },
                None,
            )
            .await?;
        let format = QuestionFormatter;
        let lines = if args.md {
            format.list_markdown(&result.questions)
        } else {
            format.list_text(&result.questions)
        };
        context.printer().result(&result, &lines);
        Ok(0)
    })
}

/// `board question show`: eine Frage mit allem
fn show(context: &CliContext, args: &ShowArgs) -> Result<u8> {
    context.with_board(async |rt| {
        let found = rt.services.questions.get(&args.id).await?;
        context
            .printer()
            .result(&found, &QuestionFormatter.show_text(&found));
        Ok(0)
    })
}

/// `board question answer`: eine Frage beantworten (nur Menschen)
fn answer(context: &CliContext, args: &AnswerArgs) -> Result<u8> {
    // Die Sitzungsprüfung läuft vor dem Öffnen der Datenbank
    context.human_actor(Some("human"), Some(args.author.as_deref().unwrap_or("")))?;
    context.with_board(async |rt| {
        let updated = rt
            .services
            .questions
            .answer(
                &args.id,
                &DecisionInput {
                    option: args.option.clone(),
                    text: args.text.clone(),
                    reason: args.reason.clone(),
                },
                &human(rt, args.author.as_deref()),
                &BoardQuestionWriteOptions::default(),
            )
            .await?;
        context.printer().result(
            &updated,
            &[format!(
                "{} beantwortet: {}",
                updated.view.question.id,
                last_history(&updated)
            )],
        );
        Ok(0)
    })
}

/// `board question discard`: eine Frage verwerfen (nur Menschen)
fn discard(context: &CliContext, args: &DiscardArgs) -> Result<u8> {
    context.human_actor(Some("human"), Some(args.author.as_deref().unwrap_or("")))?;
    context.with_board(async |rt| {
        let updated = rt
            .services
            .questions
            .discard(
                &args.id,
                args.reason.as_deref(),
                &human(rt, args.author.as_deref()),
                &BoardQuestionWriteOptions::default(),
            )
            .await?;
        context.printer().result(
            &updated,
            &[format!("{} verworfen.", updated.view.question.id)],
        );
        Ok(0)
    })
}

/// `board question reopen`: eine entschiedene Frage wieder öffnen (nur Menschen)
fn reopen(context: &CliContext, args: &ReopenArgs) -> Result<u8> {
    context.human_actor(Some("human"), Some(args.author.as_deref().unwrap_or("")))?;
    context.with_board(async |rt| {
        let updated = rt
            .services
            .questions
            .reopen(
                &args.id,
                args.reason.as_deref(),
                &human(rt, args.author.as_deref()),
                &BoardQuestionWriteOptions::default(),
            )
            .await?;
        context.printer().result(
            &updated,
            &[format!("{} wieder geöffnet.", updated.view.question.id)],
        );
        Ok(0)
    })
}

/// `board question link`: betroffene Tickets hinzufügen oder entfernen
fn link(context: &CliContext, args: &LinkArgs) -> Result<u8> {
    require_add_or_remove(&args.add, &args.remove)?;
    context.with_board(async |rt| {
        let current = rt.services.questions.get(&args.id).await?;
        let removed = canonical_tickets(&args.remove)?;
        let next = merge(&current.view.question.ticket_ids, &removed, &args.add);
        let updated = rt
            .services
            .questions
            .set_tickets(
                &args.id,
                &next,
                &actor(context, rt, &args.who)?,
                &BoardQuestionWriteOptions::rev(current.view.question.rev),
            )
            .await?;
        let question = &updated.view.question;
        context.printer().result(
            &updated,
            &[format!(
                "Betroffene Tickets von {}: {}",
                question.id,
                join_or_dash(&question.ticket_ids)
            )],
        );
        Ok(0)
    })
}

/// `board question depends`: Abhängigkeiten hinzufügen oder entfernen
fn depends(context: &CliContext, args: &DependsArgs) -> Result<u8> {
    require_add_or_remove(&args.add, &args.remove)?;
    context.with_board(async |rt| {
        let current = rt.services.questions.get(&args.id).await?;
        let removed = args
            .remove
            .iter()
            .map(|id| QuestionIds::reference(id))
            .collect::<Result<Vec<String>>>()?;
        let next = merge(&current.view.question.depends_on, &removed, &args.add);
        let updated = rt
            .services
            .questions
            .set_depends(
                &args.id,
                &next,
                &actor(context, rt, &args.who)?,
                &BoardQuestionWriteOptions::rev(current.view.question.rev),
            )
            .await?;
        let question = &updated.view.question;
        context.printer().result(
            &updated,
            &[format!(
                "Abhängigkeiten von {}: {}",
                question.id,
                join_or_dash(&question.depends_on)
            )],
        );
        Ok(0)
    })
}

/// Ohne `--add` und ohne `--remove` gibt es nichts zu tun
fn require_add_or_remove(add: &[String], remove: &[String]) -> Result<()> {
    if add.is_empty() && remove.is_empty() {
        return Err(HarnessError::new("usage", "Bitte --add oder --remove angeben.").into());
    }
    Ok(())
}

/// Die bisherigen Einträge ohne die entfernten, dann die neuen
fn merge(current: &[String], removed: &[String], added: &[String]) -> Vec<String> {
    current
        .iter()
        .filter(|id| !removed.contains(id))
        .chain(added.iter())
        .cloned()
        .collect()
}

/// Kanonische Ticket-IDs der zu entfernenden Tickets
fn canonical_tickets(ids: &[String]) -> Result<Vec<String>> {
    ids.iter().map(|id| TicketIds::canonical(id)).collect()
}

/// Eine Aufzählung oder „–“, wenn sie leer ist
fn join_or_dash(items: &[String]) -> String {
    if items.is_empty() {
        "–".to_string()
    } else {
        items.join(", ")
    }
}

/// Der Text des neuesten Verlaufseintrags, sonst leer
fn last_history(detail: &nexus_core::board::BoardQuestionDetail) -> String {
    detail
        .history
        .last()
        .map(|entry| entry.text.clone())
        .unwrap_or_default()
}

/// Zustand aus `--state`; ohne Angabe nur offene, „alle“ heißt ohne Filter
fn state(option: Option<&str>) -> Result<Option<QuestionState>> {
    let Some(raw) = option else {
        return Ok(Some(QuestionState::Open));
    };
    match raw.to_lowercase().as_str() {
        "offen" | "open" => Ok(Some(QuestionState::Open)),
        "beantwortet" | "answered" => Ok(Some(QuestionState::Answered)),
        "verworfen" | "discarded" => Ok(Some(QuestionState::Discarded)),
        "alle" | "all" => Ok(None),
        _ => Err(HarnessError::new(
            "state-invalid",
            format!("Zustand „{raw}“ ist ungültig. Erlaubt: offen, beantwortet, verworfen, alle."),
        )
        .into()),
    }
}

/// Handelnder bei Agenten-Befehlen: Art wie bei Datei-Fragen, Name aus `--author` oder `board.yaml`
fn actor(context: &CliContext, rt: &BoardRuntime, who: &WhoArgs) -> Result<BoardQuestionActor> {
    let name = who
        .author
        .clone()
        .unwrap_or_else(|| rt.config.user_name.clone());
    let question_actor =
        context.question_actor(who.kind.as_deref(), Some(&name), who.session.as_deref())?;
    Ok(BoardQuestionActor {
        actor: rt.actor(Some(&question_actor.author)),
        kind: question_actor.kind,
        session: question_actor.session,
    })
}

/// Handelnder beim Entscheiden: immer ein Mensch (die Sitzungsprüfung lief schon vor dem Öffnen
/// der Datenbank)
fn human(rt: &BoardRuntime, author: Option<&str>) -> BoardQuestionActor {
    BoardQuestionActor {
        actor: rt.actor(author),
        kind: nexus_core::AuthorKind::Human,
        session: None,
    }
}

/// Projekt aus `--project`, sonst das Projekt des Boards dieses Repos bzw. des aktiven Boards
async fn project_key(
    context: &CliContext,
    rt: &BoardRuntime,
    option: Option<&str>,
) -> Result<String> {
    if let Some(option) = option {
        return Ok(option.to_string());
    }
    let board_id = BoardSelection
        .one(&rt.services, &rt.config, None, Some(context.project_dir()))
        .await?;
    project_of(&rt.services, board_id).await
}

/// Projektschlüssel eines Boards
async fn project_of(
    services: &BoardServices<nexus_db::AnyBoardDatabase>,
    board_id: i32,
) -> Result<String> {
    services
        .boards
        .list(&nexus_core::board::board_service::BoardListFilter::default())
        .await?
        .into_iter()
        .find(|summary| summary.board.id == board_id)
        .map(|summary| summary.project_key)
        .ok_or_else(|| {
            HarnessError::new(
                "board-not-found",
                format!("Board {board_id} gibt es nicht."),
            )
            .into()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ohne_state_zahlen_nur_offene_fragen() {
        assert_eq!(state(None).unwrap(), Some(QuestionState::Open));
    }

    #[test]
    fn deutsche_und_englische_zustande_gelten_gleich() {
        assert_eq!(state(Some("Offen")).unwrap(), Some(QuestionState::Open));
        assert_eq!(
            state(Some("answered")).unwrap(),
            Some(QuestionState::Answered)
        );
        assert_eq!(
            state(Some("verworfen")).unwrap(),
            Some(QuestionState::Discarded)
        );
        assert_eq!(state(Some("alle")).unwrap(), None);
        assert_eq!(state(Some("all")).unwrap(), None);
    }

    #[test]
    fn ein_unbekannter_zustand_wird_mit_state_invalid_abgelehnt() {
        let error = state(Some("quatsch")).unwrap_err();
        assert_eq!(error.code(), Some("state-invalid"));
        assert_eq!(
            error.to_string(),
            "Zustand „quatsch“ ist ungültig. Erlaubt: offen, beantwortet, verworfen, alle."
        );
    }

    #[test]
    fn ohne_add_und_remove_meldet_der_befehl_usage() {
        let error = require_add_or_remove(&[], &[]).unwrap_err();
        assert_eq!(error.code(), Some("usage"));
        assert_eq!(error.to_string(), "Bitte --add oder --remove angeben.");
    }

    #[test]
    fn entfernte_eintrage_fallen_weg_und_neue_kommen_ans_ende() {
        let current = vec!["ARC-0001".to_string(), "ARC-0002".to_string()];
        assert_eq!(
            merge(
                &current,
                &["ARC-0001".to_string()],
                &["ARC-0003".to_string()]
            ),
            vec!["ARC-0002".to_string(), "ARC-0003".to_string()]
        );
    }

    #[test]
    fn zu_entfernende_tickets_werden_auf_die_kanonische_schreibweise_gebracht() {
        assert_eq!(
            canonical_tickets(&["arc-7".to_string()]).unwrap(),
            vec!["ARC-0007".to_string()]
        );
    }

    #[test]
    fn eine_leere_aufzahlung_wird_zum_gedankenstrich() {
        assert_eq!(join_or_dash(&[]), "–");
        assert_eq!(join_or_dash(&["ARC-0001".to_string()]), "ARC-0001");
    }
}
