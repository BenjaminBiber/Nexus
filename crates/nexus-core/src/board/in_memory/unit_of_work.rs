//! Die Unit of Work der Speicher-Datenbank: alle 17 Repository-Traits auf einem Struct über
//! dem Zustand der laufenden Transaktion. Portiert aus `unitOfWork` in
//! `packages/core/src/board/InMemoryBoardDatabase.ts`.
//!
//! Jede schreibende Methode merkt sich die berührten Boards und – bei Fragen – die berührten
//! Projekte; daraus steigen nach einer erfolgreichen Transaktion `change_marker` und
//! `question_marker`. Gelesene Werte werden immer geklont: Aufrufer dürfen den Zustand der
//! Datenbank nicht verändern (in TypeScript macht das `structuredClone`).

use std::collections::BTreeMap;

use async_trait::async_trait;

use crate::board::basics::TicketStatus;
use crate::board::database::{
    BoardFilter, BoardQuestionCommentRepository, BoardQuestionFilter,
    BoardQuestionHistoryRepository, BoardQuestionRepository, BoardQuestionResearchRepository,
    BoardRepository, BoardSearchRepository, BoardUnitOfWork, CommentRepository, DeletionRepository,
    DispatchRunRepository, DispatchStatsRepository, HistoryRepository, NewBoard, NewComment,
    NewNote, NewProject, NewQuestionComment, NewQuestionResearch, NoteFilter, NoteRepository,
    ProjectRepository, TicketFilter, TicketRepository, TicketSessionRepository,
    TicketStepRepository, UserRepository,
};
use crate::board::dispatch_run_types::{
    DispatchCommentCounts, DispatchConfigVersion, DispatchRun, DispatchRunFilter, DispatchRunSource,
};
use crate::board::question_types::{
    BoardQuestion, BoardQuestionComment, BoardQuestionHistoryEntry, BoardQuestionResearch,
    BoardQuestionResearchSummary,
};
use crate::board::search_types::{SearchRequest, SearchRows};
use crate::board::types::{
    Board, BoardUser, Comment, Deletion, HistoryEntry, Note, Project, StepProgress, Ticket,
    TicketStep,
};
use crate::dispatch::markers::{ABORT, DISPATCH_PREFIX, START};
use crate::error::{Error, HarnessError, Result};

use super::search::InMemoryBoardSearch;
use super::state::{State, TicketSession, TicketSteps, Touched};

/// Alle Repositories einer laufenden Transaktion über dem Entwurf des Zustands
pub(crate) struct InMemoryUnitOfWork<'a> {
    pub state: &'a mut State,
    pub touched: &'a mut Touched,
}

impl InMemoryUnitOfWork<'_> {
    /// Merkt sich ein berührtes Board, falls es eines gibt
    fn touch(&mut self, board_id: Option<i32>) {
        if let Some(id) = board_id {
            self.touched.boards.insert(id);
        }
    }

    /// Merkt sich das Board eines Tickets
    fn touch_ticket(&mut self, ticket_id: &str) {
        let board_id = self.state.board_of(ticket_id);
        self.touch(board_id);
    }

    /// Eine geänderte Frage berührt ihr Projekt und die Boards der betroffenen Tickets
    fn touch_question(&mut self, question: &BoardQuestion) {
        self.touched.projects.insert(question.project_id);
        let boards: Vec<i32> = question
            .ticket_ids
            .iter()
            .filter_map(|id| self.state.board_of(id))
            .collect();
        for board_id in boards {
            self.touched.boards.insert(board_id);
        }
    }

    /// Das Projekt mit dieser Kennung; es muss es geben (in TypeScript ein `!`)
    fn project_mut(&mut self, id: i32) -> Result<&mut Project> {
        self.state
            .projects
            .iter_mut()
            .find(|project| project.id == id)
            .ok_or_else(|| Error::unexpected_message(format!("Projekt {id} gibt es nicht.")))
    }
}

// ---------------------------------------------------------------------------------------------
// Benutzer, Projekte, Boards
// ---------------------------------------------------------------------------------------------

#[async_trait]
impl UserRepository for InMemoryUnitOfWork<'_> {
    async fn upsert(&mut self, user: &BoardUser) -> Result<()> {
        match self.state.users.iter_mut().find(|u| u.id == user.id) {
            Some(found) => found.name = user.name.clone(),
            None => self.state.users.push(user.clone()),
        }
        Ok(())
    }

    async fn by_id(&mut self, id: &str) -> Result<Option<BoardUser>> {
        Ok(self.state.users.iter().find(|u| u.id == id).cloned())
    }

    async fn list(&mut self) -> Result<Vec<BoardUser>> {
        Ok(self.state.users.clone())
    }
}

