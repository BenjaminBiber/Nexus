//! Textausgaben der Fragen für Menschen (Text) und Agenten (Markdown).
//! Portiert aus `packages/cli/src/board/BoardQuestionFormatter.ts`.

use nexus_core::board::question_service::BoardQuestionListItem;
use nexus_core::board::{BoardQuestionDetail, BoardQuestionView};
use nexus_core::dispatch::autopilot::is_autopilot_decision;
use nexus_core::AuthorKind;

/// Deutscher Name der Art eines Handelnden (`KIND` in TypeScript)
fn kind(kind: AuthorKind) -> &'static str {
    match kind {
        AuthorKind::Human => "Mensch",
        AuthorKind::Session => "Session",
        AuthorKind::Subagent => "Subagent",
    }
}

/// Eine Aufzählung oder „–“, wenn sie leer ist
fn list(items: &[String]) -> String {
    if items.is_empty() {
        "–".to_string()
    } else {
        items.join(", ")
    }
}

/// Zahl der Optionen der neuesten Recherche als Text („1 Option“, „2 Optionen“)
fn options(count: i32) -> String {
    if count == 1 {
        "1 Option".to_string()
    } else {
        format!("{count} Optionen")
    }
}

/// Textausgaben der Fragen.
#[derive(Debug, Clone, Copy, Default)]
pub struct QuestionFormatter;

impl QuestionFormatter {
    /// Eine Zeile je Frage: „  ARC-F0001  Titel  [bereit] [wartet auf …] [3 Optionen] → ARC-0007“
    pub fn list_text(&self, questions: &[BoardQuestionListItem]) -> Vec<String> {
        if questions.is_empty() {
            return vec!["(keine Fragen gefunden)".to_string()];
        }
        questions
            .iter()
            .map(|item| {
                let marks = self.marks(&item.view);
                let q = &item.view.question;
                if marks.is_empty() {
                    format!("  {}  {}", q.id, q.title)
                } else {
                    format!("  {}  {}  {}", q.id, q.title, marks.join(" "))
                }
            })
            .collect()
    }

