//! Fragen auf der Board-Datenbank: anlegen, lesen, auflisten, ändern, verknüpfen,
//! kommentieren, recherchieren, entscheiden, verwerfen und wieder öffnen. Die Regeln für
//! Recherchen und Entscheidungen kommen aus dem Datei-Modul (`ResearchValidator`,
//! `DecisionPolicy`). Portiert aus `packages/core/src/board/BoardQuestionService.ts`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::actors::Actor;
use crate::board::database::{
    BoardDatabase, BoardFilter, BoardQuestionFilter, BoardUnitOfWork, NewQuestionComment,
    NewQuestionResearch, TicketFilter,
};
use crate::board::dependency_rules::{DependencyGraphMap, DependencyRules};
use crate::board::labels::BoardLabels;
use crate::board::limits::BoardLimits;
use crate::board::project_service::ProjectService;
use crate::board::question_concern::{ConcernSubject, QuestionConcern};
use crate::board::question_ids::QuestionIds;
use crate::board::question_types::{
    BoardQuestion, BoardQuestionActor, BoardQuestionDecision, BoardQuestionDecisionOutcome,
    BoardQuestionDetail, BoardQuestionHistoryEntry, BoardQuestionHistoryType, BoardQuestionOption,
    BoardQuestionResearch, BoardQuestionView, NotifiableBoardQuestion, NotifiableOrigin,
    NotifiableResearch, MAX_OPTION_TITLE_LENGTH, MAX_QUESTION_OCCASION_LENGTH,
    MAX_QUESTION_TITLE_LENGTH, MAX_SESSION_LENGTH,
};
use crate::board::question_views::BoardQuestionViews;
use crate::board::repo_paths::BoardRepoPaths;
use crate::board::ticket_ids::TicketIds;
use crate::board::types::HistoryEntry;
use crate::clock::Clock;
use crate::error::{HarnessError, Result};
use crate::questions::question::{Decision, DecisionOutcome, QuestionOption, Research};
use crate::questions::{DecisionInput, DecisionPolicy, DecisionSubject, QuestionState};
use crate::questions::{ResearchInput, ResearchValidator};
use crate::text::utf16_prefix;
use crate::tx;

/// Höchstlänge der Notiz im Ticketverlauf (Spalte `board.History.Note`)
const TICKET_NOTE_LENGTH: usize = 200;

/// Eine neue Frage; nicht genannte Felder bleiben leer
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewBoardQuestionInput {
    pub title: String,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub occasion: Option<String>,
    pub ticket_ids: Option<Vec<String>>,
    pub depends_on: Option<Vec<String>>,
}

impl NewBoardQuestionInput {
    /// Eine Frage, die nur einen Titel hat
    pub fn titled(title: &str) -> Self {
        Self {
            title: title.to_string(),
            ..Self::default()
        }
    }
}

/// Änderbare Felder; nicht genannte bleiben. `occasion: Some(None)` löscht den Anlass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoardQuestionChanges {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub occasion: Option<Option<String>>,
}

impl BoardQuestionChanges {
    /// Keine Änderung angegeben?
    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.description.is_none()
            && self.tags.is_none()
            && self.occasion.is_none()
    }
}

/// Auswahl der Liste; genannte Kriterien werden mit „und“ verknüpft. `state: None` heißt
/// „alle“ (das `'all'` aus TypeScript).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoardQuestionListFilter {
    pub state: Option<QuestionState>,
    pub ticket: Option<String>,
    pub tag: Option<String>,
    pub text: Option<String>,
}

/// Frage der Liste mit Kennzeichnung „betrifft den Betrachter“ (eigene Frage, siehe
/// [`QuestionConcern`])
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionListItem {
    #[serde(flatten)]
    #[ts(flatten)]
    pub view: BoardQuestionView,
    pub mine: bool,
}

/// Zähler je Zustand über alle Fragen des Projekts (ohne Filter); `mine` = offene Fragen, die
/// den Betrachter betreffen
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionCounts {
    pub open: i32,
    pub answered: i32,
    pub discarded: i32,
    pub mine: i32,
}

/// Die Fragen eines Projekts samt Zählern
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "question.ts")]
pub struct BoardQuestionList {
    pub questions: Vec<BoardQuestionListItem>,
    pub counts: BoardQuestionCounts,
}

/// Erwartete Version beim Schreiben
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BoardQuestionWriteOptions {
    pub rev: Option<i32>,
}

impl BoardQuestionWriteOptions {
    /// Schreiben auf genau dieser Version
    pub fn rev(rev: i32) -> Self {
        Self { rev: Some(rev) }
    }
}

/// Ein Verlaufseintrag, bevor Zeit und Handelnder dazukommen
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub entry_type: BoardQuestionHistoryType,
    pub text: String,
}

/// Welcher Entscheidungsschritt läuft; der Text in der Ticketnotiz steht daneben
#[derive(Debug, Clone, PartialEq, Eq)]
enum DecisionStep {
    Answer(DecisionInput),
    Discard(Option<String>),
    Reopen(Option<String>),
}