#[async_trait]
impl ProjectRepository for InMemoryUnitOfWork<'_> {
    async fn list(&mut self) -> Result<Vec<Project>> {
        let mut projects = self.state.projects.clone();
        projects.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(projects)
    }

    async fn by_id(&mut self, id: i32) -> Result<Option<Project>> {
        Ok(self.state.projects.iter().find(|p| p.id == id).cloned())
    }

    async fn by_key(&mut self, key: &str) -> Result<Option<Project>> {
        Ok(self.state.projects.iter().find(|p| p.key == key).cloned())
    }

    async fn insert(&mut self, project: &NewProject) -> Result<Project> {
        if self.state.projects.iter().any(|p| p.key == project.key) {
            return Err(HarnessError::new(
                "project-exists",
                format!("Projekt {} gibt es schon.", project.key),
            )
            .into());
        }
        let created = Project {
            id: self.state.next_id() as i32,
            key: project.key.clone(),
            name: project.name.clone(),
            next_ticket_number: 1,
            next_question_number: 1,
            created_at: project.created_at.clone(),
            created_by: project.created_by.clone(),
        };
        self.state.projects.push(created.clone());
        Ok(created)
    }

    async fn rename(&mut self, id: i32, name: &str) -> Result<()> {
        if let Some(project) = self.state.projects.iter_mut().find(|p| p.id == id) {
            project.name = name.to_string();
        }
        Ok(())
    }

    async fn take_ticket_number(&mut self, id: i32) -> Result<i32> {
        let project = self.project_mut(id)?;
        let number = project.next_ticket_number;
        project.next_ticket_number += 1;
        Ok(number)
    }

    async fn take_question_number(&mut self, id: i32) -> Result<i32> {
        let project = self.project_mut(id)?;
        let number = project.next_question_number;
        project.next_question_number += 1;
        Ok(number)
    }
}

