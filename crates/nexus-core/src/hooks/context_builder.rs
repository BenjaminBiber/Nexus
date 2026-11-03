//! Baut den Text, den ein Hook in die Session einspeist: Leitfaden zum Sessionbeginn, neue
//! Kommentare und offene Fragen. Portiert aus `packages/core/src/hooks/ContextBuilder.ts`.
//!
//! Alles kommt aus der Board-DB; eingespeiste Kommentare markiert der Aufrufer erst nach der
//! Ausgabe mit [`ContextBuilder::mark_read`].

use std::sync::Arc;

use chrono::{FixedOffset, Offset, TimeZone};

use crate::board::comment_service::CommentService;
use crate::board::database::{BoardDatabase, BoardQuestionFilter};
use crate::board::question_types::BoardQuestion;
use crate::board::ticket_service::{TicketDetail, TicketService};
use crate::board::types::Comment;
use crate::clock::Clock;
use crate::error::{Error, Result};
use crate::questions::QuestionState;
use crate::workflow::hint::{HintTicket, WorkflowHint};
use crate::workflow::phases::{workflow_phases, WorkflowPhase};

use super::question_context_marks::QuestionContextMarks;

/// Höchstens so viele Fragen nennt der Text
const MAX_QUESTIONS: usize = 5;

/// Die Ereignisse, zu denen Kontext eingespeist wird
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextKind {
    SessionStart,
    UserPrompt,
}

/// Ergebnis von [`ContextBuilder::build`]: einzuspeisender Text und die Kommentare darin, die
/// nach der Ausgabe als gelesen gelten
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HookContext {
    pub text: Option<String>,
    pub comment_ids: Vec<i64>,
}

/// Baut den Kontexttext aus Tickets, Kommentaren und Fragen des Boards
pub struct ContextBuilder<D: BoardDatabase> {
    db: Arc<D>,
    tickets: TicketService<D>,
    comments: CommentService<D>,
    marks: Option<QuestionContextMarks>,
    clock: Clock,
    workflow: WorkflowHint,
    /// Zeitzone für die Kommentar-Zeitpunkte; `None` = die des Rechners, wie
    /// `toLocaleString('de-DE')` in TypeScript. Tests geben eine feste Zone vor.
    zone: Option<FixedOffset>,
}

impl<D: BoardDatabase> ContextBuilder<D> {
    /// Mit dem Standard-Workflow und ohne Marken der Fragen-Einspeisung
    pub fn new(
        db: Arc<D>,
        tickets: TicketService<D>,
        comments: CommentService<D>,
        marks: Option<QuestionContextMarks>,
        clock: Clock,
    ) -> Self {
        Self::with_phases(db, tickets, comments, marks, clock, workflow_phases())
    }

    /// `phases`: angewendeter Workflow ([`crate::workflow::WorkflowSource`])
    pub fn with_phases(
        db: Arc<D>,
        tickets: TicketService<D>,
        comments: CommentService<D>,
        marks: Option<QuestionContextMarks>,
        clock: Clock,
        phases: Vec<WorkflowPhase>,
    ) -> Self {
        Self {
            db,
            tickets,
            comments,
            marks,
            clock,
            workflow: WorkflowHint::new(phases),
            zone: None,
        }
    }

    /// Derselbe Erbauer mit fester Zeitzone für die Kommentar-Zeitpunkte
    pub fn in_zone(mut self, zone: FixedOffset) -> Self {
        self.zone = Some(zone);
        self
    }