impl DecisionStep {
    /// Der Text der Notiz im Ticketverlauf
    fn label(&self) -> &'static str {
        match self {
            DecisionStep::Answer(_) => "Frage entschieden",
            DecisionStep::Discard(_) => "Frage verworfen",
            DecisionStep::Reopen(_) => "Frage wieder geöffnet",
        }
    }
}

/// Fragen auf der Board-Datenbank.
#[derive(Debug, Clone)]
pub struct BoardQuestionService<D: BoardDatabase> {
    db: Arc<D>,
    clock: Clock,
    repo_paths: BoardRepoPaths,
}

impl<D: BoardDatabase> BoardQuestionService<D> {
    /// Dienst auf dieser Datenbank mit dieser Uhr und diesen Repo-Pfaden
    pub fn new(db: Arc<D>, clock: Clock, repo_paths: BoardRepoPaths) -> Self {
        Self {
            db,
            clock,
            repo_paths,
        }
    }

    /// Legt eine Frage im Projekt an; erst alle Prüfungen, dann die Nummer (ein Fehler rollt
    /// den Zähler zurück)
    pub async fn create(
        &self,
        project_key: &str,
        input: &NewBoardQuestionInput,
        actor: &BoardQuestionActor,
    ) -> Result<BoardQuestionDetail> {
        let project_key = project_key.to_string();
        let input = input.clone();
        let actor = actor.clone();
        let clock = self.clock.clone();
        tx!(self.db, [project_key, input, actor, clock], |u| {
            check_actor(&actor)?;
            let project = ProjectService::<D>::require_in(u, &project_key).await?;
            let title = require_title(&input.title)?;
            let tags = BoardLabels::normalize(input.tags.as_deref().unwrap_or(&[]))?;
            let occasion = occasion(input.occasion.as_deref())?;
            let ticket_ids =
                check_tickets(u, project.id, input.ticket_ids.as_deref().unwrap_or(&[])).await?;
            let number = u.projects().take_question_number(project.id).await?;
            let id = QuestionIds::format(&project.key, number as u32);
            let depends_on =
                check_depends(u, &id, input.depends_on.as_deref().unwrap_or(&[])).await?;
            let at = clock.iso();
            let question = BoardQuestion {
                id,
                project_id: project.id,
                number,
                title,
                description: input.description.clone().unwrap_or_default(),
                state: QuestionState::Open,
                occasion,
                origin_kind: actor.kind,
                origin_name: actor.actor.name.clone(),
                origin_session: actor.session.clone(),
                tags,
                ticket_ids,
                depends_on,
                decision: None,
                created_at: at.clone(),
                created_by_id: actor.actor.user_id.clone(),
                updated_at: at.clone(),
                rev: 1,
            };
            u.questions().insert(&question).await?;
            write_history(
                u,
                &question.id,
                &actor,
                &at,
                &[Entry {
                    entry_type: BoardQuestionHistoryType::Created,
                    text: "Frage angelegt".into(),
                }],
            )
            .await?;
            BoardQuestionViews::detail(u, &question).await
        })
        .await
    }

    /// Eine Frage mit Recherchen, Kommentaren und Verlauf
    pub async fn get(&self, id: &str) -> Result<BoardQuestionDetail> {
        let id = id.to_string();
        tx!(self.db, [id], |u| {
            let question = load(u, &id, None, false).await?;
            BoardQuestionViews::detail(u, &question).await
        })
        .await
    }