#[async_trait]
impl BoardRepository for InMemoryUnitOfWork<'_> {
    async fn list(&mut self, filter: &BoardFilter) -> Result<Vec<Board>> {
        Ok(self
            .state
            .boards
            .iter()
            .filter(|board| filter.project_id.is_none_or(|id| board.project_id == id))
            .filter(|board| {
                filter
                    .owner_id
                    .as_ref()
                    .is_none_or(|owner| &board.owner_id == owner)
            })
            .cloned()
            .collect())
    }

    async fn by_id(&mut self, id: i32) -> Result<Option<Board>> {
        Ok(self.state.boards.iter().find(|b| b.id == id).cloned())
    }

    async fn insert(&mut self, board: &NewBoard) -> Result<Board> {
        let created = Board {
            id: self.state.next_id() as i32,
            project_id: board.project_id,
            owner_id: board.owner_id.clone(),
            name: board.name.clone(),
            repo_path: board.repo_path.clone(),
            created_at: board.created_at.clone(),
        };
        self.state.boards.push(created.clone());
        self.touch(Some(created.id));
        Ok(created)
    }

    async fn update(&mut self, board: &Board) -> Result<()> {
        if let Some(found) = self.state.boards.iter_mut().find(|b| b.id == board.id) {
            *found = board.clone();
        }
        self.touch(Some(board.id));
        Ok(())
    }

    async fn delete(&mut self, id: i32) -> Result<()> {
        self.state.boards.retain(|board| board.id != id);
        self.state.notes.retain(|note| note.board_id != id);
        self.touch(Some(id));
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Tickets, Kommentare, Schritte, Sessions, Verlauf
// ---------------------------------------------------------------------------------------------

#[async_trait]
impl TicketRepository for InMemoryUnitOfWork<'_> {
    async fn by_id(&mut self, id: &str, _for_update: bool) -> Result<Option<Ticket>> {
        // `for_update` entfällt: Transaktionen laufen hier ohnehin nacheinander
        Ok(self.state.tickets.iter().find(|t| t.id == id).cloned())
    }

    async fn list(&mut self, filter: &TicketFilter) -> Result<Vec<Ticket>> {
        // `without_texts` wird bewusst ignoriert: im Speicher kostet das Laden der Texte nichts,
        // und der Vertrag verlangt nur, dass leere Texte leer bleiben
        Ok(self
            .state
            .tickets
            .iter()
            .filter(|ticket| {
                filter
                    .board_ids
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&ticket.board_id))
            })
            .filter(|ticket| {
                filter
                    .ids
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&ticket.id))
            })
            .cloned()
            .collect())
    }

    async fn insert(&mut self, ticket: &Ticket) -> Result<()> {
        self.state.tickets.push(ticket.clone());
        self.touch(Some(ticket.board_id));
        Ok(())
    }

    async fn update(&mut self, ticket: &Ticket) -> Result<()> {
        let position = self
            .state
            .tickets
            .iter()
            .position(|t| t.id == ticket.id)
            .filter(|index| self.state.tickets[*index].rev == ticket.rev - 1)
            .ok_or_else(|| {
                HarnessError::new(
                    "conflict",
                    format!(
                        "Ticket {} wurde inzwischen geändert. Bitte neu lesen.",
                        ticket.id
                    ),
                )
            })?;
        let previous_board = self.state.tickets[position].board_id;
        self.touch(Some(previous_board));
        self.touch(Some(ticket.board_id));
        self.state.tickets[position] = ticket.clone();
        Ok(())
    }

    /// Wie `FK_QuestionTickets_Tickets` in SQL (ohne Kaskade): ein Ticket mit verknüpften
    /// Fragen ist nicht löschbar
    async fn delete(&mut self, id: &str) -> Result<()> {
        if self
            .state
            .questions
            .iter()
            .any(|question| question.ticket_ids.iter().any(|t| t == id))
        {
            return Err(HarnessError::new(
                "conflict",
                format!("Ticket {id} ist noch mit Fragen verknüpft."),
            )
            .into());
        }
        self.touch_ticket(id);
        self.state.tickets.retain(|ticket| ticket.id != id);
        self.state
            .comments
            .retain(|comment| comment.ticket_id != id);
        self.state.history.retain(|entry| entry.ticket_id != id);
        self.state.steps.retain(|steps| steps.ticket_id != id);
        self.state
            .ticket_sessions
            .retain(|session| session.ticket_id != id);
        Ok(())
    }

    async fn dependents(&mut self, id: &str) -> Result<Vec<String>> {
        let mut ids: Vec<String> = self
            .state
            .tickets
            .iter()
            .filter(|ticket| ticket.depends_on.iter().any(|d| d == id))
            .map(|ticket| ticket.id.clone())
            .collect();
        ids.sort();
        Ok(ids)
    }

    async fn remove_dependency_on(&mut self, id: &str) -> Result<Vec<String>> {
        let mut changed: Vec<String> = Vec::new();
        let mut boards: Vec<i32> = Vec::new();
        for ticket in &mut self.state.tickets {
            if ticket.depends_on.iter().any(|d| d == id) {
                ticket.depends_on.retain(|d| d != id);
                changed.push(ticket.id.clone());
                boards.push(ticket.board_id);
            }
        }
        for board_id in boards {
            self.touched.boards.insert(board_id);
        }
        changed.sort();
        Ok(changed)
    }

    async fn max_sort_order(&mut self, board_id: i32, status: TicketStatus) -> Result<Option<i32>> {
        Ok(self
            .state
            .tickets
            .iter()
            .filter(|ticket| ticket.board_id == board_id && ticket.status == status)
            .map(|ticket| ticket.sort_order)
            .max())
    }

    async fn open_count_by_board(&mut self, board_ids: &[i32]) -> Result<BTreeMap<i32, i64>> {
        let mut counts: BTreeMap<i32, i64> = BTreeMap::new();
        for ticket in &self.state.tickets {
            let offen = !matches!(ticket.status, TicketStatus::Done | TicketStatus::NotNeeded);
            if board_ids.contains(&ticket.board_id) && offen {
                *counts.entry(ticket.board_id).or_insert(0) += 1;
            }
        }
        Ok(counts)
    }
}

