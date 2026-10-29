//! Notizen: Vorrat für spätere Tickets. Werden nie gelöscht, nur übernommen oder mit Grund
//! verworfen. Portiert aus `packages/core/src/board/NoteService.ts`.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::board::basics::{
    BoardActor, NoteState, Priority, TicketStatus, MAX_NOTE_LABELS_LENGTH, MAX_NOTE_REASON_LENGTH,
    MAX_NOTE_SEEN_IN_LENGTH, MAX_NOTE_TITLE_LENGTH,
};
use crate::board::database::{BoardDatabase, BoardUnitOfWork, NewNote, NoteFilter};
use crate::board::labels::BoardLabels;
use crate::board::limits::BoardLimits;
use crate::board::note_similarity::{note_title, NoteSimilarity};
use crate::board::ticket_service::{self, NewTicketInput, TicketView, WriteOptions};
use crate::board::types::Note;
use crate::clock::Clock;
use crate::error::{HarnessError, Result};
use crate::text::{utf16_len, utf16_prefix};
use crate::tx;

/// Inhalt einer neuen Notiz: ein Titel, der ohne Kontext verständlich ist, und optionale Details
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewNoteText {
    pub title: String,
    pub body: Option<String>,
}

impl NewNoteText {
    /// Eine Notiz, von der nur der Titel feststeht
    pub fn titled(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: None,
        }
    }
}

/// Woher eine neue Notiz kommt und was sie übergehen darf
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AddNoteOptions {
    pub source: Option<String>,
    pub labels: Option<Vec<String>>,
    pub force: bool,
}

/// Was aus einer Notiz für ein Ticket wird
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TakeNoteOptions {
    pub title: Option<String>,
    pub status: Option<TicketStatus>,
    pub priority: Option<Priority>,
    pub labels: Option<Vec<String>>,
}

/// Eine übernommene Notiz und das Ticket, das daraus entstand
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TakenNote {
    pub note: Note,
    pub ticket: TicketView,
}

/// Notizen eines Boards.
#[derive(Debug, Clone)]
pub struct NoteService<D: BoardDatabase> {
    db: Arc<D>,
    clock: Clock,
}

impl<D: BoardDatabase> NoteService<D> {
    /// Dienst auf dieser Datenbank mit dieser Uhr
    pub fn new(db: Arc<D>, clock: Clock) -> Self {
        Self { db, clock }
    }

    /// Legt eine offene Notiz an; `source` muss ein existierendes Ticket aus dem Projekt des
    /// Boards sein. Gibt es den Befund vermutlich schon (offen oder übernommen), wird abgelehnt
    /// – außer mit `force`.
    pub async fn add(
        &self,
        board_id: i32,
        text: &NewNoteText,
        actor: &BoardActor,
        options: &AddNoteOptions,
    ) -> Result<Note> {
        let title = text.title.trim().to_string();
        let body = text.body.as_deref().unwrap_or("").trim().to_string();
        if title.is_empty() {
            return Err(HarnessError::new(
                "note-empty",
                "Die Notiz braucht einen Titel: einen Satz, was auffällt.",
            )
            .into());
        }
        if title.contains('\n') {
            return Err(HarnessError::new(
                "note-title-multiline",
                "Der Titel der Notiz ist eine Zeile; Details gehören in den Inhalt (--details).",
            )
            .into());
        }
        let length = utf16_len(&title);
        if length > MAX_NOTE_TITLE_LENGTH {
            return Err(HarnessError::new(
                "value-too-long",
                format!("Der Titel der Notiz ist zu lang ({length} Zeichen, höchstens {MAX_NOTE_TITLE_LENGTH}). Ein kurzer Satz als Titel, der Rest in den Inhalt (--details)."),
            )
            .into());
        }
        let actor = actor.clone();
        let options = options.clone();
        let clock = self.clock.clone();
        tx!(self.db, [title, body, actor, options, clock], |u| {
            BoardLimits::actor(&actor)?;
            let labels = BoardLabels::normalize(options.labels.as_deref().unwrap_or_default())?;
            BoardLimits::text(
                &labels.join(","),
                MAX_NOTE_LABELS_LENGTH,
                "Die Label-Liste der Notiz",
            )?;
            let board = crate::board::board_service::BoardService::<D>::require_in(u, board_id).await?;
            let origin = match &options.source {
                Some(source) => Some(ticket_service::load(u, source, None, false).await?),
                None => None,
            };
            if let Some(origin) = &origin {
                if origin.project_id != board.project_id {
                    return Err(HarnessError::new(
                        "note-source-other-project",
                        format!("{} gehört zu einem anderen Projekt als Board {board_id}. Die Notiz gehört auf das Board des Tickets (--board {}).", origin.id, origin.board_id),
                    )
                    .into());
                }
            }
            if !options.force {
                let open = u
                    .notes()
                    .list(&NoteFilter {
                        board_id,
                        states: Some(vec![NoteState::Open, NoteState::Taken]),
                        source: None,
                    })
                    .await?;
                let known = NoteSimilarity::similar(&format!("{title}\n{body}"), &open);
                if !known.is_empty() {
                    let candidates: Vec<Note> = known.into_iter().take(3).collect();
                    return Err(duplicate(
                        &candidates,
                        origin.as_ref().map(|ticket| ticket.id.as_str()),
                    )
                    .into());
                }
            }
            u.notes()
                .insert(&NewNote {
                    board_id,
                    author_id: actor.user_id.clone(),
                    author_name: actor.name.clone(),
                    title: title.clone(),
                    body: body.clone(),
                    source: origin.as_ref().map(|ticket| ticket.id.clone()),
                    labels,
                    seen_count: 1,
                    seen_in: Vec::new(),
                    state: NoteState::Open,
                    ticket_id: None,
                    reason: None,
                    created_at: clock.iso(),
                })
                .await
        })
        .await
    }

