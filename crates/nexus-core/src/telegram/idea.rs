//! Ideen aus dem Bot verstehen: der Board-Kontext mit festem Budget, die Arbeitsanweisung für
//! Claude und die Prüfung der Antwort.
//!
//! Der Kontext wird im Server gesammelt (eigene Boards, offene Tickets, Notizen, Fragen); hier
//! steht nur, was daraus in den Prompt kommt und was eine Antwort erfüllen muss. Der Prüfer ist
//! nach [`crate::docs::TicketDraftValidator`] gebaut: Unbrauchbares wird nach Möglichkeit
//! zurechtgebogen und als Hinweis gemeldet, erst ein unbrauchbares Ziel ist ein Fehler.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::board::basics::TicketStatus;
use crate::error::{HarnessError, Result};

/// Höchstens so viele Tickets gehen in den Kontext
pub const MAX_CONTEXT_TICKETS: usize = 200;
/// Höchstens so viele Zeichen umfasst der Board-Kontext im Prompt
pub const MAX_CONTEXT_CHARS: usize = 30_000;
/// Mehr Rückfragen beantwortet niemand mehr in einem Chat
pub const MAX_QUESTIONS: usize = 3;
/// Höchstlänge des Titels (wie ein Ticket- oder Notiztitel)
pub const MAX_TITLE: usize = 120;
/// Höchstlänge der Details
pub const MAX_DETAILS: usize = 4000;
/// Höchstlänge der kurzen Begleittexte (Zusammenfassung, Begründungen, Rückfragen)
pub const MAX_SHORT: usize = 400;
/// Höchstens so viele Antwortmöglichkeiten hat eine Rückfrage (mehr passen nicht als Knöpfe)
pub const MAX_OPTIONS: usize = 4;
/// Höchstlänge der Beschriftung einer Antwortmöglichkeit (sie steht auf einem Knopf)
pub const MAX_OPTION_LABEL: usize = 40;

// ---------------------------------------------------------------------------------------------
// Der Entwurf
// ---------------------------------------------------------------------------------------------

/// Was aus einer Idee werden soll
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdeaKind {
    /// Eine Notiz auf dem Board (die Vorgabe)
    Note,
    /// Ein Ticket im Backlog
    Ticket,
    /// Ein Kommentar an einem vorhandenen Ticket
    Comment,
}

impl IdeaKind {
    /// Schreibweise im JSON-Schema
    pub fn as_str(&self) -> &'static str {
        match self {
            IdeaKind::Note => "note",
            IdeaKind::Ticket => "ticket",
            IdeaKind::Comment => "comment",
        }
    }

    /// Liest die Schreibweise des Schemas; nichts anderes zählt
    pub fn parse(input: &str) -> Option<Self> {
        match input.trim().to_lowercase().as_str() {
            "note" => Some(IdeaKind::Note),
            "ticket" => Some(IdeaKind::Ticket),
            "comment" => Some(IdeaKind::Comment),
            _ => None,
        }
    }
}

/// Wohin die Idee gehört
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaTargetRef {
    pub kind: IdeaKind,
    pub board_id: i32,
    /// Das Ticket bei `comment`; bei Notiz und Ticket immer null
    pub ticket_id: Option<String>,
}

/// Ein Bezug auf ein vorhandenes Ticket samt Begründung
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaRelated {
    pub ticket_id: String,
    pub why: String,
}

/// Eine Dublette (Ticket oder Notiz) samt Begründung
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaDuplicate {
    pub id: String,
    pub why: String,
}

/// Eine Antwortmöglichkeit einer Rückfrage
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaAskOption {
    /// Kurz genug für einen Knopf (höchstens [`MAX_OPTION_LABEL`] Zeichen)
    pub label: String,
    /// Ein Satz dazu, was die Wahl bedeutet
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Eine Rückfrage vor dem Anlegen: Text und – wenn es naheliegende Antworten gibt – zwei bis
/// [`MAX_OPTIONS`] Möglichkeiten zum Antippen. Frei antworten geht immer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "IdeaAskRepr")]
pub struct IdeaAsk {
    pub text: String,
    /// Leer oder zwei bis [`MAX_OPTIONS`] Möglichkeiten
    pub options: Vec<IdeaAskOption>,
    /// Die empfohlene Möglichkeit (Index in `options`)
    pub recommended: Option<usize>,
    /// Passen mehrere Möglichkeiten zugleich?
    pub multi_select: bool,
}

impl IdeaAsk {
    /// Eine Rückfrage ohne Möglichkeiten; beantwortet wird sie frei
    pub fn open(text: &str) -> Self {
        Self {
            text: text.to_string(),
            options: Vec::new(),
            recommended: None,
            multi_select: false,
        }
    }
}

/// Wie eine Rückfrage gespeichert sein kann: früher als reiner Text, heute als Objekt. So
/// bleiben Entwürfe aus `drafts.json` vor der Umstellung lesbar
#[derive(Deserialize)]
#[serde(untagged)]
enum IdeaAskRepr {
    Text(String),
    #[serde(rename_all = "camelCase")]
    Full {
        text: String,
        #[serde(default)]
        options: Vec<IdeaAskOption>,
        #[serde(default)]
        recommended: Option<usize>,
        #[serde(default)]
        multi_select: bool,
    },
}

impl From<IdeaAskRepr> for IdeaAsk {
    fn from(repr: IdeaAskRepr) -> Self {
        match repr {
            IdeaAskRepr::Text(text) => IdeaAsk::open(&text),
            IdeaAskRepr::Full {
                text,
                options,
                recommended,
                multi_select,
            } => IdeaAsk {
                text,
                options,
                recommended,
                multi_select,
            },
        }
    }
}

/// Der geprüfte Entwurf einer Idee
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaDraft {
    /// Ein Satz, was gemeint ist
    pub summary: String,
    pub target: IdeaTargetRef,
    pub title: String,
    pub details: String,
    pub related: Vec<IdeaRelated>,
    pub duplicates: Vec<IdeaDuplicate>,
    /// Rückfragen vor dem Anlegen (höchstens [`MAX_QUESTIONS`])
    pub questions: Vec<IdeaAsk>,
    /// Wie sicher sich Claude ist (0–1)
    pub confidence: f64,
    /// Die Eingabe, aus der der Entwurf entstand (Transkript oder Text)
    pub transcript: String,
    /// Wie oft der Entwurf schon gelaufen ist (erste Runde = 1)
    pub rounds: i64,
}

/// Der geprüfte Entwurf und Hinweise auf alles, was dabei zurechtgebogen wurde
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdeaDraftCheck {
    pub draft: IdeaDraft,
    pub dropped: Vec<String>,
}

// ---------------------------------------------------------------------------------------------
// Der Kontext
// ---------------------------------------------------------------------------------------------

/// Ein eigenes Board des Benutzers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaBoard {
    pub id: i32,
    pub name: String,
    /// Hauptordner des Repositorys, falls zugeordnet
    pub repo_path: Option<String>,
}