    /// Markdown mit Zustand, betroffenen Tickets, offenen Abhängigkeiten und Recherche
    pub fn list_markdown(&self, questions: &[BoardQuestionListItem]) -> Vec<String> {
        let mut lines = vec!["## Fragen".to_string()];
        if questions.is_empty() {
            lines.push(String::new());
            lines.push("- (keine Fragen gefunden)".to_string());
            return lines;
        }
        lines.push(String::new());
        for item in questions {
            let view = &item.view;
            let q = &view.question;
            lines.push(format!(
                "- **{}** {} — {}{}",
                q.id,
                q.title,
                q.state.label(),
                if view.ready {
                    ", bereit zur Entscheidung"
                } else {
                    ""
                }
            ));
            if !q.ticket_ids.is_empty() {
                lines.push(format!("  - Betrifft: {}", q.ticket_ids.join(", ")));
            }
            if !view.waiting_on.is_empty() {
                lines.push(format!(
                    "  - Wartet auf: {}",
                    view.waiting_on
                        .iter()
                        .map(|w| w.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if view.researched {
                lines.push(format!(
                    "  - Neueste Recherche: {}",
                    options(view.option_count)
                ));
            }
        }
        lines
    }

    /// Eine Frage mit Beschreibung, Recherchen, Entscheidung, Kommentaren und Verlauf
    pub fn show_text(&self, detail: &BoardQuestionDetail) -> Vec<String> {
        let view = &detail.view;
        let q = &view.question;
        let description = q.description.trim_end();
        let mut lines = vec![
            format!("{}  {}", q.id, q.title),
            format!(
                "Zustand: {}{} | rev {}",
                q.state.label(),
                if view.ready {
                    " (bereit zur Entscheidung)"
                } else {
                    ""
                },
                q.rev
            ),
            format!("Tags: {}", list(&q.tags)),
            format!(
                "Herkunft: {} ({}){}",
                q.origin_name,
                kind(q.origin_kind),
                match &q.origin_session {
                    Some(session) => format!(", Session {session}"),
                    None => String::new(),
                }
            ),
            format!("Anlass: {}", q.occasion.as_deref().unwrap_or("–")),
            format!(
                "Betrifft: {}",
                list(
                    &view
                        .tickets
                        .iter()
                        .map(|t| format!("{} {}", t.id, t.title))
                        .collect::<Vec<_>>()
                )
            ),
            format!("Abhängig von: {}", list(&q.depends_on)),
            format!(
                "Wartet auf: {}",
                list(
                    &view
                        .waiting_on
                        .iter()
                        .map(|w| format!("{} ({})", w.id, w.state))
                        .collect::<Vec<_>>()
                )
            ),
            String::new(),
            if description.is_empty() {
                "(keine Beschreibung)".to_string()
            } else {
                description.to_string()
            },
            String::new(),
        ];
        if detail.research.is_empty() {
            lines.push("Recherchen: noch keine".to_string());
            lines.push(String::new());
        } else {
            lines.push("Recherchen:".to_string());
            for research in &detail.research {
                lines.push(format!(
                    "  [{}] {} ({})",
                    research.at,
                    research.by_name,
                    kind(research.kind)
                ));
                if !research.context.trim().is_empty() {
                    lines.extend(
                        research
                            .context
                            .trim_end()
                            .split('\n')
                            .map(|line| format!("    {line}")),
                    );
                }
                for option in &research.options {
                    lines.push(format!(
                        "    {}{}: {} – {}",
                        option.id,
                        if option.recommended {
                            " (empfohlen)"
                        } else {
                            ""
                        },
                        option.title,
                        option.text
                    ));
                    lines.extend(option.pros.iter().map(|pro| format!("      + {pro}")));
                    lines.extend(option.cons.iter().map(|con| format!("      - {con}")));
                }
            }
            lines.push(String::new());
        }
        if let Some(decision) = &q.decision {
            lines.push(format!(
                "{}: {}{}",
                match decision.outcome {
                    nexus_core::board::BoardQuestionDecisionOutcome::Answered => "Entscheidung",
                    nexus_core::board::BoardQuestionDecisionOutcome::Discarded => "Verworfen",
                },
                decision
                    .option
                    .as_deref()
                    .or(decision.text.as_deref())
                    .unwrap_or("–"),
                if is_autopilot_decision(decision) {
                    " (vom Autopilot entschieden)"
                } else {
                    ""
                }
            ));
            lines.push(format!(
                "  Begründung: {} ({}, {})",
                decision.reason, decision.by_name, decision.at
            ));
            lines.push(String::new());
        }
        lines.push("Kommentare:".to_string());
        if detail.comments.is_empty() {
            lines.push("  (keine)".to_string());
        }
        for comment in &detail.comments {
            lines.push(format!(
                "  [{}] {} ({}): {}",
                comment.created_at,
                comment.author_name,
                kind(comment.author_kind),
                comment.body
            ));
        }
        lines.push(String::new());
        lines.push("Verlauf:".to_string());
        for entry in &detail.history {
            lines.push(format!(
                "  [{}] {}{} {}: {}",
                entry.at,
                entry.by_name,
                match entry.by_kind {
                    Some(by_kind) => format!(" ({})", kind(by_kind)),
                    None => String::new(),
                },
                entry.entry_type,
                entry.text
            ));
        }
        lines
    }

    /// Kennzeichen einer Frage in der Textliste
    fn marks(&self, view: &BoardQuestionView) -> Vec<String> {
        let q = &view.question;
        let mut marks = Vec::new();
        if q.state != nexus_core::questions::QuestionState::Open {
            marks.push(format!("[{}]", q.state.label()));
        }
        if view.ready {
            marks.push("[bereit]".to_string());
        }
        if !view.waiting_on.is_empty() {
            marks.push(format!(
                "[wartet auf {}]",
                view.waiting_on
                    .iter()
                    .map(|w| w.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if view.researched {
            marks.push(format!("[{}]", options(view.option_count)));
        }
        if !q.ticket_ids.is_empty() {
            marks.push(format!("→ {}", q.ticket_ids.join(", ")));
        }
        marks
    }
}

#[cfg(test)]
mod tests {
    use nexus_core::board::{
        BoardQuestionComment, BoardQuestionHistoryEntry, BoardQuestionHistoryType,
    };

    use super::*;

    /// Kleinste Frage, die `show_text` ausgeben kann; der Verlauf kommt vom Test
    fn detail(history: Vec<BoardQuestionHistoryEntry>) -> BoardQuestionDetail {
        BoardQuestionDetail {
            view: view(),
            research: Vec::new(),
            comments: Vec::new(),
            history,
        }
    }

    /// Eine offene Frage ohne Beschreibung, Tickets und Recherche
    fn view() -> BoardQuestionView {
        BoardQuestionView {
            question: nexus_core::board::BoardQuestion {
                id: "ARC-F0001".into(),
                project_id: 1,
                number: 1,
                title: "Welcher Speicher?".into(),
                description: String::new(),
                state: nexus_core::questions::QuestionState::Open,
                occasion: None,
                origin_kind: AuthorKind::Session,
                origin_name: "claude".into(),
                origin_session: None,
                tags: Vec::new(),
                ticket_ids: Vec::new(),
                depends_on: Vec::new(),
                decision: None,
                created_at: "2026-01-01".into(),
                created_by_id: String::new(),
                updated_at: "2026-01-01".into(),
                rev: 1,
            },
            project_key: "ARC".into(),
            tickets: Vec::new(),
            waiting_on: Vec::new(),
            ready: false,
            researched: false,
            option_count: 0,
        }
    }

    /// Ein Verlaufseintrag „Frage angelegt“ dieses Autors
    fn eintrag(by_kind: Option<AuthorKind>, by_name: &str) -> BoardQuestionHistoryEntry {
        BoardQuestionHistoryEntry {
            question_id: "ARC-F0001".into(),
            at: "2026-01-01".into(),
            by_id: String::new(),
            by_name: by_name.into(),
            by_kind,
            entry_type: BoardQuestionHistoryType::Created,
            text: "Frage angelegt".into(),
        }
    }

    #[test]
    fn zeigt_im_verlauf_die_deutsche_art_und_ohne_art_nur_den_namen() {
        let lines = QuestionFormatter.show_text(&detail(vec![
            eintrag(Some(AuthorKind::Human), "Florian"),
            eintrag(Some(AuthorKind::Subagent), "Bot"),
            eintrag(None, "System"),
        ]));

        assert!(lines.contains(&"  [2026-01-01] Florian (Mensch) created: Frage angelegt".into()));
        assert!(lines.contains(&"  [2026-01-01] Bot (Subagent) created: Frage angelegt".into()));
        assert!(lines.contains(&"  [2026-01-01] System created: Frage angelegt".into()));
    }

    #[test]
    fn ohne_beschreibung_und_ohne_kommentare_steht_der_ersatztext() {
        let lines = QuestionFormatter.show_text(&detail(Vec::new()));

        assert!(lines.contains(&"(keine Beschreibung)".to_string()));
        assert!(lines.contains(&"Recherchen: noch keine".to_string()));
        assert!(lines.contains(&"  (keine)".to_string()));
        assert!(lines.contains(&"Anlass: –".to_string()));
        assert!(lines.contains(&"Herkunft: claude (Session)".to_string()));
    }

    #[test]
    fn die_leere_liste_meldet_dass_nichts_gefunden_wurde() {
        assert_eq!(
            QuestionFormatter.list_text(&[]),
            vec!["(keine Fragen gefunden)".to_string()]
        );
        assert_eq!(
            QuestionFormatter.list_markdown(&[]),
            vec![
                "## Fragen".to_string(),
                String::new(),
                "- (keine Fragen gefunden)".to_string()
            ]
        );
    }

    #[test]
    fn die_textliste_kennzeichnet_zustand_recherche_und_tickets() {
        let mut item = BoardQuestionListItem {
            view: view(),
            mine: false,
        };
        item.view.question.state = nexus_core::questions::QuestionState::Discarded;
        item.view.question.ticket_ids = vec!["ARC-0001".into()];
        item.view.researched = true;
        item.view.option_count = 1;

        assert_eq!(
            QuestionFormatter.list_text(std::slice::from_ref(&item)),
            vec!["  ARC-F0001  Welcher Speicher?  [verworfen] [1 Option] → ARC-0001".to_string()]
        );
        assert_eq!(
            QuestionFormatter.list_markdown(&[item]),
            vec![
                "## Fragen".to_string(),
                String::new(),
                "- **ARC-F0001** Welcher Speicher? — verworfen".to_string(),
                "  - Betrifft: ARC-0001".to_string(),
                "  - Neueste Recherche: 1 Option".to_string(),
            ]
        );
    }

    #[test]
    fn eine_frage_ohne_kennzeichen_steht_allein_in_der_zeile() {
        let item = BoardQuestionListItem {
            view: view(),
            mine: false,
        };

        assert_eq!(
            QuestionFormatter.list_text(&[item]),
            vec!["  ARC-F0001  Welcher Speicher?".to_string()]
        );
    }

    #[test]
    fn kommentare_und_entscheidung_stehen_mit_art_und_begrundung() {
        let mut d = detail(Vec::new());
        d.view.question.decision = Some(nexus_core::board::BoardQuestionDecision {
            outcome: nexus_core::board::BoardQuestionDecisionOutcome::Answered,
            option: Some("O1".into()),
            text: None,
            reason: "Passt zum Betrieb".into(),
            by_id: String::new(),
            by_name: "Florian".into(),
            at: "2026-01-02".into(),
        });
        d.comments = vec![BoardQuestionComment {
            id: 1,
            question_id: "ARC-F0001".into(),
            author_id: String::new(),
            author_name: "claude".into(),
            author_kind: AuthorKind::Session,
            body: "Neuer Befund".into(),
            created_at: "2026-01-02".into(),
        }];

        let lines = QuestionFormatter.show_text(&d);

        assert!(lines.contains(&"Entscheidung: O1".to_string()));
        assert!(
            lines.contains(&"  Begründung: Passt zum Betrieb (Florian, 2026-01-02)".to_string())
        );
        assert!(lines.contains(&"  [2026-01-02] claude (Session): Neuer Befund".to_string()));
    }

    #[test]
    fn entscheidung_des_autopilots_ist_gekennzeichnet() {
        let mut d = detail(Vec::new());
        d.view.question.decision = Some(nexus_core::board::BoardQuestionDecision {
            outcome: nexus_core::board::BoardQuestionDecisionOutcome::Answered,
            option: Some("O2".into()),
            text: None,
            reason: "Autopilot: Empfehlung der Recherche übernommen".into(),
            by_id: String::new(),
            by_name: "dispatcher".into(),
            at: "2026-01-02".into(),
        });

        let lines = QuestionFormatter.show_text(&d);

        assert!(lines.contains(&"Entscheidung: O2 (vom Autopilot entschieden)".to_string()));
    }
}
