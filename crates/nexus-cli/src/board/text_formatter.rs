//! Textausgaben des Boards für Menschen (Text) und Agenten (Markdown).
//! Portiert aus `packages/cli/src/board/BoardTextFormatter.ts`.
//!
//! Die Ausgaben der Fragen kommen mit den Tickets, die diese Befehle portieren; hier stehen die
//! Teile für `board list`, `board show`, `board boards` und `board note`.

use nexus_core::board::basics::{NoteState, Priority};
use nexus_core::board::board_service::BoardSummary;
use nexus_core::board::note_similarity::note_title;
use nexus_core::board::repo_paths::BoardRepoSource;
use nexus_core::board::ticket_ordering::{BoardColumn, COLUMN_ORDER};
use nexus_core::board::ticket_service::{TicketDetail, TicketView};
use nexus_core::board::types::Note;

use crate::board::comment_author_kind::CommentAuthorKind;

/// So viele Agenten-Kommentare bleiben in der kompakten Ausgabe stehen
const KEPT_AGENT_COMMENTS: usize = 3;

/// Anzeigenamen der Stände einer Notiz (`NOTE_STATE_LABELS`)
fn note_state_label(state: NoteState) -> &'static str {
    match state {
        NoteState::Open => "offen",
        NoteState::Taken => "übernommen",
        NoteState::Dropped => "verworfen",
    }
}

/// Was [`BoardTextFormatter::show_text`] zusätzlich ausgibt
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShowTextOptions {
    pub history: bool,
    pub all_comments: bool,
}

/// Textausgaben des Boards für Menschen (Text) und Agenten (Markdown).
#[derive(Debug, Clone, Copy, Default)]
pub struct BoardTextFormatter {
    author_kind: CommentAuthorKind,
}