    /// `SessionStart`: Leitfaden, Kommentare und offene Fragen; `UserPrompt`: Kommentare und
    /// neue oder geänderte Fragen. Mit `session_id` merkt sich der Harness die Einspeisung der
    /// Fragen. Text `None`, wenn es nichts gibt.
    /// DB-Fehler (außer fehlenden Tickets) werden weitergereicht; der Aufrufer speist dann
    /// keinen Kontext ein.
    pub async fn build(
        &self,
        kind: ContextKind,
        ticket_ids: &[String],
        session_id: Option<&str>,
    ) -> Result<HookContext> {
        let tickets = self.load(ticket_ids).await?;
        let mut parts: Vec<String> = Vec::new();
        if kind == ContextKind::SessionStart {
            parts.push(self.guide(&tickets));
        }
        let (comment_parts, comment_ids) = self.comments(&tickets).await?;
        parts.extend(comment_parts);
        if let Some(questions) = self.questions(kind, &tickets, session_id).await? {
            parts.push(questions);
        }
        Ok(HookContext {
            text: if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n\n"))
            },
            comment_ids,
        })
    }

    /// Markiert eingespeiste Kommentare als gelesen; erst aufrufen, wenn der Kontext
    /// ausgegeben ist
    pub async fn mark_read(&self, comment_ids: &[i64]) -> Result<()> {
        self.comments.mark_read_by_agent(comment_ids).await
    }

    /// Ungelesene Kommentare je Ticket und ihre IDs; als gelesen markiert erst `mark_read`
    async fn comments(&self, tickets: &[TicketDetail]) -> Result<(Vec<String>, Vec<i64>)> {
        if tickets.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let ids: Vec<String> = tickets.iter().map(|t| t.id.clone()).collect();
        let unread = self.comments.unread_for_agent(&ids).await?;
        let parts: Vec<String> = tickets
            .iter()
            .filter_map(|ticket| {
                let own: Vec<&Comment> = unread
                    .iter()
                    .filter(|comment| comment.ticket_id == ticket.id)
                    .collect();
                (!own.is_empty()).then(|| self.comment_list(&ticket.id, &own))
            })
            .collect();
        Ok((parts, unread.iter().map(|comment| comment.id).collect()))
    }

    /// Offene Fragen zu den Tickets; beim Prompt mit Session nur neue oder geänderte seit der
    /// letzten Einspeisung
    async fn questions(
        &self,
        kind: ContextKind,
        tickets: &[TicketDetail],
        session_id: Option<&str>,
    ) -> Result<Option<String>> {
        if tickets.is_empty() {
            return Ok(None);
        }
        let ids: Vec<String> = tickets.iter().map(|t| t.id.clone()).collect();
        let since = match (kind, session_id, &self.marks) {
            (ContextKind::UserPrompt, Some(session_id), Some(marks)) => marks.get(session_id),
            _ => None,
        };
        // Zeitpunkt vor dem Lesen festhalten, damit eine Änderung während des Lesens beim
        // nächsten Mal nicht verloren geht
        let read_at = self.clock.iso();
        let filter = BoardQuestionFilter {
            ticket_ids: Some(ids.clone()),
            states: Some(vec![QuestionState::Open]),
            ..BoardQuestionFilter::default()
        };
        let open = crate::tx!(self.db, [filter], |u| { u.questions().list(&filter).await }).await?;
        let news: Vec<BoardQuestion> = open
            .into_iter()
            .filter(|question| match &since {
                None => true,
                Some(since) => question.updated_at > *since,
            })
            .collect();
        if let (Some(session_id), Some(marks)) = (session_id, &self.marks) {
            if kind == ContextKind::SessionStart || !news.is_empty() {
                marks.set(session_id, &read_at)?;
            }
        }
        Ok((!news.is_empty()).then(|| question_list(&news, &ids)))
    }

    /// Lädt die Tickets; unbekannte IDs werden übergangen, andere Fehler (z. B. DB weg)
    /// weitergereicht
    async fn load(&self, ids: &[String]) -> Result<Vec<TicketDetail>> {
        let mut tickets = Vec::new();
        for id in ids {
            match self.tickets.get(id).await {
                Ok(ticket) => tickets.push(ticket),
                // Ticket gibt es nicht (mehr)
                Err(error) if is_not_found(&error) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(tickets)
    }

    /// Leitfaden zum Sessionbeginn
    fn guide(&self, tickets: &[TicketDetail]) -> String {
        let hint = self.workflow.build(
            &tickets
                .iter()
                .map(|ticket| HintTicket {
                    id: ticket.id.clone(),
                    status: ticket.status,
                })
                .collect::<Vec<_>>(),
        );
        if tickets.is_empty() {
            return [
                "Nexus: Diese Session ist keinem Ticket zugeordnet. Mit `nexus board claim ARC-0042 --agent claude` ordnest du sie einem Ticket zu (ID anpassen).".to_string(),
                hint,
            ]
            .join("\n");
        }
        let names = tickets
            .iter()
            .map(|ticket| format!("{} „{}“{}", ticket.id, ticket.title, meta(ticket)))
            .collect::<Vec<_>>()
            .join(", ");
        let first = &tickets[0].id;
        [
            format!("Nexus: Diese Session arbeitet an {names}."),
            format!("Regeln: Tickets änderst du nur über `nexus board …`. Nutze `nexus board show {first}`, `nexus board comment {first} \"…\" --author claude`, `nexus board step {first} <nummer>` und zum Abschluss `nexus board done {first} --acceptance \"…\"`. Begründe jede Änderung am Ticket mit einem Kommentar."),
            hint,
        ]
        .join("\n")
    }

    /// Neue Kommentare eines Tickets, in zeitlicher Reihenfolge
    fn comment_list(&self, id: &str, comments: &[&Comment]) -> String {
        let mut lines = vec![format!("Neue Kommentare zu {id}:")];
        lines.extend(comments.iter().map(|comment| {
            format!(
                "- {}, {}: {}",
                self.date(&comment.created_at),
                comment.author_name,
                comment.body.replace('\n', "\n  ")
            )
        }));
        lines.join("\n")
    }

    /// Datum und Uhrzeit, deutsch (`toLocaleString('de-DE', …)`)
    fn date(&self, iso: &str) -> String {
        let Ok(at) = chrono::DateTime::parse_from_rfc3339(iso) else {
            return iso.to_string();
        };
        let zone = self.zone.unwrap_or_else(|| {
            chrono::Local
                .offset_from_utc_datetime(&at.naive_utc())
                .fix()
        });
        at.with_timezone(&zone)
            .format("%d.%m.%Y, %H:%M")
            .to_string()
    }
}

/// Spec und Fortschritt der Schritte in Klammern, sofern vorhanden
fn meta(ticket: &TicketDetail) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(spec) = ticket.spec.as_deref().filter(|spec| !spec.is_empty()) {
        parts.push(format!("Spec {spec}"));
    }
    if ticket.steps_total != 0 {
        parts.push(format!(
            "Schritte {}/{}",
            ticket.steps_done, ticket.steps_total
        ));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(", "))
    }
}