    /// Offene und übernommene Notizen des Boards, die denselben Befund beschreiben wie der Text
    /// (ähnlichste zuerst)
    pub async fn similar(&self, board_id: i32, text: &str) -> Result<Vec<Note>> {
        let text = text.to_string();
        tx!(self.db, [text], |u| {
            let open = u
                .notes()
                .list(&NoteFilter {
                    board_id,
                    states: Some(vec![NoteState::Open, NoteState::Taken]),
                    source: None,
                })
                .await?;
            Ok(NoteSimilarity::similar(&text, &open))
        })
        .await
    }

    /// Der Befund einer offenen Notiz ist erneut aufgefallen: zählt hoch und merkt sich das
    /// Ticket
    pub async fn seen(&self, id: i32, source: Option<&str>) -> Result<Note> {
        let source = source.map(str::to_string);
        tx!(self.db, [source], |u| {
            let note = load_note(u, id, true).await?;
            if note.state == NoteState::Taken {
                return Err(HarnessError::new(
                    "note-taken",
                    format!(
                        "Notiz {id} ist schon als {} übernommen – dort kommentieren.",
                        note.ticket_id.as_deref().unwrap_or("")
                    ),
                )
                .into());
            }
            if note.state == NoteState::Dropped {
                return Err(HarnessError::new(
                    "note-not-open",
                    format!(
                        "Notiz {id} ist verworfen ({}).",
                        note.reason.as_deref().unwrap_or("")
                    ),
                )
                .into());
            }
            let board =
                crate::board::board_service::BoardService::<D>::require_in(u, note.board_id)
                    .await?;
            let origin = match &source {
                Some(source) => Some(ticket_service::load(u, source, None, false).await?),
                None => None,
            };
            if let Some(origin) = &origin {
                if origin.project_id != board.project_id {
                    return Err(HarnessError::new(
                        "note-source-other-project",
                        format!(
                            "{} gehört zu einem anderen Projekt als Notiz {id}.",
                            origin.id
                        ),
                    )
                    .into());
                }
            }
            let mut seen_in = note.seen_in.clone();
            if let Some(origin) = &origin {
                if Some(&origin.id) != note.source.as_ref() && !seen_in.contains(&origin.id) {
                    seen_in.push(origin.id.clone());
                }
            }
            let updated = Note {
                seen_count: note.seen_count + 1,
                // Passt die Liste nicht mehr in die Spalte, zählt nur noch die Zahl
                seen_in: if utf16_len(&seen_in.join(",")) <= MAX_NOTE_SEEN_IN_LENGTH {
                    seen_in
                } else {
                    note.seen_in.clone()
                },
                ..note
            };
            u.notes().update(&updated).await?;
            Ok(updated)
        })
        .await
    }

    /// Notizen eines Boards; ohne Angabe nur offene
    pub async fn list(
        &self,
        board_id: i32,
        states: Option<&[NoteState]>,
        source: Option<&str>,
    ) -> Result<Vec<Note>> {
        let filter = NoteFilter {
            board_id,
            states: Some(states.map_or_else(|| vec![NoteState::Open], <[NoteState]>::to_vec)),
            source: source.map(str::to_string),
        };
        tx!(self.db, [filter], |u| { u.notes().list(&filter).await }).await
    }

    /// Eine Notiz
    pub async fn get(&self, id: i32) -> Result<Note> {
        tx!(self.db, |u| { load_note(u, id, false).await }).await
    }

    /// Macht aus einer offenen Notiz ein Ticket auf demselben Board
    pub async fn take(
        &self,
        id: i32,
        actor: &BoardActor,
        options: &TakeNoteOptions,
    ) -> Result<TakenNote> {
        let actor = actor.clone();
        let options = options.clone();
        let clock = self.clock.clone();
        tx!(self.db, [actor, options, clock], |u| {
            let note = load_note(u, id, true).await?;
            if note.state != NoteState::Open {
                let detail = if note.state == NoteState::Taken {
                    format!(
                        "schon als {} übernommen",
                        note.ticket_id.as_deref().unwrap_or("")
                    )
                } else {
                    "verworfen".to_string()
                };
                return Err(HarnessError::new(
                    "note-not-open",
                    format!("Notiz {id} ist {detail}."),
                )
                .into());
            }
            let ticket = ticket_service::create_in(
                u,
                note.board_id,
                &NewTicketInput {
                    title: options.title.clone().unwrap_or_else(|| {
                        utf16_prefix(&note_title(&note), MAX_NOTE_TITLE_LENGTH).to_string()
                    }),
                    description: Some(description(&note)),
                    status: Some(options.status.unwrap_or(TicketStatus::Backlog)),
                    priority: Some(options.priority.unwrap_or(Priority::Normal)),
                    labels: Some(
                        options
                            .labels
                            .clone()
                            .unwrap_or_else(|| note.labels.clone()),
                    ),
                    ..NewTicketInput::default()
                },
                &actor,
                &WriteOptions::default(),
                &clock,
            )
            .await?;
            let taken = Note {
                state: NoteState::Taken,
                ticket_id: Some(ticket.id.clone()),
                ..note
            };
            u.notes().update(&taken).await?;
            Ok(TakenNote {
                note: taken,
                ticket: ticket_service::view(u, ticket).await?,
            })
        })
        .await
    }