impl BoardTextFormatter {
    /// Spaltentitel
    pub fn column_label(&self, column: BoardColumn) -> &'static str {
        column.label()
    }

    /// Boards mit Projekt, Besitzer, offenen Tickets und eigenem Repo-Pfad; `*` = aktives Board
    pub fn boards_text(&self, boards: &[BoardSummary], active: Option<i32>) -> Vec<String> {
        if boards.is_empty() {
            return vec![
                "Keine Boards. Mit „harness board boards create <projekt> <name>“ anlegen."
                    .to_string(),
            ];
        }
        boards
            .iter()
            .map(|b| {
                let repo = match b.board.repo_path.as_deref() {
                    Some(_) => format!(
                        "  {}",
                        self.repo_path_text(b.board.repo_path.as_deref(), b.repo_source)
                    ),
                    None => String::new(),
                };
                format!(
                    "{} {}  {}  {}  @{}  offen {}{repo}",
                    if Some(b.board.id) == active { "*" } else { " " },
                    b.board.id,
                    b.project_key,
                    b.board.name,
                    b.owner_name,
                    b.open_tickets
                )
            })
            .collect()
    }

    /// Eigener Repo-Pfad eines Boards; aus der DB übernommen (Altbestand) wird gekennzeichnet
    pub fn repo_path_text(&self, path: Option<&str>, source: Option<BoardRepoSource>) -> String {
        let Some(path) = path.filter(|path| !path.is_empty()) else {
            return "(keiner)".to_string();
        };
        if source == Some(BoardRepoSource::Legacy) {
            format!("{path} (aus der DB, Altbestand)")
        } else {
            path.to_string()
        }
    }

    /// Notizen als Text
    pub fn notes_text(&self, notes: &[Note]) -> Vec<String> {
        if notes.is_empty() {
            return vec!["(keine Notizen)".to_string()];
        }
        notes
            .iter()
            .map(|n| {
                format!(
                    "  {}  [{}] {}{}{}{}",
                    n.id,
                    note_state_label(n.state),
                    note_title(n),
                    if n.seen_count > 1 {
                        format!(" [{}×]", n.seen_count)
                    } else {
                        String::new()
                    },
                    match n.source.as_deref() {
                        Some(source) => format!(" (aus {source})"),
                        None => String::new(),
                    },
                    if n.labels.is_empty() {
                        String::new()
                    } else {
                        format!("  {}", n.labels.join(","))
                    }
                )
            })
            .collect()
    }

    /// Notizen als Markdown
    pub fn notes_markdown(&self, notes: &[Note]) -> Vec<String> {
        if notes.is_empty() {
            return vec!["- (keine Notizen)".to_string()];
        }
        notes
            .iter()
            .map(|n| {
                let meta: Vec<String> = [
                    note_state_label(n.state).to_string(),
                    if n.seen_count > 1 {
                        format!("{}× aufgefallen", n.seen_count)
                    } else {
                        String::new()
                    },
                    n.source
                        .as_deref()
                        .map(|source| format!("aus {source}"))
                        .unwrap_or_default(),
                    n.ticket_id
                        .as_deref()
                        .map(|id| format!("→ {id}"))
                        .unwrap_or_default(),
                    n.labels.join(","),
                ]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect();
                format!("- **{}** {} — {}", n.id, note_title(n), meta.join(" · "))
            })
            .collect()
    }

    /// Eine Notiz vollständig
    pub fn note_text(&self, n: &Note) -> Vec<String> {
        let mut lines = vec![format!(
            "Notiz {}  [{}]  {}  {}",
            n.id,
            note_state_label(n.state),
            n.created_at,
            n.author_name
        )];
        if let Some(source) = &n.source {
            lines.push(format!("Aus: {source}"));
        }
        if n.seen_count > 1 {
            lines.push(format!(
                "Aufgefallen: {}×{}",
                n.seen_count,
                if n.seen_in.is_empty() {
                    String::new()
                } else {
                    format!(" (auch bei {})", n.seen_in.join(", "))
                }
            ));
        }
        if !n.labels.is_empty() {
            lines.push(format!("Labels: {}", n.labels.join(", ")));
        }
        if let Some(ticket) = &n.ticket_id {
            lines.push(format!("Ticket: {ticket}"));
        }
        if let Some(reason) = &n.reason {
            lines.push(format!("Grund: {reason}"));
        }
        // Ältere Notizen ohne Titel: der ganze Text steht im Inhalt
        if !n.title.is_empty() {
            lines.push(String::new());
            lines.push(format!("# {}", n.title));
        }
        if !n.body.trim().is_empty() {
            lines.push(String::new());
            lines.push(n.body.trim_end().to_string());
        }
        lines
    }

    /// Liste je Spalte: „  ARC-0007  Titel  [wartet auf …] [planung] @agent (high) ui,api“
    pub fn list_text(&self, tickets: &[TicketView]) -> Vec<String> {
        if tickets.is_empty() {
            return vec!["(keine Tickets gefunden)".to_string()];
        }
        let mut lines = Vec::new();
        for column in COLUMN_ORDER {
            let in_column = Self::in_column(tickets, column);
            if in_column.is_empty() {
                continue;
            }
            lines.push(String::new());
            lines.push(format!(
                "{} ({})",
                self.column_label(column),
                in_column.len()
            ));
            for t in in_column {
                let marks = self.marks(t);
                let tail = if marks.is_empty() {
                    String::new()
                } else {
                    format!("  {}", marks.join(" "))
                };
                lines.push(format!("  {}  {}{tail}", t.ticket.id, t.ticket.title));
            }
        }
        lines
    }

    /// Markdown je Spalte mit Blockiert-Grund, Abnahme und offenen Voraussetzungen
    pub fn list_markdown(&self, heading: &str, tickets: &[TicketView]) -> Vec<String> {
        let mut lines = vec![format!("## {heading}")];
        if tickets.is_empty() {
            lines.push(String::new());
            lines.push("- (keine Tickets gefunden)".to_string());
            return lines;
        }
        for column in COLUMN_ORDER {
            let in_column = Self::in_column(tickets, column);
            if in_column.is_empty() {
                continue;
            }
            lines.push(String::new());
            lines.push(format!(
                "### {} ({})",
                self.column_label(column),
                in_column.len()
            ));
            for t in in_column {
                let meta = Self::keep([
                    if t.ticket.needs_planning {
                        "[planung]".to_string()
                    } else {
                        String::new()
                    },
                    Self::text(&t.ticket.assignee)
                        .map(|name| format!("@{name}"))
                        .unwrap_or_default(),
                    if t.ticket.priority == Priority::Normal {
                        String::new()
                    } else {
                        t.ticket.priority.as_str().to_string()
                    },
                    if t.steps_total == 0 {
                        String::new()
                    } else {
                        format!("{}/{} Schritte", t.steps_done, t.steps_total)
                    },
                    t.ticket.labels.join(","),
                ]);
                let tail = if meta.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", meta.join(" · "))
                };
                lines.push(format!("- **{}** {}{tail}", t.ticket.id, t.ticket.title));
                if !t.depends_open.is_empty() {
                    lines.push(format!("  - Wartet auf: {}", t.depends_open.join(", ")));
                }
                if !t.waiting_on_questions.is_empty() {
                    lines.push(format!(
                        "  - Wartet auf Frage: {}",
                        Self::questions(t).join("; ")
                    ));
                }
                if let Some(reason) = Self::text(&t.ticket.blocked_reason) {
                    lines.push(format!("  - Blockiert: {reason}"));
                }
                if !t.ticket.acceptance.is_empty() {
                    lines.push(format!("  - Abnahme: {}", t.ticket.acceptance));
                }
            }
        }
        lines
    }

    /// Ein Ticket mit allen Texten; Verlauf und ältere Agenten-Kommentare nur auf Wunsch
    pub fn show_text(&self, t: &TicketDetail, options: ShowTextOptions) -> Vec<String> {
        let mut head = vec![
            format!(
                "Status {}{}",
                t.view.ticket.status,
                if t.view.waiting { " (wartet)" } else { "" }
            ),
            format!("Prio {}", t.view.ticket.priority),
            format!("rev {}", t.view.ticket.rev),
            match Self::text(&t.view.ticket.assignee) {
                Some(name) => format!("@{name}"),
                None => "nicht zugewiesen".to_string(),
            },
        ];
        if t.view.ticket.needs_planning {
            head.push("braucht Planung".to_string());
        }
        if !t.view.ticket.labels.is_empty() {
            head.push(t.view.ticket.labels.join(","));
        }
        let mut lines = vec![
            format!("{}  {}", t.view.ticket.id, t.view.ticket.title),
            head.join(" | "),
        ];
        if let Some(reason) = Self::text(&t.view.ticket.blocked_reason) {
            lines.push(format!("Blockiert: {reason}"));
        }
        if !t.view.ticket.acceptance.is_empty() {
            lines.push(format!("Abnahme: {}", t.view.ticket.acceptance));
        }
        if !t.view.ticket.depends_on.is_empty() {
            let texts: Vec<String> = t
                .view
                .ticket
                .depends_on
                .iter()
                .map(|d| {
                    let state = if t.view.depends_open.contains(d) {
                        "offen"
                    } else {
                        "erfüllt"
                    };
                    format!("{d} ({state})")
                })
                .collect();
            lines.push(format!("Setzt voraus: {}", texts.join(", ")));
        }
        if !t.view.dependents.is_empty() {
            lines.push(format!(
                "Voraussetzung für: {}",
                t.view.dependents.join(", ")
            ));
        }
        if !t.view.waiting_on_questions.is_empty() {
            lines.push(format!(
                "Wartet auf Frage: {}",
                Self::questions(&t.view).join("; ")
            ));
        }
        if let Some(spec) = Self::text(&t.view.ticket.spec) {
            let task = Self::text(&t.view.ticket.spec_task)
                .map(|task| format!(" / {task}"))
                .unwrap_or_default();
            lines.push(format!("Spec: {spec}{task}"));
        }
        if let Some(target) = Self::text(&t.view.ticket.target_branch) {
            lines.push(format!("Zielbranch: {target}"));
        }
        if let Some(branch) = Self::text(&t.view.ticket.branch) {
            let worktree = Self::text(&t.view.ticket.worktree)
                .map(|w| format!(" (Worktree {w})"))
                .unwrap_or_default();
            lines.push(format!("Branch: {branch}{worktree}"));
        }
        if !t.view.ticket.touches.is_empty() {
            lines.push(format!("Bereiche: {}", t.view.ticket.touches.join(", ")));
        }
        if !t.view.ticket.consumes.is_empty() {
            lines.push(format!("Nutzt: {}", t.view.ticket.consumes.join("; ")));
        }
        if !t.view.ticket.produces.is_empty() {
            lines.push(format!("Liefert: {}", t.view.ticket.produces.join("; ")));
        }
        if !t.view.sessions.is_empty() {
            lines.push(format!("Sessions: {}", t.view.sessions.join(", ")));
        }
        let sections = [
            ("Beschreibung", &t.view.ticket.description),
            ("Plan", &t.view.ticket.plan),
            ("Handbuch", &t.view.ticket.handbook),
            ("Diagramm (mermaid)", &t.view.ticket.diagram),
        ];
        for (title, text) in sections {
            if !text.trim().is_empty() {
                lines.push(String::new());
                lines.push(format!("## {title}"));
                lines.push(text.trim_end().to_string());
            }
        }
        if !t.steps.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "## Schritte ({}/{})",
                t.view.steps_done, t.view.steps_total
            ));
            for (index, step) in t.steps.iter().enumerate() {
                lines.push(format!(
                    "  [{}] {}. {}",
                    if step.done { "x" } else { " " },
                    index + 1,
                    step.text
                ));
                if let Some(detail) = &step.detail {
                    lines.extend(detail.split('\n').map(|line| format!("        {line}")));
                }
            }
        }
        lines.push(String::new());
        lines.push("## Kommentare".to_string());
        lines.extend(self.comment_lines(t, options.all_comments));
        if options.history {
            lines.push(String::new());
            lines.push("## Verlauf".to_string());
            for h in &t.history {
                let note = h
                    .note
                    .as_ref()
                    .map(|note| format!(" ({note})"))
                    .unwrap_or_default();
                lines.push(format!(
                    "  {} {} {}: {} → {}{note}",
                    h.at,
                    h.by_name,
                    h.field,
                    h.from.as_deref().unwrap_or("–"),
                    h.to.as_deref().unwrap_or("–")
                ));
            }
        } else if !t.history.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "Verlauf: {} {} – --history zeigt sie",
                t.history.len(),
                if t.history.len() == 1 {
                    "Eintrag"
                } else {
                    "Einträge"
                }
            ));
        }
        lines
    }

    /// Kommentare; ohne `all` bleiben von den Agenten nur die letzten drei, Menschen immer
    fn comment_lines(&self, t: &TicketDetail, all: bool) -> Vec<String> {
        if t.comments.is_empty() {
            return vec!["(keine)".to_string()];
        }
        let agent_indexes: Vec<usize> = t
            .comments
            .iter()
            .enumerate()
            .filter(|(_, c)| self.author_kind.is_agent(&c.author_name))
            .map(|(index, _)| index)
            .collect();
        let hidden: &[usize] = if all {
            &[]
        } else {
            let keep_from = agent_indexes.len().saturating_sub(KEPT_AGENT_COMMENTS);
            &agent_indexes[..keep_from]
        };
        let mut lines = Vec::new();
        if !hidden.is_empty() {
            lines.push(format!(
                "  {} ältere Agenten-Kommentare ausgeblendet – --all-comments zeigt sie",
                hidden.len()
            ));
        }
        lines.extend(
            t.comments
                .iter()
                .enumerate()
                .filter(|(index, _)| !hidden.contains(index))
                .map(|(_, c)| format!("  {} {}: {}", c.created_at, c.author_name, c.body)),
        );
        lines
    }

    /// Merkmale eines Tickets in der Textliste
    fn marks(&self, t: &TicketView) -> Vec<String> {
        let mut marks = Vec::new();
        if t.waiting {
            marks.push(format!("[wartet auf {}]", t.depends_open.join(", ")));
        }
        if !t.waiting_on_questions.is_empty() {
            let ids: Vec<&str> = t
                .waiting_on_questions
                .iter()
                .map(|q| q.id.as_str())
                .collect();
            marks.push(format!("[Frage {}]", ids.join(", ")));
        }
        if t.ticket.needs_planning {
            marks.push("[planung]".to_string());
        }
        if let Some(assignee) = Self::text(&t.ticket.assignee) {
            marks.push(format!("@{assignee}"));
        }
        if t.ticket.priority != Priority::Normal {
            marks.push(format!("({})", t.ticket.priority));
        }
        if t.steps_total != 0 {
            marks.push(format!("[{}/{} Schritte]", t.steps_done, t.steps_total));
        }
        if !t.ticket.labels.is_empty() {
            marks.push(t.ticket.labels.join(","));
        }
        marks
    }

    /// Offene Fragen eines Tickets als „ID Titel“
    fn questions(t: &TicketView) -> Vec<String> {
        t.waiting_on_questions
            .iter()
            .map(|q| format!("{} {}", q.id, q.title))
            .collect()
    }

    /// Die Tickets einer Spalte in der Reihenfolge der Liste
    fn in_column(tickets: &[TicketView], column: BoardColumn) -> Vec<&TicketView> {
        tickets.iter().filter(|t| t.column == column).collect()
    }

    /// Ein gesetztes Feld, das nicht leer ist (`if (t.feld)` in TypeScript)
    fn text(value: &Option<String>) -> Option<&str> {
        value.as_deref().filter(|text| !text.is_empty())
    }

    /// Leere Angaben fallen weg (`.filter(Boolean)` in TypeScript)
    fn keep<const N: usize>(parts: [String; N]) -> Vec<String> {
        parts.into_iter().filter(|part| !part.is_empty()).collect()
    }
}