    /// Fragen eines Projekts, gefiltert und sortiert (bereite zuerst, darin die ältesten), mit
    /// Zählern je Zustand. Mit `viewer` (Benutzer-GUID) trägt jede Frage `mine` und
    /// `counts.mine` zählt die offenen eigenen; ohne ist nichts eigen.
    pub async fn list(
        &self,
        project_key: &str,
        filter: &BoardQuestionListFilter,
        viewer: Option<&str>,
    ) -> Result<BoardQuestionList> {
        let project_key = project_key.to_string();
        let filter = filter.clone();
        let viewer = viewer.map(str::to_string);
        tx!(self.db, [project_key, filter, viewer], |u| {
            let project = ProjectService::<D>::require_in(u, &project_key).await?;
            let own: HashSet<i32> = match viewer.as_deref() {
                Some(viewer) => u
                    .boards()
                    .list(&BoardFilter {
                        owner_id: Some(viewer.to_string()),
                        ..BoardFilter::default()
                    })
                    .await?
                    .into_iter()
                    .map(|board| board.id)
                    .collect(),
                None => HashSet::new(),
            };
            let questions = u
                .questions()
                .list(&BoardQuestionFilter {
                    project_id: Some(project.id),
                    ..BoardQuestionFilter::default()
                })
                .await?;
            let views = BoardQuestionViews::build(u, &questions).await?;
            let mut items: Vec<BoardQuestionListItem> = views
                .into_iter()
                .map(|view| {
                    let mine = match viewer.as_deref() {
                        Some(viewer) => QuestionConcern::concerns(
                            &ConcernSubject {
                                created_by_id: view.question.created_by_id.clone(),
                                board_ids: view
                                    .tickets
                                    .iter()
                                    .map(|ticket| ticket.board_id)
                                    .collect(),
                            },
                            viewer,
                            &own,
                        ),
                        None => false,
                    };
                    BoardQuestionListItem { view, mine }
                })
                .collect();
            let mut counts = BoardQuestionCounts::default();
            for item in &items {
                match item.view.question.state {
                    QuestionState::Open => counts.open += 1,
                    QuestionState::Answered => counts.answered += 1,
                    QuestionState::Discarded => counts.discarded += 1,
                }
                if item.view.question.state == QuestionState::Open && item.mine {
                    counts.mine += 1;
                }
            }
            let ticket = match filter.ticket.as_deref() {
                Some(ticket) => Some(TicketIds::canonical(ticket)?),
                None => None,
            };
            let text = filter.text.as_deref().unwrap_or("").trim().to_lowercase();
            items.retain(|item| {
                let question = &item.view.question;
                filter.state.is_none_or(|state| question.state == state)
                    && ticket
                        .as_deref()
                        .is_none_or(|ticket| question.ticket_ids.iter().any(|id| id == ticket))
                    && filter
                        .tag
                        .as_deref()
                        .is_none_or(|tag| question.tags.iter().any(|t| t == tag))
                    && (text.is_empty() || matches_text(question, &text))
            });
            items.sort_by(|a, b| compare(&a.view, &b.view));
            Ok(BoardQuestionList {
                questions: items,
                counts,
            })
        })
        .await
    }

    /// Ändert Titel, Beschreibung, Tags und Anlass; jede tatsächliche Änderung steht im Verlauf
    pub async fn update(
        &self,
        id: &str,
        changes: &BoardQuestionChanges,
        actor: &BoardQuestionActor,
        options: &BoardQuestionWriteOptions,
    ) -> Result<BoardQuestionDetail> {
        if changes.is_empty() {
            return Err(HarnessError::new("nothing-changed", "Keine Änderung angegeben.").into());
        }
        let id = id.to_string();
        let changes = changes.clone();
        let actor = actor.clone();
        let options = *options;
        let clock = self.clock.clone();
        tx!(self.db, [id, changes, actor, options, clock], |u| {
            check_actor(&actor)?;
            let before = load(u, &id, options.rev, true).await?;
            let mut after = before.clone();
            let mut entries: Vec<Entry> = Vec::new();
            if let Some(raw) = changes.title.as_deref() {
                let title = require_title(raw)?;
                if title != before.title {
                    entries.push(Entry {
                        entry_type: BoardQuestionHistoryType::Title,
                        text: format!("{} → {title}", before.title),
                    });
                    after.title = title;
                }
            }
            if let Some(description) = changes.description.as_deref() {
                if description != before.description {
                    // Der Text enthält die frühere Fassung, wie im Datei-Modul
                    entries.push(Entry {
                        entry_type: BoardQuestionHistoryType::Description,
                        text: if before.description.is_empty() {
                            "(leer)".into()
                        } else {
                            before.description.clone()
                        },
                    });
                    after.description = description.to_string();
                }
            }
            if let Some(raw) = changes.tags.as_deref() {
                let tags = BoardLabels::normalize(raw)?;
                if tags != before.tags {
                    entries.push(Entry {
                        entry_type: BoardQuestionHistoryType::Tags,
                        text: format!("{} → {}", names(&before.tags), names(&tags)),
                    });
                    after.tags = tags;
                }
            }
            if let Some(raw) = changes.occasion.as_ref() {
                let next = occasion(raw.as_deref())?;
                if next != before.occasion {
                    entries.push(Entry {
                        entry_type: BoardQuestionHistoryType::Occasion,
                        text: format!(
                            "{} → {}",
                            before.occasion.as_deref().unwrap_or("(keiner)"),
                            next.as_deref().unwrap_or("(keiner)")
                        ),
                    });
                    after.occasion = next;
                }
            }
            if entries.is_empty() {
                return BoardQuestionViews::detail(u, &before).await;
            }
            let saved = save(u, &before, after, &actor, &entries, &clock.iso()).await?;
            BoardQuestionViews::detail(u, &saved).await
        })
        .await
    }

    /// Setzt die betroffenen Tickets (müssen existieren und zum Projekt gehören)
    pub async fn set_tickets(
        &self,
        id: &str,
        ticket_ids: &[String],
        actor: &BoardQuestionActor,
        options: &BoardQuestionWriteOptions,
    ) -> Result<BoardQuestionDetail> {
        let id = id.to_string();
        let ticket_ids = ticket_ids.to_vec();
        let actor = actor.clone();
        let options = *options;
        let clock = self.clock.clone();
        tx!(self.db, [id, ticket_ids, actor, options, clock], |u| {
            check_actor(&actor)?;
            let before = load(u, &id, options.rev, true).await?;
            let next = check_tickets(u, before.project_id, &ticket_ids).await?;
            if next == before.ticket_ids {
                return BoardQuestionViews::detail(u, &before).await;
            }
            let entry = Entry {
                entry_type: BoardQuestionHistoryType::Tickets,
                text: format!("{} → {}", names(&before.ticket_ids), names(&next)),
            };
            let after = BoardQuestion {
                ticket_ids: next,
                ..before.clone()
            };
            let saved = save(u, &before, after, &actor, &[entry], &clock.iso()).await?;
            BoardQuestionViews::detail(u, &saved).await
        })
        .await
    }