/// Liste der offenen Fragen mit Regeln für den Agenten
fn question_list(questions: &[BoardQuestion], ticket_ids: &[String]) -> String {
    let shown = &questions[..questions.len().min(MAX_QUESTIONS)];
    let count = if questions.len() > MAX_QUESTIONS {
        format!("{MAX_QUESTIONS} von {}", questions.len())
    } else {
        questions.len().to_string()
    };
    let first = &shown[0].id;
    let mut lines = vec![format!("Offene Fragen ({count}, nicht entscheiden):")];
    lines.extend(shown.iter().map(|question| {
        let betrifft = question
            .ticket_ids
            .iter()
            .filter(|id| ticket_ids.contains(id))
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        format!("- {} {} (betrifft {betrifft})", question.id, question.title)
    }));
    lines.push("Diese Fragen sperren ihre Tickets für In Arbeit und Erledigt. Triff die Entscheidungen nie selbst und nimm nichts stillschweigend an.".to_string());
    lines.push(format!("Ergänze Kontext mit `nexus board question research {first} --from <datei> --kind session` oder kommentiere mit `nexus board question comment {first} \"…\" --kind session`."));
    lines.join("\n")
}

/// Meldet der Fehler ein fehlendes Ticket?
fn is_not_found(error: &Error) -> bool {
    error
        .code()
        .is_some_and(|code| code.ends_with("-not-found"))
}

#[cfg(test)]
mod tests {
    use crate::board::basics::TicketStatus;
    use crate::board::in_memory::InMemoryBoardDatabase;
    use crate::board::question_service::{BoardQuestionChanges, NewBoardQuestionInput};
    use crate::board::ticket_service::{NewTicketInput, TicketPatch, WriteOptions};
    use crate::board::types::TicketStep;
    use crate::board::{BoardQuestionWriteOptions, MoveOptions};
    use crate::project::HarnessLayout;
    use crate::testing::BoardFixture;

    use super::*;

    /// Die Umgebung des Tests: Board mit zwei Tickets und ein Erbauer mit Marken
    struct Umgebung {
        f: BoardFixture,
        dir: tempfile::TempDir,
        builder: ContextBuilder<InMemoryBoardDatabase>,
    }