#[cfg(test)]
mod tests {
    use nexus_core::board::ticket_service::TicketPatch;
    use nexus_core::board::types::Comment;
    use nexus_core::testing::BoardFixture;

    use super::*;

    /// Ein Kommentar, von dem nur Zeitpunkt, Autor und Text zählen
    fn kommentar(created_at: &str, author: &str, body: &str) -> Comment {
        Comment {
            id: 0,
            ticket_id: "AIH-0070".into(),
            author_id: String::new(),
            author_name: author.into(),
            body: body.into(),
            created_at: created_at.into(),
            read_by_agent_at: None,
        }
    }

    /// Ein Ticket mit einem Mensch-Kommentar zu Beginn, fünf Agenten-Kommentaren und zehn
    /// Verlaufseinträgen
    async fn detail() -> TicketDetail {
        let f = BoardFixture::create().await;
        f.tickets
            .create(
                f.board_id,
                &nexus_core::board::ticket_service::NewTicketInput::titled("Kompakte Ausgabe"),
                &f.actor,
                &Default::default(),
            )
            .await
            .unwrap();
        // Zehn Verlaufseinträge: der des Anlegens und neun Statuswechsel
        for index in 0..9 {
            let status = if index % 2 == 0 {
                nexus_core::board::basics::TicketStatus::InProgress
            } else {
                nexus_core::board::basics::TicketStatus::Planned
            };
            f.tickets
                .update(
                    "ARC-0001",
                    &TicketPatch {
                        status: Some(status),
                        ..TicketPatch::default()
                    },
                    &f.actor,
                    &Default::default(),
                )
                .await
                .unwrap();
        }
        let mut detail = f.tickets.get("ARC-0001").await.unwrap();
        detail.comments =
            std::iter::once(kommentar("2026-01-01", "Florian", "Bitte kompakt halten"))
                .chain((1..=5).map(|i| {
                    kommentar(
                        &format!("2026-01-0{}", i + 1),
                        "junior-AIH-0070",
                        &format!("Bericht {i}"),
                    )
                }))
                .collect();
        detail
    }