    /// Setzt die Abhängigkeiten (Fragen oder Tickets); Unbekanntes, Selbstbezug und Kreise
    /// werden abgelehnt
    pub async fn set_depends(
        &self,
        id: &str,
        depends_on: &[String],
        actor: &BoardQuestionActor,
        options: &BoardQuestionWriteOptions,
    ) -> Result<BoardQuestionDetail> {
        let id = id.to_string();
        let depends_on = depends_on.to_vec();
        let actor = actor.clone();
        let options = *options;
        let clock = self.clock.clone();
        tx!(self.db, [id, depends_on, actor, options, clock], |u| {
            check_actor(&actor)?;
            let before = load(u, &id, options.rev, true).await?;
            let next = check_depends(u, &before.id, &depends_on).await?;
            if next == before.depends_on {
                return BoardQuestionViews::detail(u, &before).await;
            }
            let entry = Entry {
                entry_type: BoardQuestionHistoryType::Depends,
                text: format!("{} → {}", names(&before.depends_on), names(&next)),
            };
            let after = BoardQuestion {
                depends_on: next,
                ..before.clone()
            };
            let saved = save(u, &before, after, &actor, &[entry], &clock.iso()).await?;
            BoardQuestionViews::detail(u, &saved).await
        })
        .await
    }

    /// Hängt einen Kommentar an; er erhöht die Version der Frage
    pub async fn comment(
        &self,
        id: &str,
        body: &str,
        actor: &BoardQuestionActor,
    ) -> Result<BoardQuestionDetail> {
        let text = body.trim().to_string();
        if text.is_empty() {
            return Err(
                HarnessError::new("comment-empty", "Der Kommentar darf nicht leer sein.").into(),
            );
        }
        let id = id.to_string();
        let actor = actor.clone();
        let clock = self.clock.clone();
        tx!(self.db, [id, text, actor, clock], |u| {
            check_actor(&actor)?;
            let before = load(u, &id, None, true).await?;
            let at = clock.iso();
            u.question_comments()
                .insert(&NewQuestionComment {
                    question_id: before.id.clone(),
                    author_id: actor.actor.user_id.clone(),
                    author_name: actor.actor.name.clone(),
                    author_kind: actor.kind,
                    body: text.clone(),
                    created_at: at.clone(),
                })
                .await?;
            let saved = save(u, &before, before.clone(), &actor, &[], &at).await?;
            BoardQuestionViews::detail(u, &saved).await
        })
        .await
    }

    /// Ergänzt eine Recherche (nur anhängend); Optionen zählen über alle Recherchen der Frage
    /// weiter
    pub async fn add_research(
        &self,
        id: &str,
        input: &ResearchInput,
        actor: &BoardQuestionActor,
    ) -> Result<BoardQuestionDetail> {
        let id = id.to_string();
        let input = input.clone();
        let actor = actor.clone();
        let clock = self.clock.clone();
        tx!(self.db, [id, input, actor, clock], |u| {
            check_actor(&actor)?;
            let before = load(u, &id, None, true).await?;
            let at = clock.iso();
            let existing: Vec<Research> = u
                .question_research()
                .list(&before.id)
                .await?
                .iter()
                .map(to_research)
                .collect();
            let built = ResearchValidator::build(&input, &existing, &file_actor(&actor), &at)?;
            for option in &built.options {
                BoardLimits::text(
                    &option.title,
                    MAX_OPTION_TITLE_LENGTH,
                    "Der Titel einer Option",
                )?;
            }
            let options: Vec<BoardQuestionOption> =
                built.options.iter().map(to_board_option).collect();
            u.question_research()
                .insert(&NewQuestionResearch {
                    question_id: before.id.clone(),
                    at: at.clone(),
                    by_id: actor.actor.user_id.clone(),
                    by_name: actor.actor.name.clone(),
                    kind: actor.kind,
                    context: built.context.clone(),
                    options,
                })
                .await?;
            let entry = Entry {
                entry_type: BoardQuestionHistoryType::Research,
                text: ResearchValidator::describe(&built.options),
            };
            let saved = save(u, &before, before.clone(), &actor, &[entry], &at).await?;
            BoardQuestionViews::detail(u, &saved).await
        })
        .await
    }

    /// Beantwortet eine offene Frage (nur Mensch): Option der neuesten Recherche oder Freitext
    pub async fn answer(
        &self,
        id: &str,
        input: &DecisionInput,
        actor: &BoardQuestionActor,
        options: &BoardQuestionWriteOptions,
    ) -> Result<BoardQuestionDetail> {
        self.decide(id, DecisionStep::Answer(input.clone()), actor, options)
            .await
    }