#[async_trait]
impl CommentRepository for InMemoryUnitOfWork<'_> {
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<Comment>> {
        Ok(self
            .state
            .comments
            .iter()
            .filter(|comment| comment.ticket_id == ticket_id)
            .cloned()
            .collect())
    }

    async fn insert(&mut self, comment: &NewComment) -> Result<Comment> {
        let created = Comment {
            id: self.state.next_id(),
            ticket_id: comment.ticket_id.clone(),
            author_id: comment.author_id.clone(),
            author_name: comment.author_name.clone(),
            body: comment.body.clone(),
            created_at: comment.created_at.clone(),
            read_by_agent_at: comment.read_by_agent_at.clone(),
        };
        self.state.comments.push(created.clone());
        self.touch_ticket(&created.ticket_id);
        Ok(created)
    }

    async fn count_by_board(&mut self, board_id: i32) -> Result<BTreeMap<String, i64>> {
        let ids: Vec<&str> = self
            .state
            .tickets
            .iter()
            .filter(|ticket| ticket.board_id == board_id)
            .map(|ticket| ticket.id.as_str())
            .collect();
        let mut counts: BTreeMap<String, i64> = BTreeMap::new();
        for comment in &self.state.comments {
            if ids.contains(&comment.ticket_id.as_str()) {
                *counts.entry(comment.ticket_id.clone()).or_insert(0) += 1;
            }
        }
        Ok(counts)
    }

    async fn unread_by_agent(&mut self, ticket_ids: &[String]) -> Result<Vec<Comment>> {
        let mut comments: Vec<Comment> = self
            .state
            .comments
            .iter()
            .filter(|comment| {
                ticket_ids.contains(&comment.ticket_id) && comment.read_by_agent_at.is_none()
            })
            .cloned()
            .collect();
        comments.sort_by_key(|comment| comment.id);
        Ok(comments)
    }

    async fn mark_read_by_agent(&mut self, ids: &[i64], at: &str) -> Result<()> {
        let mut tickets: Vec<String> = Vec::new();
        for comment in &mut self.state.comments {
            if ids.contains(&comment.id) && comment.read_by_agent_at.is_none() {
                comment.read_by_agent_at = Some(at.to_string());
                tickets.push(comment.ticket_id.clone());
            }
        }
        for ticket_id in tickets {
            self.touch_ticket(&ticket_id);
        }
        Ok(())
    }
}

#[async_trait]
impl TicketStepRepository for InMemoryUnitOfWork<'_> {
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<TicketStep>> {
        Ok(self
            .state
            .steps
            .iter()
            .find(|entry| entry.ticket_id == ticket_id)
            .map(|entry| entry.steps.clone())
            .unwrap_or_default())
    }

    async fn replace(&mut self, ticket_id: &str, steps: &[TicketStep]) -> Result<()> {
        self.state
            .steps
            .retain(|entry| entry.ticket_id != ticket_id);
        if !steps.is_empty() {
            self.state.steps.push(TicketSteps {
                ticket_id: ticket_id.to_string(),
                steps: steps.to_vec(),
            });
        }
        self.touch_ticket(ticket_id);
        Ok(())
    }

    async fn set_done(&mut self, ticket_id: &str, position: i32, done: bool) -> Result<bool> {
        let step = usize::try_from(position).ok().and_then(|index| {
            self.state
                .steps
                .iter_mut()
                .find(|entry| entry.ticket_id == ticket_id)
                .and_then(|entry| entry.steps.get_mut(index))
        });
        let Some(step) = step else {
            return Ok(false);
        };
        step.done = done;
        self.touch_ticket(ticket_id);
        Ok(true)
    }

    async fn progress(&mut self, ticket_ids: &[String]) -> Result<BTreeMap<String, StepProgress>> {
        let mut progress: BTreeMap<String, StepProgress> = BTreeMap::new();
        for entry in &self.state.steps {
            if ticket_ids.contains(&entry.ticket_id) {
                progress.insert(
                    entry.ticket_id.clone(),
                    StepProgress {
                        done: entry.steps.iter().filter(|step| step.done).count() as i32,
                        total: entry.steps.len() as i32,
                    },
                );
            }
        }
        Ok(progress)
    }
}

#[async_trait]
impl TicketSessionRepository for InMemoryUnitOfWork<'_> {
    async fn add(&mut self, ticket_id: &str, session_id: &str, at: &str) -> Result<bool> {
        if self
            .state
            .ticket_sessions
            .iter()
            .any(|entry| entry.ticket_id == ticket_id && entry.session_id == session_id)
        {
            return Ok(false);
        }
        self.state.ticket_sessions.push(TicketSession {
            ticket_id: ticket_id.to_string(),
            session_id: session_id.to_string(),
            added_at: at.to_string(),
        });
        self.touch_ticket(ticket_id);
        Ok(true)
    }

    async fn by_tickets(&mut self, ticket_ids: &[String]) -> Result<BTreeMap<String, Vec<String>>> {
        let mut sorted: Vec<&TicketSession> = self.state.ticket_sessions.iter().collect();
        sorted.sort_by(|a, b| {
            a.added_at
                .cmp(&b.added_at)
                .then_with(|| a.session_id.cmp(&b.session_id))
        });
        let mut result: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for entry in sorted {
            if ticket_ids.contains(&entry.ticket_id) {
                result
                    .entry(entry.ticket_id.clone())
                    .or_default()
                    .push(entry.session_id.clone());
            }
        }
        Ok(result)
    }

    async fn tickets_of(&mut self, session_id: &str) -> Result<Vec<String>> {
        let mut ids: Vec<String> = self
            .state
            .ticket_sessions
            .iter()
            .filter(|entry| entry.session_id == session_id)
            .map(|entry| entry.ticket_id.clone())
            .collect();
        ids.sort();
        ids.dedup();
        Ok(ids)
    }
}