    #[tokio::test]
    async fn lasst_ohne_optionen_den_verlauf_und_altere_agenten_kommentare_weg() {
        let lines =
            BoardTextFormatter::default().show_text(&detail().await, ShowTextOptions::default());

        assert!(!lines.contains(&"## Verlauf".to_string()));
        assert!(lines.contains(&"Verlauf: 10 Einträge – --history zeigt sie".to_string()));
        assert!(lines.contains(
            &"  2 ältere Agenten-Kommentare ausgeblendet – --all-comments zeigt sie".to_string()
        ));
        assert!(lines.contains(&"  2026-01-01 Florian: Bitte kompakt halten".to_string()));
        let text = lines.join("\n");
        assert!(!text.contains("Bericht 1"), "{text}");
        assert!(!text.contains("Bericht 2"), "{text}");
        for n in [3, 4, 5] {
            assert!(
                lines.contains(&format!(
                    "  2026-01-0{} junior-AIH-0070: Bericht {n}",
                    n + 1
                )),
                "{text}"
            );
        }
    }

    #[tokio::test]
    async fn gibt_mit_history_und_all_comments_alles_aus() {
        let lines = BoardTextFormatter::default().show_text(
            &detail().await,
            ShowTextOptions {
                history: true,
                all_comments: true,
            },
        );

        assert!(lines.contains(&"## Verlauf".to_string()));
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.contains("status:") && line.contains("→"))
                .count(),
            10
        );
        let text = lines.join("\n");
        assert!(text.contains("Bericht 1"), "{text}");
        assert!(!text.contains("ausgeblendet"), "{text}");
        assert!(!text.contains("--history zeigt sie"), "{text}");
    }

    #[tokio::test]
    async fn zeigt_ohne_kommentare_und_verlauf_keine_hinweiszeilen() {
        let mut t = detail().await;
        t.comments.clear();
        t.history.clear();

        let lines = BoardTextFormatter::default().show_text(&t, ShowTextOptions::default());

        assert!(lines.contains(&"(keine)".to_string()));
        let text = lines.join("\n");
        assert!(!text.contains("ausgeblendet"), "{text}");
        assert!(!text.contains("Verlauf:"), "{text}");
    }

    #[tokio::test]
    async fn blendet_nichts_aus_wenn_es_hochstens_drei_agenten_kommentare_gibt() {
        let mut t = detail().await;
        t.comments = vec![
            kommentar("2026-01-01", "Florian", "Hallo"),
            kommentar("2026-01-02", "claude", "A"),
            kommentar("2026-01-03", "dispatcher", "B"),
        ];

        let text = BoardTextFormatter::default()
            .show_text(&t, ShowTextOptions::default())
            .join("\n");

        assert!(!text.contains("ausgeblendet"), "{text}");
        assert!(text.contains("A"), "{text}");
    }

    #[tokio::test]
    async fn zeigt_den_zielbranch_vor_dem_branch_des_tickets() {
        let f = BoardFixture::create().await;
        f.tickets
            .create(
                f.board_id,
                &nexus_core::board::ticket_service::NewTicketInput {
                    target_branch: Some("release/1".into()),
                    ..nexus_core::board::ticket_service::NewTicketInput::titled("A")
                },
                &f.actor,
                &Default::default(),
            )
            .await
            .unwrap();
        f.tickets
            .update(
                "ARC-0001",
                &TicketPatch {
                    branch: Some(Some("ticket/ARC-0001-a".into())),
                    worktree: Some(Some("C:/w/ARC-0001".into())),
                    ..TicketPatch::default()
                },
                &f.actor,
                &Default::default(),
            )
            .await
            .unwrap();

        let lines = BoardTextFormatter::default().show_text(
            &f.tickets.get("ARC-0001").await.unwrap(),
            ShowTextOptions::default(),
        );

        let target = lines
            .iter()
            .position(|line| line == "Zielbranch: release/1")
            .expect("die Zeile des Zielbranchs");
        assert_eq!(
            lines[target + 1],
            "Branch: ticket/ARC-0001-a (Worktree C:/w/ARC-0001)"
        );
    }

    #[tokio::test]
    async fn zeigt_den_zielbranch_auch_ohne_branch_und_lasst_die_zeile_ohne_zielbranch_weg() {
        let f = BoardFixture::create().await;
        f.tickets
            .create(
                f.board_id,
                &nexus_core::board::ticket_service::NewTicketInput {
                    target_branch: Some("release/1".into()),
                    ..nexus_core::board::ticket_service::NewTicketInput::titled("A")
                },
                &f.actor,
                &Default::default(),
            )
            .await
            .unwrap();
        f.tickets
            .create(
                f.board_id,
                &nexus_core::board::ticket_service::NewTicketInput::titled("B"),
                &f.actor,
                &Default::default(),
            )
            .await
            .unwrap();

        let format = BoardTextFormatter::default();
        assert!(format
            .show_text(
                &f.tickets.get("ARC-0001").await.unwrap(),
                ShowTextOptions::default()
            )
            .contains(&"Zielbranch: release/1".to_string()));
        assert!(!format
            .show_text(
                &f.tickets.get("ARC-0002").await.unwrap(),
                ShowTextOptions::default()
            )
            .iter()
            .any(|line| line.starts_with("Zielbranch")));
    }
}