    /// Verwirft eine offene Frage (nur Mensch) mit Begründung
    pub async fn discard(
        &self,
        id: &str,
        reason: Option<&str>,
        actor: &BoardQuestionActor,
        options: &BoardQuestionWriteOptions,
    ) -> Result<BoardQuestionDetail> {
        self.decide(
            id,
            DecisionStep::Discard(reason.map(str::to_string)),
            actor,
            options,
        )
        .await
    }

    /// Öffnet eine entschiedene oder verworfene Frage wieder (nur Mensch); die frühere
    /// Entscheidung bleibt im Verlauf
    pub async fn reopen(
        &self,
        id: &str,
        reason: Option<&str>,
        actor: &BoardQuestionActor,
        options: &BoardQuestionWriteOptions,
    ) -> Result<BoardQuestionDetail> {
        self.decide(
            id,
            DecisionStep::Reopen(reason.map(str::to_string)),
            actor,
            options,
        )
        .await
    }

    /// Offene Fragen, die einen Benutzer betreffen („Eigene“: selbst gestellt oder zu Tickets
    /// auf seinen Boards), mit ihren Recherchen (für Windows-Meldungen)
    pub async fn notifiable(&self, owner_id: &str) -> Result<Vec<NotifiableBoardQuestion>> {
        let owner_id = owner_id.to_string();
        let repo_paths = self.repo_paths.clone();
        tx!(self.db, [owner_id, repo_paths], |u| {
            let boards = repo_paths.apply(
                u.boards()
                    .list(&BoardFilter {
                        owner_id: Some(owner_id.clone()),
                        ..BoardFilter::default()
                    })
                    .await?,
            )?;
            let own: HashSet<i32> = boards.iter().map(|board| board.id).collect();
            let board_ids: Vec<i32> = boards.iter().map(|board| board.id).collect();
            let tickets = if board_ids.is_empty() {
                Vec::new()
            } else {
                u.tickets()
                    .list(&TicketFilter {
                        board_ids: Some(board_ids),
                        without_texts: true,
                        ..TicketFilter::default()
                    })
                    .await?
            };
            let board_of: HashMap<String, i32> = tickets
                .iter()
                .map(|ticket| (ticket.id.clone(), ticket.board_id))
                .collect();
            let repo_of: HashMap<i32, Option<String>> = boards
                .iter()
                .map(|board| (board.id, board.repo_path.clone()))
                .collect();
            let open: Vec<BoardQuestion> = u
                .questions()
                .list(&BoardQuestionFilter {
                    states: Some(vec![QuestionState::Open]),
                    ..BoardQuestionFilter::default()
                })
                .await?
                .into_iter()
                .filter(|question| {
                    QuestionConcern::concerns(
                        &ConcernSubject {
                            created_by_id: question.created_by_id.clone(),
                            board_ids: question
                                .ticket_ids
                                .iter()
                                .filter_map(|id| board_of.get(id).copied())
                                .collect(),
                        },
                        &owner_id,
                        &own,
                    )
                })
                .collect();
            let projects: HashMap<i32, (String, String)> = u
                .projects()
                .list()
                .await?
                .into_iter()
                .map(|project| (project.id, (project.key, project.name)))
                .collect();
            let mut result = Vec::new();
            for question in open {
                let research = u.question_research().list(&question.id).await?;
                // Repo-Pfade der eigenen Boards mit betroffenen Tickets, sonst der eigenen
                // Boards im Projekt der Frage
                let by_ticket: Vec<Option<String>> = question
                    .ticket_ids
                    .iter()
                    .map(|id| {
                        board_of
                            .get(id)
                            .and_then(|board_id| repo_of.get(board_id))
                            .cloned()
                            .flatten()
                    })
                    .collect();
                let found = if by_ticket.iter().any(Option::is_some) {
                    by_ticket
                } else {
                    let mut own_in_project: Vec<_> = boards
                        .iter()
                        .filter(|board| board.project_id == question.project_id)
                        .collect();
                    own_in_project.sort_by_key(|board| board.id);
                    own_in_project
                        .into_iter()
                        .map(|board| board.repo_path.clone())
                        .collect()
                };
                let mut repo_paths: Vec<String> = Vec::new();
                for path in found.into_iter().flatten() {
                    if !repo_paths.contains(&path) {
                        repo_paths.push(path);
                    }
                }
                let project = projects.get(&question.project_id);
                result.push(NotifiableBoardQuestion {
                    id: question.id.clone(),
                    title: question.title.clone(),
                    occasion: question.occasion.clone(),
                    origin: NotifiableOrigin {
                        kind: question.origin_kind,
                    },
                    research: research
                        .into_iter()
                        .map(|entry| NotifiableResearch {
                            kind: entry.kind,
                            options: entry.options,
                        })
                        .collect(),
                    project_key: project
                        .map(|(key, _)| key.clone())
                        .unwrap_or_else(|| "?".to_string()),
                    project_name: project
                        .map(|(_, name)| name.clone())
                        .unwrap_or_else(|| "?".to_string()),
                    repo_paths,
                });
            }
            Ok(result)
        })
        .await
    }