#[async_trait]
impl HistoryRepository for InMemoryUnitOfWork<'_> {
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<HistoryEntry>> {
        Ok(self
            .state
            .history
            .iter()
            .filter(|entry| entry.ticket_id == ticket_id)
            .cloned()
            .collect())
    }

    async fn insert(&mut self, entries: &[HistoryEntry]) -> Result<()> {
        self.state.history.extend(entries.iter().cloned());
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Notizen und Papierkorb
// ---------------------------------------------------------------------------------------------

#[async_trait]
impl NoteRepository for InMemoryUnitOfWork<'_> {
    async fn list(&mut self, filter: &NoteFilter) -> Result<Vec<Note>> {
        Ok(self
            .state
            .notes
            .iter()
            .filter(|note| note.board_id == filter.board_id)
            .filter(|note| {
                filter
                    .states
                    .as_ref()
                    .is_none_or(|states| states.contains(&note.state))
            })
            .filter(|note| {
                filter
                    .source
                    .as_ref()
                    .is_none_or(|source| note.source.as_ref() == Some(source))
            })
            .cloned()
            .collect())
    }

    async fn by_id(&mut self, id: i32, _for_update: bool) -> Result<Option<Note>> {
        // `for_update` entfällt: Transaktionen laufen hier ohnehin nacheinander
        Ok(self.state.notes.iter().find(|note| note.id == id).cloned())
    }

    async fn insert(&mut self, note: &NewNote) -> Result<Note> {
        let created = Note {
            id: self.state.next_id() as i32,
            board_id: note.board_id,
            author_id: note.author_id.clone(),
            author_name: note.author_name.clone(),
            title: note.title.clone(),
            body: note.body.clone(),
            source: note.source.clone(),
            labels: note.labels.clone(),
            seen_count: note.seen_count,
            seen_in: note.seen_in.clone(),
            state: note.state,
            ticket_id: note.ticket_id.clone(),
            reason: note.reason.clone(),
            created_at: note.created_at.clone(),
        };
        self.state.notes.push(created.clone());
        self.touch(Some(created.board_id));
        Ok(created)
    }

    async fn update(&mut self, note: &Note) -> Result<()> {
        if let Some(found) = self.state.notes.iter_mut().find(|n| n.id == note.id) {
            *found = note.clone();
        }
        self.touch(Some(note.board_id));
        Ok(())
    }
}

#[async_trait]
impl DeletionRepository for InMemoryUnitOfWork<'_> {
    async fn insert(&mut self, deletion: &Deletion) -> Result<()> {
        self.state.deletions.push(deletion.clone());
        Ok(())
    }

    async fn list(&mut self) -> Result<Vec<Deletion>> {
        Ok(self.state.deletions.clone())
    }
}

// ---------------------------------------------------------------------------------------------
// Fragen
// ---------------------------------------------------------------------------------------------