/// Ein offenes Ticket im Kontext
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaTicket {
    pub id: String,
    pub board_id: i32,
    pub title: String,
    pub status: TicketStatus,
    pub labels: Vec<String>,
    /// Letzte Änderung (ISO); bestimmt die Reihenfolge im Budget
    pub updated_at: String,
}

/// Eine offene Notiz im Kontext
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaNote {
    pub id: i32,
    pub board_id: i32,
    pub title: String,
    /// `true` = `NoteSimilarity` hält sie für denselben Befund wie die Eingabe
    pub similar: bool,
}

/// Eine offene Frage im Kontext
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaQuestion {
    pub id: String,
    pub title: String,
}

/// Alles, was Claude über die eigenen Boards wissen muss
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaContext {
    pub boards: Vec<IdeaBoard>,
    /// Board, auf dem der Benutzer gerade arbeitet (steht im Prompt vorn)
    pub active_board_id: Option<i32>,
    pub tickets: Vec<IdeaTicket>,
    pub notes: Vec<IdeaNote>,
    pub questions: Vec<IdeaQuestion>,
}

impl IdeaContext {
    /// Gibt es dieses Board unter den eigenen?
    pub fn has_board(&self, board_id: i32) -> bool {
        self.boards.iter().any(|board| board.id == board_id)
    }

    /// Das Ticket mit dieser Nummer, wenn es im Kontext steht
    pub fn ticket(&self, id: &str) -> Option<&IdeaTicket> {
        let id = id.trim();
        self.tickets
            .iter()
            .find(|ticket| ticket.id.eq_ignore_ascii_case(id))
    }

    /// Gibt es eine Notiz mit dieser Nummer?
    pub fn has_note(&self, id: &str) -> bool {
        id.trim()
            .parse::<i32>()
            .is_ok_and(|number| self.notes.iter().any(|note| note.id == number))
    }

    /// Das Board, auf das eine Idee ohne brauchbare Angabe fällt: das aktive, sonst das erste
    fn default_board(&self) -> Option<i32> {
        self.active_board_id
            .filter(|id| self.has_board(*id))
            .or_else(|| self.boards.first().map(|board| board.id))
    }

    /// Die offenen Tickets in der Reihenfolge des Budgets: aktives Board zuerst, dann nach
    /// Änderungsdatum (neueste zuerst); erledigte Tickets fallen weg
    fn budgeted_tickets(&self) -> Vec<&IdeaTicket> {
        let active = self.active_board_id;
        let mut tickets: Vec<&IdeaTicket> = self
            .tickets
            .iter()
            .filter(|ticket| !is_closed(ticket.status))
            .collect();
        tickets.sort_by(|a, b| {
            let a_active = Some(a.board_id) == active;
            let b_active = Some(b.board_id) == active;
            b_active
                .cmp(&a_active)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
        });
        tickets.truncate(MAX_CONTEXT_TICKETS);
        tickets
    }
}

/// Erledigte und nicht mehr nötige Tickets gehören nicht in den Kontext
fn is_closed(status: TicketStatus) -> bool {
    matches!(status, TicketStatus::Done | TicketStatus::NotNeeded)
}

// ---------------------------------------------------------------------------------------------
// Die Arbeitsanweisung
// ---------------------------------------------------------------------------------------------

/// Der bisherige Entwurf und die Antwort des Benutzers darauf (Verfeinerungsrunde)
#[derive(Debug, Clone, PartialEq)]
pub struct IdeaRefinement<'a> {
    pub draft: &'a IdeaDraft,
    pub answer: &'a str,
}

/// Das JSON-Schema der Antwort – genau so, wie der Prüfer es liest
const SCHEMA: &str = concat!(
    "{\"summary\": string, ",
    "\"target\": {\"kind\": \"note\" | \"ticket\" | \"comment\", \"boardId\": number, ",
    "\"ticketId\": string | null}, ",
    "\"title\": string, \"details\": string, ",
    "\"related\": [{\"ticketId\": string, \"why\": string}], ",
    "\"duplicates\": [{\"id\": string, \"why\": string}], ",
    "\"questions\": [{\"text\": string, \"options\": [{\"label\": string, \"detail\": string | null}], ",
    "\"recommended\": number | null, \"multiSelect\": boolean}], ",
    "\"confidence\": number}"
);