    impl Umgebung {
        async fn neu() -> Self {
            let f = BoardFixture::create().await;
            let dir = tempfile::tempdir().unwrap();
            let layout = HarnessLayout::new(dir.path());
            std::fs::create_dir_all(layout.dir()).unwrap();
            let marks = QuestionContextMarks::new(layout, f.clock.clone());
            let builder = ContextBuilder::new(
                f.db.clone(),
                TicketService::new(f.db.clone(), f.clock.clone()),
                CommentService::new(f.db.clone(), f.clock.clone()),
                Some(marks),
                f.clock.clone(),
            );
            f.tickets
                .create(
                    f.board_id,
                    &NewTicketInput {
                        spec: Some("001-x".into()),
                        spec_task: Some("T001".into()),
                        steps: Some(vec![TicketStep {
                            text: "A".into(),
                            done: false,
                            detail: None,
                        }]),
                        ..NewTicketInput::titled("Login")
                    },
                    &f.actor,
                    &WriteOptions::default(),
                )
                .await
                .unwrap();
            f.tickets
                .create(
                    f.board_id,
                    &NewTicketInput::titled("Zweites"),
                    &f.actor,
                    &WriteOptions::default(),
                )
                .await
                .unwrap();
            Self { f, dir, builder }
        }

        /// Baut den Kontext und markiert die Kommentare wie der Hook nach der Ausgabe als
        /// gelesen
        async fn text(
            &self,
            kind: ContextKind,
            ids: &[&str],
            session_id: Option<&str>,
        ) -> Option<String> {
            let ids: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
            let context = self.builder.build(kind, &ids, session_id).await.unwrap();
            self.builder.mark_read(&context.comment_ids).await.unwrap();
            context.text
        }
    }

    /// Der Mensch, dessen Kommentare eingespeist werden
    fn human() -> crate::board::basics::BoardActor {
        crate::board::basics::BoardActor {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            name: "Florian".into(),
        }
    }