#[async_trait]
impl BoardQuestionRepository for InMemoryUnitOfWork<'_> {
    async fn by_id(&mut self, id: &str, _for_update: bool) -> Result<Option<BoardQuestion>> {
        // `for_update` entfällt: Transaktionen laufen hier ohnehin nacheinander
        Ok(self
            .state
            .questions
            .iter()
            .find(|question| question.id == id)
            .cloned())
    }

    async fn list(&mut self, filter: &BoardQuestionFilter) -> Result<Vec<BoardQuestion>> {
        let mut questions: Vec<BoardQuestion> = self
            .state
            .questions
            .iter()
            .filter(|question| filter.project_id.is_none_or(|id| question.project_id == id))
            .filter(|question| {
                filter
                    .ids
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&question.id))
            })
            .filter(|question| {
                filter
                    .states
                    .as_ref()
                    .is_none_or(|states| states.contains(&question.state))
            })
            .filter(|question| {
                filter.ticket_ids.as_ref().is_none_or(|ids| {
                    question
                        .ticket_ids
                        .iter()
                        .any(|ticket_id| ids.contains(ticket_id))
                })
            })
            .cloned()
            .collect();
        questions.sort_by(|a, b| {
            a.project_id
                .cmp(&b.project_id)
                .then_with(|| a.number.cmp(&b.number))
        });
        Ok(questions)
    }

    async fn insert(&mut self, question: &BoardQuestion) -> Result<()> {
        if self.state.questions.iter().any(|q| q.id == question.id) {
            return Err(HarnessError::new(
                "conflict",
                format!("Frage {} gibt es schon.", question.id),
            )
            .into());
        }
        let mut stored = question.clone();
        stored.ticket_ids.sort();
        self.state.questions.push(stored);
        self.touch_question(question);
        Ok(())
    }

    async fn update(&mut self, question: &BoardQuestion) -> Result<()> {
        let position = self
            .state
            .questions
            .iter()
            .position(|q| q.id == question.id)
            .filter(|index| self.state.questions[*index].rev == question.rev - 1)
            .ok_or_else(|| {
                HarnessError::new(
                    "conflict",
                    format!(
                        "Frage {} wurde inzwischen geändert. Bitte neu lesen.",
                        question.id
                    ),
                )
            })?;
        let previous = self.state.questions[position].clone();
        self.touch_question(&previous);
        self.touch_question(question);
        self.state.questions[position] = question.clone();
        Ok(())
    }

    async fn ids_referencing_ticket(&mut self, ticket_id: &str) -> Result<Vec<String>> {
        let mut ids: Vec<String> = self
            .state
            .questions
            .iter()
            .filter(|question| {
                question.ticket_ids.iter().any(|id| id == ticket_id)
                    || question.depends_on.iter().any(|id| id == ticket_id)
            })
            .map(|question| question.id.clone())
            .collect();
        ids.sort();
        Ok(ids)
    }

    async fn remove_ticket(&mut self, ticket_id: &str) -> Result<Vec<String>> {
        let betroffen: Vec<BoardQuestion> = self
            .state
            .questions
            .iter()
            .filter(|question| {
                question.ticket_ids.iter().any(|id| id == ticket_id)
                    || question.depends_on.iter().any(|id| id == ticket_id)
            })
            .cloned()
            .collect();
        for question in &betroffen {
            self.touch_question(question);
        }
        for question in &mut self.state.questions {
            if betroffen.iter().any(|q| q.id == question.id) {
                question.ticket_ids.retain(|id| id != ticket_id);
                question.depends_on.retain(|id| id != ticket_id);
            }
        }
        let mut changed: Vec<String> = betroffen.into_iter().map(|question| question.id).collect();
        changed.sort();
        Ok(changed)
    }
}

#[async_trait]
impl BoardQuestionResearchRepository for InMemoryUnitOfWork<'_> {
    async fn list(&mut self, question_id: &str) -> Result<Vec<BoardQuestionResearch>> {
        Ok(self
            .state
            .question_research
            .iter()
            .filter(|research| research.question_id == question_id)
            .cloned()
            .collect())
    }

    async fn insert(&mut self, research: &NewQuestionResearch) -> Result<BoardQuestionResearch> {
        let created = BoardQuestionResearch {
            id: self.state.next_id(),
            question_id: research.question_id.clone(),
            at: research.at.clone(),
            by_id: research.by_id.clone(),
            by_name: research.by_name.clone(),
            kind: research.kind,
            context: research.context.clone(),
            options: research.options.clone(),
        };
        self.state.question_research.push(created.clone());
        Ok(created)
    }

    async fn summaries(
        &mut self,
        question_ids: &[String],
    ) -> Result<BTreeMap<String, BoardQuestionResearchSummary>> {
        let mut summaries: BTreeMap<String, BoardQuestionResearchSummary> = BTreeMap::new();
        // In Reihenfolge des Anlegens: die letzte Recherche je Frage bestimmt die Zahl der Optionen
        for research in &self.state.question_research {
            if !question_ids.contains(&research.question_id) {
                continue;
            }
            let count = summaries
                .get(&research.question_id)
                .map_or(0, |summary| summary.count);
            summaries.insert(
                research.question_id.clone(),
                BoardQuestionResearchSummary {
                    count: count + 1,
                    option_count: research.options.len() as i32,
                },
            );
        }
        Ok(summaries)
    }
}

#[async_trait]
impl BoardQuestionCommentRepository for InMemoryUnitOfWork<'_> {
    async fn list(&mut self, question_id: &str) -> Result<Vec<BoardQuestionComment>> {
        Ok(self
            .state
            .question_comments
            .iter()
            .filter(|comment| comment.question_id == question_id)
            .cloned()
            .collect())
    }

    async fn insert(&mut self, comment: &NewQuestionComment) -> Result<BoardQuestionComment> {
        let created = BoardQuestionComment {
            id: self.state.next_id(),
            question_id: comment.question_id.clone(),
            author_id: comment.author_id.clone(),
            author_name: comment.author_name.clone(),
            author_kind: comment.author_kind,
            body: comment.body.clone(),
            created_at: comment.created_at.clone(),
        };
        self.state.question_comments.push(created.clone());
        Ok(created)
    }
}