/// Die Arbeitsanweisung für einen Lauf: Aufgabe, Board-Kontext, Eingabe, Schema und Regeln.
/// Mit `previous` wird aus dem Lauf eine Verfeinerungsrunde: bisheriger Entwurf und Antwort
/// des Benutzers stehen dann mit im Prompt.
pub fn build_prompt(
    context: &IdeaContext,
    input: &str,
    previous: Option<IdeaRefinement<'_>>,
) -> String {
    let mut lines: Vec<String> = vec![
        "Du verstehst eine kurz hingeworfene Idee und schlägst vor, was daraus auf dem Board werden soll.".into(),
        String::new(),
        "Regeln:".into(),
        "- Nenne ausschließlich Boards und Ticketnummern, die unten im Board-Kontext stehen; erfinde nichts dazu.".into(),
        "- Im Zweifel wird die Idee eine Notiz („note“). Ein Ticket („ticket“) nur, wenn die Idee eine klar umsetzbare Aufgabe beschreibt; ein Kommentar („comment“) nur, wenn sie erkennbar zu einem genannten Ticket gehört – dann ist „ticketId“ Pflicht.".into(),
        format!("- Höchstens {MAX_QUESTIONS} Rückfragen, und nur zu dem, was ohne Antwort nicht entschieden werden kann. Ist alles klar, bleibt „questions“ leer."),
        format!("- Jede Rückfrage bietet 2 bis {MAX_OPTIONS} naheliegende Antworten als „options“ (Beschriftung höchstens {MAX_OPTION_LABEL} Zeichen, „detail“ ein kurzer Satz oder null); frei antworten kann der Nutzer immer. „recommended“ ist der Index der naheliegendsten Antwort oder null, „multiSelect“ nur true, wenn mehrere Antworten zugleich passen. Gibt es keine sinnvollen Vorschläge, bleibt „options“ leer."),
        format!("- „title“ ist ein ganzer Satz mit höchstens {MAX_TITLE} Zeichen, „details“ hat höchstens {MAX_DETAILS} Zeichen (wo, woran man es sieht, vermutete Ursache, Vorschlag)."),
        "- „related“ nennt Tickets, die inhaltlich dazugehören; „duplicates“ nur Tickets oder Notizen, die denselben Punkt schon beschreiben.".into(),
        "- „confidence“ liegt zwischen 0 und 1 und sagt, wie sicher du dir bei Ziel und Inhalt bist.".into(),
        "- Antworte deutsch und ausschließlich mit einem JSON-Objekt, ohne Markdown und ohne Werkzeuge.".into(),
        String::new(),
        "Antwortformat:".into(),
        SCHEMA.into(),
    ];
    lines.extend(context_part(context));
    lines.extend(previous_part(previous));
    lines.push(String::new());
    lines.push("## Eingabe".into());
    lines.push(cut(input.trim(), MAX_DETAILS));
    lines.join("\n")
}

/// Der Board-Kontext als Abschnitt; Tickets, Notizen und Fragen halten gemeinsam das Budget
/// von [`MAX_CONTEXT_CHARS`] Zeichen ein
fn context_part(context: &IdeaContext) -> Vec<String> {
    let mut lines: Vec<String> = vec![String::new(), "## Board-Kontext".into()];
    let mut left = MAX_CONTEXT_CHARS;
    let mut cut_off = false;

    lines.push(String::new());
    lines.push("### Eigene Boards".into());
    for board in &context.boards {
        let active = if Some(board.id) == context.active_board_id {
            " – aktives Board"
        } else {
            ""
        };
        let repo = match board.repo_path.as_deref().map(str::trim) {
            Some(path) if !path.is_empty() => format!(" (Repo {path})"),
            _ => String::new(),
        };
        // Die Boards gehören immer dazu: ohne sie kann Claude kein Ziel nennen
        let line = format!("- Board {} „{}“{repo}{active}", board.id, board.name);
        left = left.saturating_sub(line.chars().count());
        lines.push(line);
    }
    if context.boards.is_empty() {
        lines.push("- (keine)".into());
    }

    lines.push(String::new());
    lines.push("### Offene Tickets".into());
    let tickets = context.budgeted_tickets();
    cut_off |= tickets.len()
        < context
            .tickets
            .iter()
            .filter(|t| !is_closed(t.status))
            .count();
    let mut written = 0usize;
    for ticket in tickets {
        let labels = if ticket.labels.is_empty() {
            String::new()
        } else {
            format!(", Labels: {}", ticket.labels.join(", "))
        };
        let line = format!(
            "- {} [{}] „{}“ (Board {}{labels})",
            ticket.id,
            ticket.status.as_str(),
            ticket.title,
            ticket.board_id
        );
        if !fits(&line, &mut left) {
            cut_off = true;
            break;
        }
        written += 1;
        lines.push(line);
    }
    if written == 0 {
        lines.push("- (keine)".into());
    }

    lines.push(String::new());
    lines.push("### Offene Notizen".into());
    // Ähnliche Notizen zuerst: sie entscheiden über „duplicates“
    let mut notes: Vec<&IdeaNote> = context.notes.iter().collect();
    notes.sort_by_key(|note| std::cmp::Reverse(note.similar));
    let mut written = 0usize;
    for note in notes {
        let similar = if note.similar {
            " – ähnelt der Eingabe"
        } else {
            ""
        };
        let line = format!(
            "- Notiz {} „{}“ (Board {}){similar}",
            note.id, note.title, note.board_id
        );
        if !fits(&line, &mut left) {
            cut_off = true;
            break;
        }
        written += 1;
        lines.push(line);
    }
    if written == 0 {
        lines.push("- (keine)".into());
    }

    lines.push(String::new());
    lines.push("### Offene Fragen".into());
    let mut written = 0usize;
    for question in &context.questions {
        let line = format!("- {} „{}“", question.id, question.title);
        if !fits(&line, &mut left) {
            cut_off = true;
            break;
        }
        written += 1;
        lines.push(line);
    }
    if written == 0 {
        lines.push("- (keine)".into());
    }

    if cut_off {
        lines.push("- (gekürzt: weitere Einträge passen nicht ins Budget)".into());
    }
    lines
}