    /// Gemeinsamer Ablauf: Frage gesperrt lesen (mit Version), Regel anwenden, speichern,
    /// Verlauf an Frage und Tickets
    async fn decide(
        &self,
        id: &str,
        step: DecisionStep,
        actor: &BoardQuestionActor,
        options: &BoardQuestionWriteOptions,
    ) -> Result<BoardQuestionDetail> {
        let id = id.to_string();
        let actor = actor.clone();
        let options = *options;
        let clock = self.clock.clone();
        tx!(self.db, [id, step, actor, options, clock], |u| {
            check_actor(&actor)?;
            let before = load(u, &id, options.rev, true).await?;
            let research: Vec<Research> = u
                .question_research()
                .list(&before.id)
                .await?
                .iter()
                .map(to_research)
                .collect();
            let at = clock.iso();
            let subject = DecisionSubject {
                id: before.id.clone(),
                state: before.state,
                research,
            };
            let who = file_actor(&actor);
            let (state, decision, entry) = match &step {
                DecisionStep::Answer(input) => {
                    let decision = DecisionPolicy::answer(&subject, input, &who, &at)?;
                    let text = DecisionPolicy::describe(&subject.research, &decision);
                    (
                        QuestionState::Answered,
                        Some(decision),
                        Entry {
                            entry_type: BoardQuestionHistoryType::Decided,
                            text,
                        },
                    )
                }
                DecisionStep::Discard(reason) => {
                    let decision = DecisionPolicy::discard(
                        &subject.id,
                        subject.state,
                        reason.as_deref(),
                        &who,
                        &at,
                    )?;
                    let text = DecisionPolicy::describe(&subject.research, &decision);
                    (
                        QuestionState::Discarded,
                        Some(decision),
                        Entry {
                            entry_type: BoardQuestionHistoryType::Discarded,
                            text,
                        },
                    )
                }
                DecisionStep::Reopen(reason) => {
                    DecisionPolicy::assert_reopenable(&subject.id, subject.state, &who)?;
                    let earlier = before
                        .decision
                        .as_ref()
                        .map(|decision| {
                            format!(
                                "Frühere Entscheidung: {} ({})",
                                DecisionPolicy::describe(
                                    &subject.research,
                                    &file_decision(decision)
                                ),
                                decision.by_name
                            )
                        })
                        .unwrap_or_default();
                    let because = reason
                        .as_deref()
                        .map(str::trim)
                        .filter(|reason| !reason.is_empty())
                        .map(|reason| format!("Grund: {reason}."))
                        .unwrap_or_default();
                    let text = ["Wieder geöffnet.".to_string(), because, earlier]
                        .into_iter()
                        .filter(|part| !part.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    (
                        QuestionState::Open,
                        None,
                        Entry {
                            entry_type: BoardQuestionHistoryType::Reopened,
                            text,
                        },
                    )
                }
            };
            let after = BoardQuestion {
                state,
                decision: decision
                    .as_ref()
                    .map(|decision| board_decision(decision, &actor)),
                ..before.clone()
            };
            let saved = save(u, &before, after, &actor, &[entry], &at).await?;
            note_tickets(u, &before, &saved, step.label(), &actor, &at).await?;
            BoardQuestionViews::detail(u, &saved).await
        })
        .await
    }
}

/// Liest eine Frage (Schreibweise egal) und prüft die erwartete Version
pub async fn load(
    u: &mut dyn BoardUnitOfWork,
    id: &str,
    rev: Option<i32>,
    for_update: bool,
) -> Result<BoardQuestion> {
    let canonical = QuestionIds::canonical(id)?;
    let question = u
        .questions()
        .by_id(&canonical, for_update)
        .await?
        .ok_or_else(|| {
            HarnessError::new(
                "question-not-found",
                format!("Frage {canonical} gibt es nicht."),
            )
        })?;
    if let Some(rev) = rev {
        if rev != question.rev {
            return Err(HarnessError::new(
                "conflict",
                format!(
                    "Frage {canonical} wurde inzwischen geändert (Version {}, erwartet {rev}). Bitte neu lesen.",
                    question.rev
                ),
            )
            .into());
        }
    }
    Ok(question)
}

/// Schreibt die neue Fassung mit nächster Version und Zeitstempel und protokolliert die
/// Einträge
pub async fn save(
    u: &mut dyn BoardUnitOfWork,
    before: &BoardQuestion,
    after: BoardQuestion,
    actor: &BoardQuestionActor,
    entries: &[Entry],
    at: &str,
) -> Result<BoardQuestion> {
    let next = BoardQuestion {
        rev: before.rev + 1,
        updated_at: at.to_string(),
        ..after
    };
    u.questions().update(&next).await?;
    write_history(u, &next.id, actor, at, entries).await?;
    Ok(next)
}

/// Schreibt Verlaufseinträge mit Zeit, Handelndem und Art
async fn write_history(
    u: &mut dyn BoardUnitOfWork,
    question_id: &str,
    actor: &BoardQuestionActor,
    at: &str,
    entries: &[Entry],
) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let rows: Vec<BoardQuestionHistoryEntry> = entries
        .iter()
        .map(|entry| BoardQuestionHistoryEntry {
            question_id: question_id.to_string(),
            at: at.to_string(),
            by_id: actor.actor.user_id.clone(),
            by_name: actor.actor.name.clone(),
            by_kind: Some(actor.kind),
            entry_type: entry.entry_type,
            text: entry.text.clone(),
        })
        .collect();
    u.question_history().insert(&rows).await
}