#[async_trait]
impl BoardQuestionHistoryRepository for InMemoryUnitOfWork<'_> {
    async fn list(&mut self, question_id: &str) -> Result<Vec<BoardQuestionHistoryEntry>> {
        Ok(self
            .state
            .question_history
            .iter()
            .filter(|entry| entry.question_id == question_id)
            .cloned()
            .collect())
    }

    async fn insert(&mut self, entries: &[BoardQuestionHistoryEntry]) -> Result<()> {
        self.state.question_history.extend(entries.iter().cloned());
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Suche
// ---------------------------------------------------------------------------------------------

#[async_trait]
impl BoardSearchRepository for InMemoryUnitOfWork<'_> {
    async fn find(&mut self, request: &SearchRequest) -> Result<SearchRows> {
        Ok(InMemoryBoardSearch::new().find(&self.state.searchable(), request))
    }
}

// ---------------------------------------------------------------------------------------------
// Dispatcher-Läufe und Auswertung
// ---------------------------------------------------------------------------------------------

#[async_trait]
impl DispatchRunRepository for InMemoryUnitOfWork<'_> {
    async fn insert(&mut self, run: &DispatchRun) -> Result<bool> {
        if self.state.dispatch_runs.iter().any(|r| r.id == run.id) {
            return Ok(false);
        }
        self.state.dispatch_runs.push(run.clone());
        Ok(true)
    }

    async fn list(&mut self, filter: &DispatchRunFilter) -> Result<Vec<DispatchRun>> {
        let mut runs: Vec<DispatchRun> = self
            .state
            .dispatch_runs
            .iter()
            .filter(|run| run.project_id == filter.project_id)
            .filter(|run| filter.board_id.is_none_or(|id| run.board_id == Some(id)))
            .filter(|run| run.started_at >= filter.from && run.started_at < filter.to)
            .cloned()
            .collect();
        sort_runs(&mut runs);
        Ok(runs)
    }

    async fn list_all(&mut self, from: &str, to: &str) -> Result<Vec<DispatchRun>> {
        let mut runs: Vec<DispatchRun> = self
            .state
            .dispatch_runs
            .iter()
            .filter(|run| run.started_at.as_str() >= from && run.started_at.as_str() < to)
            .cloned()
            .collect();
        sort_runs(&mut runs);
        Ok(runs)
    }

    async fn first_started(
        &mut self,
        project_id: i32,
        source: DispatchRunSource,
    ) -> Result<Option<String>> {
        Ok(self
            .state
            .dispatch_runs
            .iter()
            .filter(|run| run.project_id == project_id && run.source == source)
            .map(|run| run.started_at.clone())
            .min())
    }

    async fn existing(&mut self, ids: &[String]) -> Result<Vec<String>> {
        Ok(self
            .state
            .dispatch_runs
            .iter()
            .filter(|run| ids.contains(&run.id))
            .map(|run| run.id.clone())
            .collect())
    }

    async fn record_config_version(&mut self, version: &DispatchConfigVersion) -> Result<bool> {
        if self
            .state
            .dispatch_config_versions
            .iter()
            .any(|v| v.hash == version.hash)
        {
            return Ok(false);
        }
        self.state.dispatch_config_versions.push(version.clone());
        Ok(true)
    }

    async fn list_config_versions(
        &mut self,
        from: &str,
        to: &str,
    ) -> Result<Vec<DispatchConfigVersion>> {
        let mut versions: Vec<DispatchConfigVersion> = self
            .state
            .dispatch_config_versions
            .iter()
            .filter(|version| {
                version.first_seen_at.as_str() >= from && version.first_seen_at.as_str() < to
            })
            .cloned()
            .collect();
        versions.sort_by(|a, b| {
            a.first_seen_at
                .cmp(&b.first_seen_at)
                .then_with(|| a.hash.cmp(&b.hash))
        });
        Ok(versions)
    }
}

/// Läufe ältester zuerst, bei gleichem Start nach ID
fn sort_runs(runs: &mut [DispatchRun]) {
    runs.sort_by(|a, b| {
        a.started_at
            .cmp(&b.started_at)
            .then_with(|| a.id.cmp(&b.id))
    });
}