/// Passt die Zeile noch ins Budget? Dann wird sie abgezogen
fn fits(line: &str, left: &mut usize) -> bool {
    let need = line.chars().count();
    if need > *left {
        return false;
    }
    *left -= need;
    true
}

/// Bisheriger Entwurf und Antwort des Benutzers; ohne Verfeinerung bleibt der Abschnitt weg
fn previous_part(previous: Option<IdeaRefinement<'_>>) -> Vec<String> {
    let Some(previous) = previous else {
        return Vec::new();
    };
    let draft = previous.draft;
    let mut lines = vec![
        String::new(),
        "## Bisheriger Entwurf".into(),
        format!("Zusammenfassung: {}", draft.summary),
        format!(
            "Ziel: {} auf Board {}{}",
            draft.target.kind.as_str(),
            draft.target.board_id,
            match draft.target.ticket_id.as_deref() {
                Some(ticket) => format!(" an {ticket}"),
                None => String::new(),
            }
        ),
        format!("Titel: {}", draft.title),
        format!("Details: {}", cut(&draft.details, MAX_DETAILS)),
    ];
    if !draft.questions.is_empty() {
        let asked: Vec<&str> = draft
            .questions
            .iter()
            .map(|ask| ask.text.as_str())
            .collect();
        lines.push(format!("Rückfragen: {}", asked.join(" | ")));
    }
    lines.push(String::new());
    lines.push("## Antwort des Nutzers".into());
    lines.push(cut(previous.answer.trim(), MAX_DETAILS));
    lines.push(String::new());
    lines.push(
        "Arbeite die Antwort in den Entwurf ein und beantworte beantwortete Rückfragen nicht erneut.".into(),
    );
    lines
}

// ---------------------------------------------------------------------------------------------
// Der Prüfer
// ---------------------------------------------------------------------------------------------

/// Prüft die Antwort von Claude auf eine Idee.
#[derive(Debug, Clone, Copy, Default)]
pub struct IdeaDraftValidator;

impl IdeaDraftValidator {
    /// Neuer Prüfer (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Liefert den brauchbaren Entwurf und Hinweise auf alles Zurechtgebogene; ein Ziel, das
    /// sich nicht auf die eigenen Boards abbilden lässt, ist ein Fehler
    pub fn check(
        &self,
        value: &Value,
        context: &IdeaContext,
        transcript: &str,
        rounds: i64,
    ) -> Result<IdeaDraftCheck> {
        if !value.is_object() {
            return Err(failed("Antwort ist kein Entwurf."));
        }
        let mut dropped: Vec<String> = Vec::new();
        let title = text(value.get("title"));
        if title.is_empty() {
            return Err(failed("Die Antwort enthält keinen Titel."));
        }
        let target = self.target(value.get("target"), context, &mut dropped)?;
        let summary = match text(value.get("summary")) {
            summary if summary.is_empty() => title.clone(),
            summary => summary,
        };
        let draft = IdeaDraft {
            summary: shorten(&summary, MAX_SHORT, "Zusammenfassung", &mut dropped),
            target,
            title: shorten(&title, MAX_TITLE, "Titel", &mut dropped),
            details: shorten(
                &text(value.get("details")),
                MAX_DETAILS,
                "Details",
                &mut dropped,
            ),
            related: related(value.get("related"), context, &mut dropped),
            duplicates: duplicates(value.get("duplicates"), context, &mut dropped),
            questions: questions(value.get("questions"), &mut dropped),
            confidence: confidence(value.get("confidence"), &mut dropped),
            transcript: transcript.trim().to_string(),
            rounds,
        };
        Ok(IdeaDraftCheck { draft, dropped })
    }

    /// Das Ziel: Art, eigenes Board und – bei `comment` – ein vorhandenes Ticket
    fn target(
        &self,
        value: Option<&Value>,
        context: &IdeaContext,
        dropped: &mut Vec<String>,
    ) -> Result<IdeaTargetRef> {
        let raw = text(value.and_then(|found| found.get("kind")));
        let Some(kind) = IdeaKind::parse(&raw) else {
            return Err(failed(&format!(
                "Ziel „{raw}“ ist unbekannt. Erlaubt: note, ticket, comment."
            )));
        };
        let ticket_id = text(value.and_then(|found| found.get("ticketId")));
        let ticket = context.ticket(&ticket_id);
        // Ein Kommentar ohne vorhandenes Ticket hat keinen Ort
        if kind == IdeaKind::Comment {
            if ticket_id.is_empty() {
                return Err(failed("Ziel „comment“ nennt kein Ticket."));
            }
            let Some(ticket) = ticket else {
                return Err(failed(&format!("Ticket „{ticket_id}“ gibt es nicht.")));
            };
            return Ok(IdeaTargetRef {
                kind,
                board_id: ticket.board_id,
                ticket_id: Some(ticket.id.clone()),
            });
        }
        if !ticket_id.is_empty() {
            dropped.push(format!(
                "Ticket „{ticket_id}“ beim Ziel „{}“ entfernt.",
                kind.as_str()
            ));
        }
        let board_id = integer(value.and_then(|found| found.get("boardId")));
        let board_id = match board_id {
            // Fremde Boards bedient deren Besitzer; der Kontext nennt nur die eigenen
            Some(board_id) if !context.has_board(board_id as i32) => {
                return Err(failed(&format!(
                    "Board {board_id} gehört nicht zu dir oder gibt es nicht."
                )))
            }
            Some(board_id) => board_id as i32,
            None => match context.default_board() {
                Some(board_id) => {
                    dropped.push(format!("Ohne Board-Angabe: Board {board_id} genommen."));
                    board_id
                }
                None => return Err(failed("Die Antwort nennt kein Board.")),
            },
        };
        Ok(IdeaTargetRef {
            kind,
            board_id,
            ticket_id: None,
        })
    }
}