/// Verlaufseintrag an jedem betroffenen Ticket: Feld „question“, Zustand vorher → nachher
async fn note_tickets(
    u: &mut dyn BoardUnitOfWork,
    before: &BoardQuestion,
    after: &BoardQuestion,
    label: &str,
    actor: &BoardQuestionActor,
    at: &str,
) -> Result<()> {
    let note = clip(
        &format!("{label}: {} „{}“", after.id, after.title),
        TICKET_NOTE_LENGTH,
    );
    let entries: Vec<HistoryEntry> = after
        .ticket_ids
        .iter()
        .map(|ticket_id| HistoryEntry {
            ticket_id: ticket_id.clone(),
            at: at.to_string(),
            by_id: actor.actor.user_id.clone(),
            by_name: actor.actor.name.clone(),
            field: "question".into(),
            from: Some(before.state.as_str().to_string()),
            to: Some(after.state.as_str().to_string()),
            note: Some(note.clone()),
        })
        .collect();
    if entries.is_empty() {
        return Ok(());
    }
    u.history().insert(&entries).await
}

/// Entscheidung des Datei-Moduls als Entscheidung der Board-Datenbank (mit Benutzer-GUID)
fn board_decision(decision: &Decision, actor: &BoardQuestionActor) -> BoardQuestionDecision {
    BoardQuestionDecision {
        outcome: match decision.outcome {
            DecisionOutcome::Answered => BoardQuestionDecisionOutcome::Answered,
            DecisionOutcome::Discarded => BoardQuestionDecisionOutcome::Discarded,
        },
        option: decision.option.clone(),
        text: decision.text.clone(),
        reason: decision.reason.clone(),
        by_id: actor.actor.user_id.clone(),
        by_name: decision.by.clone(),
        at: decision.at.clone(),
    }
}

/// Entscheidung der Board-Datenbank im Format des Datei-Moduls (für `DecisionPolicy::describe`)
fn file_decision(decision: &BoardQuestionDecision) -> Decision {
    Decision {
        outcome: match decision.outcome {
            BoardQuestionDecisionOutcome::Answered => DecisionOutcome::Answered,
            BoardQuestionDecisionOutcome::Discarded => DecisionOutcome::Discarded,
        },
        option: decision.option.clone(),
        text: decision.text.clone(),
        reason: decision.reason.clone(),
        by: decision.by_name.clone(),
        at: decision.at.clone(),
    }
}

/// Recherche im Format des Datei-Moduls (für `ResearchValidator` und `DecisionPolicy`)
fn to_research(research: &BoardQuestionResearch) -> Research {
    Research {
        at: research.at.clone(),
        author: research.by_name.clone(),
        kind: research.kind,
        context: research.context.clone(),
        options: research
            .options
            .iter()
            .map(|option| QuestionOption {
                id: option.id.clone(),
                title: option.title.clone(),
                text: option.text.clone(),
                pros: option.pros.clone(),
                cons: option.cons.clone(),
                recommended: option.recommended,
            })
            .collect(),
    }
}

/// Option des Datei-Moduls als Option der Board-Datenbank
fn to_board_option(option: &QuestionOption) -> BoardQuestionOption {
    BoardQuestionOption {
        id: option.id.clone(),
        title: option.title.clone(),
        text: option.text.clone(),
        pros: option.pros.clone(),
        cons: option.cons.clone(),
        recommended: option.recommended,
    }
}

/// Handelnder im Format des Datei-Moduls
fn file_actor(actor: &BoardQuestionActor) -> Actor {
    Actor {
        author: actor.actor.name.clone(),
        kind: actor.kind,
    }
}