#[async_trait]
impl DispatchStatsRepository for InMemoryUnitOfWork<'_> {
    async fn done_ticket_ids(&mut self, filter: &DispatchRunFilter) -> Result<Vec<String>> {
        let tickets: Vec<&str> = self
            .state
            .tickets
            .iter()
            .filter(|ticket| ticket.project_id == filter.project_id)
            .filter(|ticket| filter.board_id.is_none_or(|id| ticket.board_id == id))
            .map(|ticket| ticket.id.as_str())
            .collect();
        // Je Ticket der letzte Wechsel nach „erledigt“
        let mut last_done: BTreeMap<&str, &str> = BTreeMap::new();
        for entry in &self.state.history {
            let passt = entry.field == "status"
                && entry.to.as_deref() == Some("done")
                && tickets.contains(&entry.ticket_id.as_str());
            if !passt {
                continue;
            }
            let seen = last_done.get(entry.ticket_id.as_str()).copied();
            if seen.is_none_or(|at| entry.at.as_str() > at) {
                last_done.insert(entry.ticket_id.as_str(), entry.at.as_str());
            }
        }
        Ok(last_done
            .into_iter()
            .filter(|(_, at)| *at >= filter.from.as_str() && *at < filter.to.as_str())
            .map(|(id, _)| id.to_string())
            .collect())
    }

    async fn status_history(&mut self, ticket_ids: &[String]) -> Result<Vec<HistoryEntry>> {
        Ok(self
            .state
            .history
            .iter()
            .filter(|entry| entry.field == "status" || entry.field == "blockedReason")
            .filter(|entry| ticket_ids.contains(&entry.ticket_id))
            .cloned()
            .collect())
    }

    async fn dispatch_comment_counts(
        &mut self,
        ticket_ids: &[String],
    ) -> Result<BTreeMap<String, DispatchCommentCounts>> {
        let mut counts: BTreeMap<String, DispatchCommentCounts> = BTreeMap::new();
        for comment in &self.state.comments {
            if !ticket_ids.contains(&comment.ticket_id) {
                continue;
            }
            let start = comment.body.starts_with(START);
            let abort = comment.body.starts_with(ABORT);
            if !start && !abort {
                continue;
            }
            let entry = counts
                .entry(comment.ticket_id.clone())
                .or_insert(DispatchCommentCounts {
                    starts: 0,
                    aborts: 0,
                });
            if start {
                entry.starts += 1;
            } else {
                entry.aborts += 1;
            }
        }
        Ok(counts)
    }

    async fn dispatch_comments(&mut self, project_id: i32) -> Result<Vec<Comment>> {
        let ids: Vec<&str> = self
            .state
            .tickets
            .iter()
            .filter(|ticket| ticket.project_id == project_id)
            .map(|ticket| ticket.id.as_str())
            .collect();
        let mut comments: Vec<Comment> = self
            .state
            .comments
            .iter()
            .filter(|comment| {
                ids.contains(&comment.ticket_id.as_str())
                    && comment.body.starts_with(DISPATCH_PREFIX)
            })
            .cloned()
            .collect();
        comments.sort_by_key(|comment| comment.id);
        Ok(comments)
    }
}

// ---------------------------------------------------------------------------------------------
// Die Unit of Work selbst
// ---------------------------------------------------------------------------------------------

impl BoardUnitOfWork for InMemoryUnitOfWork<'_> {
    fn users(&mut self) -> &mut dyn UserRepository {
        self
    }
    fn projects(&mut self) -> &mut dyn ProjectRepository {
        self
    }
    fn boards(&mut self) -> &mut dyn BoardRepository {
        self
    }
    fn tickets(&mut self) -> &mut dyn TicketRepository {
        self
    }
    fn comments(&mut self) -> &mut dyn CommentRepository {
        self
    }
    fn steps(&mut self) -> &mut dyn TicketStepRepository {
        self
    }
    fn ticket_sessions(&mut self) -> &mut dyn TicketSessionRepository {
        self
    }
    fn history(&mut self) -> &mut dyn HistoryRepository {
        self
    }
    fn notes(&mut self) -> &mut dyn NoteRepository {
        self
    }
    fn deletions(&mut self) -> &mut dyn DeletionRepository {
        self
    }
    fn questions(&mut self) -> &mut dyn BoardQuestionRepository {
        self
    }
    fn question_research(&mut self) -> &mut dyn BoardQuestionResearchRepository {
        self
    }
    fn question_comments(&mut self) -> &mut dyn BoardQuestionCommentRepository {
        self
    }
    fn question_history(&mut self) -> &mut dyn BoardQuestionHistoryRepository {
        self
    }
    fn search(&mut self) -> &mut dyn BoardSearchRepository {
        self
    }
    fn dispatch_runs(&mut self) -> &mut dyn DispatchRunRepository {
        self
    }
    fn dispatch_stats(&mut self) -> &mut dyn DispatchStatsRepository {
        self
    }
}