/// Bezüge auf vorhandene Tickets; alles andere fällt weg
fn related(
    value: Option<&Value>,
    context: &IdeaContext,
    dropped: &mut Vec<String>,
) -> Vec<IdeaRelated> {
    let mut out: Vec<IdeaRelated> = Vec::new();
    for item in array(value) {
        let id = text(item.get("ticketId"));
        match context.ticket(&id) {
            Some(ticket) if !out.iter().any(|found| found.ticket_id == ticket.id) => {
                out.push(IdeaRelated {
                    ticket_id: ticket.id.clone(),
                    why: cut(&text(item.get("why")), MAX_SHORT),
                });
            }
            Some(_) => {}
            None => dropped.push(format!("Bezug „{id}“ entfernt: kein bekanntes Ticket.")),
        }
    }
    out
}

/// Dubletten: vorhandene Tickets oder Notizen; alles andere fällt weg
fn duplicates(
    value: Option<&Value>,
    context: &IdeaContext,
    dropped: &mut Vec<String>,
) -> Vec<IdeaDuplicate> {
    let mut out: Vec<IdeaDuplicate> = Vec::new();
    for item in array(value) {
        let id = text(item.get("id"));
        let known = context
            .ticket(&id)
            .map(|ticket| ticket.id.clone())
            .or_else(|| context.has_note(&id).then(|| id.trim().to_string()));
        match known {
            Some(known) if !out.iter().any(|found| found.id == known) => out.push(IdeaDuplicate {
                id: known,
                why: cut(&text(item.get("why")), MAX_SHORT),
            }),
            Some(_) => {}
            None => dropped.push(format!(
                "Dublette „{id}“ entfernt: kein bekanntes Ticket und keine bekannte Notiz."
            )),
        }
    }
    out
}

/// Höchstens [`MAX_QUESTIONS`] Rückfragen, jede ohne Rand und nicht zu lang. Eine Rückfrage
/// als reiner Text wird eine ohne Möglichkeiten
fn questions(value: Option<&Value>, dropped: &mut Vec<String>) -> Vec<IdeaAsk> {
    let all: Vec<IdeaAsk> = array(value)
        .iter()
        .filter_map(|item| ask(item, dropped))
        .collect();
    if all.len() > MAX_QUESTIONS {
        dropped.push(format!(
            "Nur die ersten {MAX_QUESTIONS} von {} Rückfragen übernommen.",
            all.len()
        ));
    }
    all.into_iter().take(MAX_QUESTIONS).collect()
}

/// Eine Rückfrage; ohne Text fällt sie weg
fn ask(item: &Value, dropped: &mut Vec<String>) -> Option<IdeaAsk> {
    if item.is_string() {
        let question = text(Some(item));
        return (!question.is_empty()).then(|| IdeaAsk::open(&cut(&question, MAX_SHORT)));
    }
    let question = text(item.get("text"));
    if question.is_empty() {
        return None;
    }
    let question = cut(&question, MAX_SHORT);
    let mut options: Vec<IdeaAskOption> = Vec::new();
    for option in array(item.get("options")) {
        let label = match option.is_string() {
            true => text(Some(&option)),
            false => text(option.get("label")),
        };
        if label.is_empty() {
            continue;
        }
        if label.chars().count() > MAX_OPTION_LABEL {
            dropped.push(format!(
                "Antwort „{}…“ auf {MAX_OPTION_LABEL} Zeichen gekürzt.",
                cut(&label, 20)
            ));
        }
        let detail = text(option.get("detail"));
        options.push(IdeaAskOption {
            label: cut(&label, MAX_OPTION_LABEL),
            detail: (!detail.is_empty()).then(|| cut(&detail, MAX_SHORT)),
        });
    }
    if options.len() > MAX_OPTIONS {
        dropped.push(format!(
            "Nur die ersten {MAX_OPTIONS} von {} Antworten auf „{question}“ übernommen.",
            options.len()
        ));
        options.truncate(MAX_OPTIONS);
    }
    // Eine einzige Möglichkeit ist keine Wahl
    if options.len() == 1 {
        options.clear();
    }
    let recommended = integer(item.get("recommended"))
        .and_then(|index| usize::try_from(index).ok())
        .filter(|index| *index < options.len());
    let multi_select = item
        .get("multiSelect")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && options.len() > 1;
    Some(IdeaAsk {
        text: question,
        options,
        recommended,
        multi_select,
    })
}

/// Sicherheit zwischen 0 und 1; Werte daneben werden gekappt, Unbrauchbares wird 0
fn confidence(value: Option<&Value>, dropped: &mut Vec<String>) -> f64 {
    let Some(number) = value
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
    else {
        dropped.push("Ohne brauchbare „confidence“: 0 genommen.".into());
        return 0.0;
    };
    if number < 0.0 || number > 1.0 {
        let clamped = number.clamp(0.0, 1.0);
        dropped.push(format!("„confidence“ {number} auf {clamped} gekappt."));
        return clamped;
    }
    number
}

/// Kürzt das Feld und meldet es als Hinweis
fn shorten(value: &str, max: usize, name: &str, dropped: &mut Vec<String>) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    dropped.push(format!("{name} auf {max} Zeichen gekürzt."));
    cut(value, max)
}