    /// Verwirft eine Notiz mit Grund; übernommene Notizen lassen sich nicht verwerfen
    pub async fn drop_note(&self, id: i32, reason: &str) -> Result<Note> {
        let why = reason.trim().to_string();
        if why.is_empty() {
            return Err(HarnessError::new(
                "reason-missing",
                "Zum Verwerfen gehört ein Grund (--reason).",
            )
            .into());
        }
        let length = utf16_len(&why);
        if length > MAX_NOTE_REASON_LENGTH {
            return Err(HarnessError::new(
                "value-too-long",
                format!(
                    "Der Grund ist zu lang ({length} Zeichen, höchstens {MAX_NOTE_REASON_LENGTH})."
                ),
            )
            .into());
        }
        tx!(self.db, [why], |u| {
            let note = load_note(u, id, true).await?;
            if note.state == NoteState::Taken {
                return Err(HarnessError::new(
                    "note-taken",
                    format!(
                        "Notiz {id} ist schon als {} übernommen.",
                        note.ticket_id.as_deref().unwrap_or("")
                    ),
                )
                .into());
            }
            let dropped = Note {
                state: NoteState::Dropped,
                reason: Some(why.clone()),
                ticket_id: None,
                ..note
            };
            u.notes().update(&dropped).await?;
            Ok(dropped)
        })
        .await
    }
}

/// Beschreibung des Tickets aus einer Notiz: Inhalt (bei älteren Notizen der ganze Text) und
/// woher sie kommt
fn description(note: &Note) -> String {
    let origin = match &note.source {
        Some(source) => format!(", aufgefallen bei {source}"),
        None => String::new(),
    };
    let again = if note.seen_count > 1 {
        let also = if note.seen_in.is_empty() {
            String::new()
        } else {
            format!(", auch bei {}", note.seen_in.join(", "))
        };
        format!("; insgesamt {}× aufgefallen{also}", note.seen_count)
    } else {
        String::new()
    };
    let footer = format!("(Notiz vom {}{origin}{again})", note.created_at);
    if note.body.is_empty() {
        footer
    } else {
        format!("{}\n\n{footer}", note.body)
    }
}

/// Ablehnung „gibt es schon“ mit den Kandidaten und dem Weg, sie zu zählen
fn duplicate(known: &[Note], source: Option<&str>) -> HarnessError {
    let mut lines: Vec<String> = vec!["Diesen Befund gibt es vermutlich schon:".into()];
    for note in known {
        let state = match note.state {
            NoteState::Taken => format!(
                "[übernommen als {}]",
                note.ticket_id.as_deref().unwrap_or("")
            ),
            _ => "[offen]".to_string(),
        };
        lines.push(format!("  {}  {state} {}", note.id, note_title(note)));
    }
    let open = known.iter().find(|note| note.state == NoteState::Open);
    let taken = known.iter().find(|note| note.state == NoteState::Taken);
    if let Some(open) = open {
        let from = match source {
            Some(source) => format!(" --from {source}"),
            None => String::new(),
        };
        lines.push(format!(
            "Derselbe Befund: harness board note seen {}{from}",
            open.id
        ));
    }
    if let (Some(taken), None) = (taken, open) {
        lines.push(format!(
            "Schon als Ticket übernommen: an {} kommentieren statt neu notieren.",
            taken.ticket_id.as_deref().unwrap_or("")
        ));
    }
    lines.push("Wirklich etwas anderes: mit --force anlegen.".into());
    HarnessError::new("note-similar", lines.join("\n")).with_details(json!({
        "notes": known
            .iter()
            .map(|note| json!({
                "id": note.id,
                "title": note_title(note),
                "state": note.state,
                "ticketId": note.ticket_id,
            }))
            .collect::<Vec<_>>(),
    }))
}

/// Notiz zur ID oder Fehler; `for_update` sperrt sie bis zum Ende der Transaktion
async fn load_note(u: &mut dyn BoardUnitOfWork, id: i32, for_update: bool) -> Result<Note> {
    u.notes().by_id(id, for_update).await?.ok_or_else(|| {
        HarnessError::new("note-not-found", format!("Notiz {id} gibt es nicht.")).into()
    })
}

#[cfg(test)]
mod tests;