/// Betroffene Tickets: Schreibweise vereinheitlichen, Doppelte entfernen, sortieren; jedes
/// muss im Projekt existieren
async fn check_tickets(
    u: &mut dyn BoardUnitOfWork,
    project_id: i32,
    raw: &[String],
) -> Result<Vec<String>> {
    let mut ids: Vec<String> = Vec::new();
    for item in raw {
        if item.trim().is_empty() {
            continue;
        }
        let id = TicketIds::canonical(item)?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids.sort();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let found: HashMap<String, i32> = u
        .tickets()
        .list(&TicketFilter {
            ids: Some(ids.clone()),
            without_texts: true,
            ..TicketFilter::default()
        })
        .await?
        .into_iter()
        .map(|ticket| (ticket.id, ticket.project_id))
        .collect();
    for id in &ids {
        match found.get(id) {
            None => {
                return Err(HarnessError::new(
                    "question-ticket-unknown",
                    format!("Ticket {id} gibt es nicht."),
                )
                .into())
            }
            Some(other) if *other != project_id => {
                return Err(HarnessError::new(
                    "question-ticket-other-project",
                    format!("Ticket {id} gehört zu einem anderen Projekt. Eine Frage betrifft nur Tickets ihres Projekts."),
                )
                .into())
            }
            Some(_) => {}
        }
    }
    Ok(ids)
}

/// Abhängigkeiten: Verweise vereinheitlichen (Reihenfolge bleibt), dann gegen alle Fragen und
/// die genannten Tickets prüfen. Tickets hängen nie von Fragen ab, deshalb sind sie im Graphen
/// Blätter.
async fn check_depends(
    u: &mut dyn BoardUnitOfWork,
    id: &str,
    raw: &[String],
) -> Result<Vec<String>> {
    let mut refs: Vec<String> = Vec::new();
    for item in raw {
        if item.trim().is_empty() {
            continue;
        }
        let reference = QuestionIds::reference(item)?;
        if !refs.contains(&reference) {
            refs.push(reference);
        }
    }
    if refs.is_empty() {
        return Ok(Vec::new());
    }
    let mut graph: DependencyGraphMap = u
        .questions()
        .list(&BoardQuestionFilter::default())
        .await?
        .into_iter()
        .map(|question| {
            let deps = question
                .depends_on
                .into_iter()
                .filter(|dep| QuestionIds::is_question(dep))
                .collect();
            (question.id, deps)
        })
        .collect();
    let tickets: Vec<String> = refs
        .iter()
        .filter(|reference| !QuestionIds::is_question(reference))
        .cloned()
        .collect();
    if !tickets.is_empty() {
        for ticket in u
            .tickets()
            .list(&TicketFilter {
                ids: Some(tickets),
                without_texts: true,
                ..TicketFilter::default()
            })
            .await?
        {
            graph.insert(ticket.id, Vec::new());
        }
    }
    DependencyRules::validate(id, &refs, &graph)?;
    Ok(refs)
}

/// Name des Handelnden und Kennung der Session dürfen die Spalten nicht sprengen
fn check_actor(actor: &BoardQuestionActor) -> Result<()> {
    BoardLimits::actor(&actor.actor)?;
    BoardLimits::text_opt(
        actor.session.as_deref(),
        MAX_SESSION_LENGTH,
        "Die Kennung der Session",
    )?;
    Ok(())
}

/// Titel ohne Leerraum am Rand; leer oder zu lang ist nicht erlaubt
fn require_title(title: &str) -> Result<String> {
    let value = title.trim();
    if value.is_empty() {
        return Err(HarnessError::new("title-empty", "Der Titel darf nicht leer sein.").into());
    }
    Ok(BoardLimits::text(value, MAX_QUESTION_TITLE_LENGTH, "Der Titel")?.to_string())
}

/// Anlass ohne Leerraum am Rand; leer wird zu `None`
fn occasion(value: Option<&str>) -> Result<Option<String>> {
    let text = value.map(str::trim).filter(|text| !text.is_empty());
    Ok(
        BoardLimits::text_opt(text, MAX_QUESTION_OCCASION_LENGTH, "Der Anlass")?
            .map(str::to_string),
    )
}

/// Trifft der Suchtext auf die Frage zu? (ID, Titel, Beschreibung, Anlass, Tags, Tickets)
fn matches_text(question: &BoardQuestion, text: &str) -> bool {
    [
        question.id.as_str(),
        question.title.as_str(),
        question.description.as_str(),
        question.occasion.as_deref().unwrap_or(""),
    ]
    .into_iter()
    .chain(question.tags.iter().map(String::as_str))
    .chain(question.ticket_ids.iter().map(String::as_str))
    .any(|part| part.to_lowercase().contains(text))
}

/// Bereite zuerst, darin die ältesten zuerst, bei Gleichstand nach Nummer (wie `QuestionFilter`
/// im Datei-Modul)
fn compare(a: &BoardQuestionView, b: &BoardQuestionView) -> std::cmp::Ordering {
    b.ready
        .cmp(&a.ready)
        .then_with(|| a.question.created_at.cmp(&b.question.created_at))
        .then_with(|| a.question.project_id.cmp(&b.question.project_id))
        .then_with(|| a.question.number.cmp(&b.question.number))
}

/// Liste als Text, leer als „(keine)“
fn names(items: &[String]) -> String {
    if items.is_empty() {
        "(keine)".to_string()
    } else {
        items.join(", ")
    }
}

/// Kürzt einen Text auf höchstens `max` Zeichen (mit „…“); gezählt wie in JavaScript
fn clip(text: &str, max: usize) -> String {
    if crate::text::utf16_len(text) > max {
        format!("{}…", utf16_prefix(text, max - 1))
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests;