/// Kürzt auf so viele Zeichen (wie `cut` in den Docs-Diensten)
fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// Ein Textfeld ohne Rand; alles, was kein Text ist, gilt als leer
fn text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .map(|text| text.trim().to_string())
        .unwrap_or_default()
}

/// Eine Liste von Objekten; alles andere ergibt eine leere Liste
fn array(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

/// Ganze Zahl; `None`, wenn der Wert keine ist
fn integer(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// „Die Antwort ist unbrauchbar“ – derselbe Code wie bei den Ticket-Entwürfen
fn failed(message: &str) -> crate::error::Error {
    HarnessError::new("analysis-failed", message).into()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Ein Board mit dieser Nummer
    fn board(id: i32, name: &str) -> IdeaBoard {
        IdeaBoard {
            id,
            name: name.into(),
            repo_path: Some(format!("/repos/{name}")),
        }
    }

    /// Ein offenes Ticket
    fn ticket(id: &str, board_id: i32, updated_at: &str) -> IdeaTicket {
        IdeaTicket {
            id: id.into(),
            board_id,
            title: format!("Titel {id}"),
            status: TicketStatus::Backlog,
            labels: vec!["server".into()],
            updated_at: updated_at.into(),
        }
    }

    /// Kontext mit zwei Boards, zwei Tickets, einer Notiz und einer Frage
    fn context() -> IdeaContext {
        IdeaContext {
            boards: vec![board(1, "nexus"), board(2, "harness")],
            active_board_id: Some(1),
            tickets: vec![
                ticket("NXS-0001", 1, "2026-11-01T10:00:00.000Z"),
                ticket("AIH-0002", 2, "2026-11-02T10:00:00.000Z"),
            ],
            notes: vec![IdeaNote {
                id: 7,
                board_id: 1,
                title: "Cache läuft nie ab".into(),
                similar: true,
            }],
            questions: vec![IdeaQuestion {
                id: "NXS-F0001".into(),
                title: "Auch für SQL Server?".into(),
            }],
        }
    }

    /// Eine vollständige Antwort, mit weiteren Feldern überschreibbar
    fn answer(extra: Value) -> Value {
        let mut item = json!({
            "summary": "Der Cache läuft nie ab.",
            "target": { "kind": "note", "boardId": 1, "ticketId": null },
            "title": "Cache der Board-Ansicht läuft nie ab",
            "details": "Fällt in der Ansicht auf; Vorschlag: Lebenszeit setzen.",
            "related": [{ "ticketId": "NXS-0001", "why": "gleiche Ansicht" }],
            "duplicates": [],
            "questions": [],
            "confidence": 0.8,
        });
        for (key, value) in extra.as_object().cloned().unwrap_or_default() {
            item[key] = value;
        }
        item
    }

    /// Der geprüfte Entwurf oder ein Panik mit den Hinweisen
    fn checked(value: &Value) -> IdeaDraftCheck {
        IdeaDraftValidator::new()
            .check(value, &context(), "Eingabe", 1)
            .expect("Entwurf")
    }

    #[test]
    fn nennt_im_prompt_schema_boards_tickets_notizen_und_fragen() {
        let prompt = build_prompt(&context(), "  Der Cache läuft nie ab.  ", None);

        assert!(prompt.contains("\"kind\": \"note\" | \"ticket\" | \"comment\""));
        assert!(prompt.contains("- Board 1 „nexus“ (Repo /repos/nexus) – aktives Board"));
        assert!(prompt.contains("- Board 2 „harness“ (Repo /repos/harness)\n"));
        assert!(prompt.contains("- NXS-0001 [backlog] „Titel NXS-0001“ (Board 1, Labels: server)"));
        assert!(prompt.contains("- Notiz 7 „Cache läuft nie ab“ (Board 1) – ähnelt der Eingabe"));
        assert!(prompt.contains("- NXS-F0001 „Auch für SQL Server?“"));
        assert!(prompt.ends_with("## Eingabe\nDer Cache läuft nie ab."));
        assert!(!prompt.contains("## Bisheriger Entwurf"));
    }

    #[test]
    fn haelt_das_budget_von_200_tickets_und_30000_zeichen_ein() {
        let mut context = context();
        context.tickets = (0..400)
            .map(|index| ticket(&format!("NXS-{index:04}"), 1, "2026-11-01T10:00:00.000Z"))
            .collect();

        let prompt = build_prompt(&context, "Eingabe", None);

        let genannt = context
            .tickets
            .iter()
            .filter(|ticket| prompt.contains(&format!("- {} [", ticket.id)))
            .count();
        assert_eq!(genannt, MAX_CONTEXT_TICKETS);
        assert!(prompt.contains("(gekürzt:"));

        // Dasselbe über die Zeichen: lange Titel reißen das Budget, bevor 200 Tickets erreicht sind
        context.tickets = (0..400)
            .map(|index| {
                let mut ticket = ticket(&format!("NXS-{index:04}"), 1, "2026-11-01T10:00:00.000Z");
                ticket.title = "x".repeat(500);
                ticket
            })
            .collect();
        let prompt = build_prompt(&context, "Eingabe", None);
        let genannt = context
            .tickets
            .iter()
            .filter(|ticket| prompt.contains(&format!("- {} [", ticket.id)))
            .count();
        assert!(genannt < MAX_CONTEXT_TICKETS, "{genannt} Tickets");
        assert!(prompt.contains("(gekürzt:"));
        let kontext = prompt
            .split("## Board-Kontext")
            .nth(1)
            .expect("Kontext")
            .chars()
            .count();
        // Überschriften und Zeilenumbrüche zählen nicht gegen das Budget der Einträge
        assert!(kontext <= MAX_CONTEXT_CHARS + 1000, "{kontext} Zeichen");
    }

    #[test]
    fn laesst_erledigte_tickets_aus_dem_kontext_weg() {
        let mut context = context();
        context.tickets[0].status = TicketStatus::Done;
        context.tickets[1].status = TicketStatus::NotNeeded;

        let prompt = build_prompt(&context, "Eingabe", None);

        assert!(!prompt.contains("NXS-0001"));
        assert!(!prompt.contains("AIH-0002"));
        assert!(prompt.contains("### Offene Tickets\n- (keine)"));
    }

    #[test]
    fn nennt_tickets_des_aktiven_boards_zuerst_und_dann_die_neuesten() {
        let mut context = context();
        context
            .tickets
            .push(ticket("AIH-0003", 2, "2026-11-03T10:00:00.000Z"));
        let prompt = build_prompt(&context, "Eingabe", None);

        let platz = |id: &str| prompt.find(&format!("- {id} [")).expect("Ticket");
        assert!(platz("NXS-0001") < platz("AIH-0003"));
        assert!(platz("AIH-0003") < platz("AIH-0002"));
    }

    #[test]
    fn nennt_in_der_verfeinerungsrunde_entwurf_und_antwort() {
        let draft = checked(&answer(json!({ "questions": ["Auch für SQL Server?"] }))).draft;

        let prompt = build_prompt(
            &context(),
            "Der Cache läuft nie ab.",
            Some(IdeaRefinement {
                draft: &draft,
                answer: "  Ja, auch dort.  ",
            }),
        );

        assert!(prompt.contains("## Bisheriger Entwurf"));
        assert!(prompt.contains("Titel: Cache der Board-Ansicht läuft nie ab"));
        assert!(prompt.contains("Ziel: note auf Board 1"));
        assert!(prompt.contains("Rückfragen: Auch für SQL Server?"));
        assert!(prompt.contains("## Antwort des Nutzers\nJa, auch dort."));
    }

    #[test]
    fn uebernimmt_eine_gueltige_antwort_unveraendert() {
        let check = checked(&answer(json!({})));

        assert!(check.dropped.is_empty(), "{:?}", check.dropped);
        assert_eq!(check.draft.target.kind, IdeaKind::Note);
        assert_eq!(check.draft.target.board_id, 1);
        assert_eq!(check.draft.target.ticket_id, None);
        assert_eq!(check.draft.related[0].ticket_id, "NXS-0001");
        assert_eq!(check.draft.confidence, 0.8);
        assert_eq!(check.draft.transcript, "Eingabe");
        assert_eq!(check.draft.rounds, 1);
    }

    #[test]
    fn meldet_ein_fremdes_oder_unbekanntes_board() {
        let validator = IdeaDraftValidator::new();
        let error = validator
            .check(
                &answer(json!({ "target": { "kind": "note", "boardId": 9 } })),
                &context(),
                "Eingabe",
                1,
            )
            .unwrap_err();

        assert_eq!(error.code(), Some("analysis-failed"));
        assert!(error.to_string().contains("Board 9"));
    }

    #[test]
    fn meldet_eine_unbekannte_ticketnummer_und_comment_ohne_ticket() {
        let validator = IdeaDraftValidator::new();
        let unbekannt = validator
            .check(
                &answer(json!({ "target": { "kind": "comment", "ticketId": "NXS-9999" } })),
                &context(),
                "Eingabe",
                1,
            )
            .unwrap_err();
        assert_eq!(unbekannt.code(), Some("analysis-failed"));
        assert!(unbekannt.to_string().contains("NXS-9999"));

        let ohne = validator
            .check(
                &answer(json!({ "target": { "kind": "comment", "boardId": 1 } })),
                &context(),
                "Eingabe",
                1,
            )
            .unwrap_err();
        assert_eq!(ohne.code(), Some("analysis-failed"));
        assert!(ohne.to_string().contains("kein Ticket"));
    }

    #[test]
    fn nimmt_beim_kommentar_das_board_des_tickets() {
        let check = checked(&answer(
            json!({ "target": { "kind": "comment", "boardId": 1, "ticketId": " aih-0002 " } }),
        ));

        assert_eq!(check.draft.target.board_id, 2);
        assert_eq!(check.draft.target.ticket_id.as_deref(), Some("AIH-0002"));
    }

    #[test]
    fn kuerzt_zu_lange_felder() {
        let check = checked(&answer(json!({
            "title": "t".repeat(200),
            "details": "d".repeat(5000),
        })));

        assert_eq!(check.draft.title.chars().count(), MAX_TITLE);
        assert_eq!(check.draft.details.chars().count(), MAX_DETAILS);
        assert!(
            check.dropped.iter().any(|hint| hint.contains("Titel")),
            "{:?}",
            check.dropped
        );
        assert!(
            check.dropped.iter().any(|hint| hint.contains("Details")),
            "{:?}",
            check.dropped
        );
    }

    #[test]
    fn nimmt_hoechstens_drei_rueckfragen() {
        let check = checked(&answer(
            json!({ "questions": ["a?", "b?", "", "c?", "d?"] }),
        ));

        assert_eq!(
            check.draft.questions,
            [
                IdeaAsk::open("a?"),
                IdeaAsk::open("b?"),
                IdeaAsk::open("c?")
            ]
        );
        assert!(
            check.dropped.iter().any(|hint| hint.contains("Rückfragen")),
            "{:?}",
            check.dropped
        );
    }

    #[test]
    fn uebernimmt_rueckfragen_mit_antworten_empfehlung_und_mehrfachauswahl() {
        let check = checked(&answer(json!({ "questions": [
            {
                "text": "Welche Engine?",
                "options": [
                    { "label": "faster-whisper", "detail": "schnell auf CPU" },
                    { "label": "Vosk", "detail": null },
                ],
                "recommended": 0,
                "multiSelect": false,
            },
            {
                "text": "Welche Sprachen?",
                "options": [{ "label": "Deutsch" }, { "label": "Englisch" }],
                "recommended": null,
                "multiSelect": true,
            },
        ] })));

        assert!(check.dropped.is_empty(), "{:?}", check.dropped);
        let engine = &check.draft.questions[0];
        assert_eq!(engine.text, "Welche Engine?");
        assert_eq!(engine.options[0].label, "faster-whisper");
        assert_eq!(engine.options[0].detail.as_deref(), Some("schnell auf CPU"));
        assert_eq!(engine.options[1].detail, None);
        assert_eq!(engine.recommended, Some(0));
        assert!(!engine.multi_select);
        assert!(check.draft.questions[1].multi_select);
        assert_eq!(check.draft.questions[1].recommended, None);
    }

    #[test]
    fn biegt_unbrauchbare_antworten_zurecht() {
        let check = checked(&answer(json!({ "questions": [
            {
                "text": "Wohin?",
                "options": ["a", "b", "c", "d", "e", { "label": "x".repeat(60) }],
                "recommended": 9,
                "multiSelect": true,
            },
            { "text": "Nur eine?", "options": [{ "label": "ja" }], "recommended": 0, "multiSelect": true },
            { "text": "  ", "options": [] },
        ] })));

        let wohin = &check.draft.questions[0];
        assert_eq!(wohin.options.len(), MAX_OPTIONS);
        assert_eq!(wohin.recommended, None, "Index außerhalb fällt weg");
        assert!(wohin.multi_select);
        assert!(check
            .dropped
            .iter()
            .any(|hint| hint.contains("Antworten auf „Wohin?“")));
        assert!(check.dropped.iter().any(|hint| hint.contains("gekürzt")));
        let eine = &check.draft.questions[1];
        assert!(eine.options.is_empty(), "eine Möglichkeit ist keine Wahl");
        assert_eq!(eine.recommended, None);
        assert!(!eine.multi_select);
        assert_eq!(check.draft.questions.len(), 2, "die leere Frage fällt weg");
    }

    #[test]
    fn liest_alte_rueckfragen_als_text_aus_gespeicherten_entwuerfen() {
        let ask: IdeaAsk = serde_json::from_value(json!("Auch für SQL Server?")).unwrap();
        assert_eq!(ask, IdeaAsk::open("Auch für SQL Server?"));

        let neu = IdeaAsk {
            options: vec![IdeaAskOption {
                label: "ja".into(),
                detail: None,
            }],
            recommended: Some(0),
            multi_select: true,
            ..IdeaAsk::open("Frage?")
        };
        let rundreise: IdeaAsk =
            serde_json::from_value(serde_json::to_value(&neu).unwrap()).unwrap();
        assert_eq!(rundreise, neu);
    }

    #[test]
    fn kappt_confidence_ausserhalb_von_null_bis_eins() {
        assert_eq!(
            checked(&answer(json!({ "confidence": 7 })))
                .draft
                .confidence,
            1.0
        );
        assert_eq!(
            checked(&answer(json!({ "confidence": -2.5 })))
                .draft
                .confidence,
            0.0
        );
        let check = checked(&answer(json!({ "confidence": "viel" })));
        assert_eq!(check.draft.confidence, 0.0);
        assert!(
            check.dropped.iter().any(|hint| hint.contains("confidence")),
            "{:?}",
            check.dropped
        );
    }

    #[test]
    fn filtert_bezuege_und_dubletten_auf_vorhandene_ids() {
        let check = checked(&answer(json!({
            "related": [
                { "ticketId": "NXS-0001", "why": "a" },
                { "ticketId": "NXS-0001", "why": "doppelt" },
                { "ticketId": "NXS-9999", "why": "b" },
            ],
            "duplicates": [
                { "id": "7", "why": "schon notiert" },
                { "id": "AIH-0002", "why": "schon als Ticket" },
                { "id": "42", "why": "gibt es nicht" },
            ],
        })));

        assert_eq!(
            check
                .draft
                .related
                .iter()
                .map(|found| found.ticket_id.as_str())
                .collect::<Vec<_>>(),
            ["NXS-0001"]
        );
        assert_eq!(
            check
                .draft
                .duplicates
                .iter()
                .map(|found| found.id.as_str())
                .collect::<Vec<_>>(),
            ["7", "AIH-0002"]
        );
        assert_eq!(check.dropped.len(), 2, "{:?}", check.dropped);
    }

    #[test]
    fn meldet_eine_antwort_die_kein_entwurf_ist() {
        let validator = IdeaDraftValidator::new();
        for value in [json!([]), json!("nein"), json!({ "title": "  " })] {
            assert_eq!(
                validator
                    .check(&value, &context(), "Eingabe", 1)
                    .unwrap_err()
                    .code(),
                Some("analysis-failed")
            );
        }
        assert_eq!(
            validator
                .check(
                    &answer(json!({ "target": { "kind": "plan" } })),
                    &context(),
                    "Eingabe",
                    1
                )
                .unwrap_err()
                .code(),
            Some("analysis-failed")
        );
    }

    #[test]
    fn nimmt_ohne_board_angabe_das_aktive_board() {
        let check = checked(&answer(json!({ "target": { "kind": "ticket" } })));

        assert_eq!(check.draft.target.kind, IdeaKind::Ticket);
        assert_eq!(check.draft.target.board_id, 1);
        assert!(
            check.dropped.iter().any(|hint| hint.contains("Board 1")),
            "{:?}",
            check.dropped
        );
    }
}