    /// Setzt den Status eines Tickets
    async fn set_status(u: &Umgebung, id: &str, status: TicketStatus) {
        u.f.tickets
            .update(
                id,
                &TicketPatch {
                    status: Some(status),
                    ..TicketPatch::default()
                },
                &u.f.actor,
                &WriteOptions::default(),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn nennt_zum_sessionbeginn_ticket_spec_schritte_regeln_und_den_ablauf() {
        let u = Umgebung::neu().await;
        let out = u
            .text(ContextKind::SessionStart, &["ARC-0001"], None)
            .await
            .unwrap();
        assert!(out.contains("ARC-0001 „Login“ (Spec 001-x, Schritte 0/1)"));
        for part in [
            "nexus board comment ARC-0001",
            "nexus board show ARC-0001",
            "nexus board step ARC-0001",
            "nexus board done ARC-0001",
            "Ablauf im Harness",
        ] {
            assert!(out.contains(part), "{part}");
        }
        for phase in workflow_phases() {
            for step in phase.steps {
                if let Some(command) = step.command {
                    assert!(out.contains(&command), "{command}");
                }
            }
        }
        assert!(out.contains("Stand: ARC-0001 ist Backlog. Vorschlag: vor dem Umsetzen planen: Plan, Handbuch und Diagramm mit `nexus board plan-json ARC-0001 --file payload.json --by claude` schreiben (setzt Geplant); erst danach mit /nexus:harness-implement ARC-0001 umsetzen."));
        assert!(
            out.contains("Aus einem Superpowers-Plan legst du auf Wunsch je Task ein Ticket an")
        );
        assert!(out.contains("Tickets mit Planungsbedarf (etwa aus der Ticket-Auswahl der Docs-Seite) werden vor dem Umsetzen geplant: `nexus --json board next-planning --claim claude` holt das nächste"));
        assert!(!out.contains("/speckit-"));
    }

    #[tokio::test]
    async fn schlagt_fur_tickets_in_planung_den_abschluss_des_plans_vor() {
        let u = Umgebung::neu().await;
        set_status(&u, "ARC-0001", TicketStatus::InPlanning).await;
        let planning = u
            .text(ContextKind::SessionStart, &["ARC-0001"], None)
            .await
            .unwrap();
        assert!(planning.contains(
            "Stand: ARC-0001 ist In Planung. Vorschlag: den Plan mit dem Nutzer abschließen"
        ));
        assert!(planning.contains("nexus board plan-json ARC-0001"));
        set_status(&u, "ARC-0001", TicketStatus::Planned).await;
        assert!(u
            .text(ContextKind::SessionStart, &["ARC-0001"], None)
            .await
            .unwrap()
            .contains("Vorschlag: mit /nexus:harness-implement ARC-0001 umsetzen."));
    }

    #[tokio::test]
    async fn weist_ohne_ticket_auf_claim_hin() {
        let u = Umgebung::neu().await;
        let out = u.text(ContextKind::SessionStart, &[], None).await.unwrap();
        assert!(out.contains("nexus board claim"));
        assert!(out.contains("Stand: keinem Ticket zugeordnet"));
    }

    #[tokio::test]
    async fn schlagt_je_nach_stand_den_nachsten_schritt_vor() {
        let u = Umgebung::neu().await;
        set_status(&u, "ARC-0001", TicketStatus::InProgress).await;
        assert!(u
            .text(ContextKind::SessionStart, &["ARC-0001"], None)
            .await
            .unwrap()
            .contains("Stand: ARC-0001 ist In Arbeit. Vorschlag: nach der Umsetzung Review und Abschluss mit `nexus board done` vorschlagen."));
        u.f.tickets
            .move_to(
                "ARC-0001",
                TicketStatus::Done,
                &u.f.actor,
                &MoveOptions {
                    acceptance: Some("ok".into()),
                    ..MoveOptions::default()
                },
            )
            .await
            .unwrap();
        let done = u
            .text(ContextKind::SessionStart, &["ARC-0001"], None)
            .await
            .unwrap();
        assert!(done.contains("Stand: ARC-0001 ist erledigt."));
        assert!(done.contains("nexus board list --status geplant --md"));
    }

    #[tokio::test]
    async fn speist_ungelesene_kommentare_von_menschen_genau_einmal_ein() {
        let u = Umgebung::neu().await;
        u.f.comments
            .add("ARC-0001", "Bitte prüfen", &human(), false)
            .await
            .unwrap();
        u.f.comments
            .add("ARC-0001", "Eigener Stand", &u.f.actor, true)
            .await
            .unwrap();
        let first = u
            .text(ContextKind::UserPrompt, &["ARC-0001"], None)
            .await
            .unwrap();
        assert!(first.contains("Neue Kommentare zu ARC-0001:"));
        assert!(first.contains("Florian: Bitte prüfen"));
        assert!(!first.contains("Eigener Stand"));
        assert!(!first.contains("Ablauf im Harness"));
        assert_eq!(
            u.text(ContextKind::UserPrompt, &["ARC-0001"], None).await,
            None
        );
        u.f.comments
            .add("ARC-0002", "Gesammelt", &human(), false)
            .await
            .unwrap();
        assert!(u
            .text(ContextKind::SessionStart, &["ARC-0002"], None)
            .await
            .unwrap()
            .contains("Gesammelt"));
    }

    #[tokio::test]
    async fn nennt_offene_fragen_beim_start_und_beim_prompt_nur_neue_oder_geanderte() {
        let u = Umgebung::neu().await;
        let q =
            u.f.questions
                .create(
                    "ARC",
                    &NewBoardQuestionInput {
                        ticket_ids: Some(vec!["ARC-0001".into()]),
                        ..NewBoardQuestionInput::titled("Welcher Speicher?")
                    },
                    &u.f.agent,
                )
                .await
                .unwrap()
                .view
                .question
                .id
                .clone();
        u.f.questions
            .create(
                "ARC",
                &NewBoardQuestionInput {
                    ticket_ids: Some(vec!["ARC-0002".into()]),
                    ..NewBoardQuestionInput::titled("Andere Frage")
                },
                &u.f.agent,
            )
            .await
            .unwrap();
        let start = u
            .text(ContextKind::SessionStart, &["ARC-0001"], Some("s-1"))
            .await
            .unwrap();
        assert!(start.contains("Offene Fragen (1, nicht entscheiden):"));
        assert!(start.contains(&format!("- {q} Welcher Speicher? (betrifft ARC-0001)")));
        assert!(start.contains(&format!(
            "nexus board question research {q} --from <datei> --kind session"
        )));
        assert!(!start.contains("Andere Frage"));
        assert_eq!(
            u.text(ContextKind::UserPrompt, &["ARC-0001"], Some("s-1"))
                .await,
            None
        );
        u.f.advance(1);
        u.f.questions
            .update(
                &q,
                &BoardQuestionChanges {
                    title: Some("Welcher Speicher genau?".into()),
                    ..BoardQuestionChanges::default()
                },
                &u.f.human,
                &BoardQuestionWriteOptions::default(),
            )
            .await
            .unwrap();
        assert!(u
            .text(ContextKind::UserPrompt, &["ARC-0001"], Some("s-1"))
            .await
            .unwrap()
            .contains("Welcher Speicher genau?"));
        assert_eq!(
            u.text(ContextKind::UserPrompt, &["ARC-0001"], Some("s-1"))
                .await,
            None
        );
    }

    #[tokio::test]
    async fn ohne_session_kennung_nennt_der_prompt_alle_offenen_fragen() {
        let u = Umgebung::neu().await;
        u.f.questions
            .create(
                "ARC",
                &NewBoardQuestionInput {
                    ticket_ids: Some(vec!["ARC-0001".into()]),
                    ..NewBoardQuestionInput::titled("Offen?")
                },
                &u.f.agent,
            )
            .await
            .unwrap();
        assert!(u
            .text(ContextKind::UserPrompt, &["ARC-0001"], None)
            .await
            .unwrap()
            .contains("Offen?"));
    }

    #[tokio::test]
    async fn ubergeht_ein_unbekanntes_ticket_ohne_zu_scheitern() {
        let u = Umgebung::neu().await;
        let out = u
            .text(ContextKind::SessionStart, &["ARC-0099", "ARC-0001"], None)
            .await
            .unwrap();
        assert!(out.contains("ARC-0001"));
        assert!(!out.contains("ARC-0099"));
    }

    #[tokio::test]
    async fn markiert_kommentare_erst_mit_mark_read_als_gelesen() {
        let u = Umgebung::neu().await;
        u.f.comments
            .add("ARC-0001", "Bitte prüfen", &human(), false)
            .await
            .unwrap();
        let ids = vec!["ARC-0001".to_string()];
        let first = u
            .builder
            .build(ContextKind::UserPrompt, &ids, None)
            .await
            .unwrap();
        assert!(first.text.unwrap().contains("Bitte prüfen"));
        assert_eq!(first.comment_ids.len(), 1);
        let again = u
            .builder
            .build(ContextKind::UserPrompt, &ids, None)
            .await
            .unwrap();
        assert!(again.text.unwrap().contains("Bitte prüfen"));
        u.builder.mark_read(&again.comment_ids).await.unwrap();
        assert_eq!(
            u.builder
                .build(ContextKind::UserPrompt, &ids, None)
                .await
                .unwrap(),
            HookContext::default()
        );
    }

    #[tokio::test]
    async fn nennt_mehrere_ungelesene_kommentare_eines_tickets_in_zeitlicher_reihenfolge() {
        let u = Umgebung::neu().await;
        for body in ["Erster", "Zweiter", "Dritter"] {
            u.f.comments
                .add("ARC-0001", body, &human(), false)
                .await
                .unwrap();
            u.f.advance(1);
        }
        let out = u
            .text(ContextKind::UserPrompt, &["ARC-0001"], None)
            .await
            .unwrap();
        assert_eq!(out.matches("Neue Kommentare zu ARC-0001:").count(), 1);
        let order: Vec<usize> = ["Erster", "Zweiter", "Dritter"]
            .iter()
            .map(|body| {
                out.find(&format!("Florian: {body}"))
                    .expect("der Kommentar steht im Text")
            })
            .collect();
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(order, sorted);
    }

    #[tokio::test]
    async fn formuliert_den_vorschlag_ohne_befehl_wenn_planen_und_umsetzen_keine_befehle_haben() {
        let u = Umgebung::neu().await;
        let phases: Vec<WorkflowPhase> = workflow_phases()
            .into_iter()
            .map(|phase| {
                if phase.id == "planen" || phase.id == "umsetzen" {
                    WorkflowPhase {
                        steps: Vec::new(),
                        ..phase
                    }
                } else {
                    phase
                }
            })
            .collect();
        let hint = ContextBuilder::with_phases(
            u.f.db.clone(),
            TicketService::new(u.f.db.clone(), u.f.clock.clone()),
            CommentService::new(u.f.db.clone(), u.f.clock.clone()),
            None,
            u.f.clock.clone(),
            phases,
        );
        // Kein Ticket, offenes Ticket (geplant) und erledigtes Ticket
        let none = hint
            .build(ContextKind::SessionStart, &[], None)
            .await
            .unwrap()
            .text
            .unwrap();
        set_status(&u, "ARC-0002", TicketStatus::Planned).await;
        let ids = vec!["ARC-0002".to_string()];
        let open = hint
            .build(ContextKind::SessionStart, &ids, None)
            .await
            .unwrap()
            .text
            .unwrap();
        u.f.tickets
            .move_to(
                "ARC-0002",
                TicketStatus::Done,
                &u.f.actor,
                &MoveOptions {
                    force: true,
                    acceptance: Some("geprüft".into()),
                    ..MoveOptions::default()
                },
            )
            .await
            .unwrap();
        let done = hint
            .build(ContextKind::SessionStart, &ids, None)
            .await
            .unwrap()
            .text
            .unwrap();
        assert!(none.contains("eine neue Funktion planen (siehe Phase „Neue Funktion planen“)"));
        assert!(open.contains("ARC-0002 umsetzen (siehe Phase „Ticket umsetzen“)"));
        assert!(done.contains("und das Ticket umsetzen (siehe Phase „Ticket umsetzen“)"));
        for text in [&none, &open, &done] {
            assert!(!text.contains("  "), "{text}");
            assert!(!text.contains(": ;") && !text.contains(": ."), "{text}");
            assert!(!text.contains("mit  "), "{text}");
        }
    }

    #[tokio::test]
    async fn nennt_die_befehle_des_angewendeten_workflows() {
        let u = Umgebung::neu().await;
        let phases: Vec<WorkflowPhase> = workflow_phases()
            .into_iter()
            .map(|phase| {
                if phase.id == "umsetzen" {
                    let mut steps = phase.steps.clone();
                    steps.push(crate::workflow::phases::WorkflowStep {
                        step: "Review".into(),
                        command: Some("/nexus:mein-review".into()),
                        place: None,
                        when: "x".into(),
                        options: None,
                    });
                    WorkflowPhase { steps, ..phase }
                } else {
                    phase
                }
            })
            .collect();
        let out = ContextBuilder::with_phases(
            u.f.db.clone(),
            TicketService::new(u.f.db.clone(), u.f.clock.clone()),
            CommentService::new(u.f.db.clone(), u.f.clock.clone()),
            None,
            u.f.clock.clone(),
            phases,
        )
        .build(ContextKind::SessionStart, &[], None)
        .await
        .unwrap()
        .text
        .unwrap();
        assert!(out.contains("/nexus:mein-review"));
    }

    #[tokio::test]
    async fn erlaubt_schnelle_aufgaben_ohne_ablauf_nur_nach_ausdrucklicher_bestatigung() {
        let u = Umgebung::neu().await;
        for ids in [vec![], vec!["ARC-0001"]] {
            let out = u.text(ContextKind::SessionStart, &ids, None).await.unwrap();
            assert!(out.contains("Schnelle Aufgaben und Nebenaufgaben"));
            assert!(out.contains("ohne den Ablauf"));
            assert!(out.contains("ausdrücklich"));
            assert!(out.contains("„Ohne Workflow direkt umsetzen?“"));
        }
    }

    #[tokio::test]
    async fn schreibt_die_kommentar_zeitpunkte_deutsch_in_der_vorgegebenen_zone() {
        let u = Umgebung::neu().await;
        u.f.comments
            .add("ARC-0001", "Bitte prüfen", &human(), false)
            .await
            .unwrap();
        let builder = ContextBuilder::new(
            u.f.db.clone(),
            TicketService::new(u.f.db.clone(), u.f.clock.clone()),
            CommentService::new(u.f.db.clone(), u.f.clock.clone()),
            None,
            u.f.clock.clone(),
        )
        .in_zone(FixedOffset::east_opt(2 * 3600).unwrap());
        let out = builder
            .build(ContextKind::UserPrompt, &["ARC-0001".to_string()], None)
            .await
            .unwrap()
            .text
            .unwrap();
        // Die Vorlage der Umgebung steht auf 2026-10-13T10:00:00Z, hier also 12:00
        assert!(
            out.contains("- 13.10.2026, 12:00, Florian: Bitte prüfen"),
            "{out}"
        );
        // `dir` lebt bis hierher, damit die Marken-Datei nicht vorher verschwindet
        drop(u.dir);
    }
}
