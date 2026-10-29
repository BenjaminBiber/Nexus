//! Der Bericht des Schattenbetriebs: zählt die Zeilen eines Zeitraums und zeigt jede
//! Abweichung mit Node- und Rust-Wert. `nexus dispatch-compare` gibt ihn als Markdown aus,
//! mit `--json` dieselben Zahlen als JSON.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::dispatch::shadow::types::{ShadowDecision, ShadowEvaluation, ShadowReadiness};

/// So lang darf ein Wert in der Tabelle sein, bevor er gekürzt wird
pub const MAX_VALUE: usize = 160;

/// Die Zahlen eines Zeitraums
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ShadowSummary {
    /// Läufe, zu denen eine Entscheidung verglichen wurde
    pub runs: usize,
    pub decisions_equal: usize,
    pub decisions_different: usize,
    /// Entscheidungen ohne Urteil (Ticket weg, Repo fehlt …)
    pub decisions_skipped: usize,
    pub evaluations_equal: usize,
    pub evaluations_different: usize,
    /// Auswertungen, zu denen keine Zeile in `board.DispatchRuns` erschienen ist
    pub evaluations_missing: usize,
    pub readiness_different: usize,
    /// Davon solche, bei denen sich das Ticket kurz vorher geändert hat
    pub readiness_timing: usize,
}

/// Bericht über einen Zeitraum
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct ShadowReport {
    /// Erster Tag des Zeitraums (`JJJJ-MM-TT`); `None` = ohne Grenze
    pub from: Option<String>,
    /// Letzter Tag des Zeitraums; `None` = ohne Grenze
    pub until: Option<String>,
    pub summary: ShadowSummary,
    /// Nur die abweichenden und die ohne Urteil
    pub decisions: Vec<ShadowDecision>,
    /// Nur die abweichenden und die ohne Zeile
    pub evaluations: Vec<ShadowEvaluation>,
    pub readiness: Vec<ShadowReadiness>,
}

impl ShadowReport {
    /// Zählt die Zeilen und behält, was zu zeigen ist
    pub fn build(
        from: Option<String>,
        until: Option<String>,
        decisions: Vec<ShadowDecision>,
        evaluations: Vec<ShadowEvaluation>,
        readiness: Vec<ShadowReadiness>,
    ) -> Self {
        let mut summary = ShadowSummary {
            runs: decisions.len(),
            ..ShadowSummary::default()
        };
        for line in &decisions {
            if line.skipped.is_some() {
                summary.decisions_skipped += 1;
            } else if line.equal {
                summary.decisions_equal += 1;
            } else {
                summary.decisions_different += 1;
            }
        }
        for line in &evaluations {
            if line.missing_row {
                summary.evaluations_missing += 1;
            } else if line.equal {
                summary.evaluations_equal += 1;
            } else {
                summary.evaluations_different += 1;
            }
        }
        summary.readiness_different = readiness.len();
        summary.readiness_timing = readiness.iter().filter(|line| line.timing).count();
        Self {
            from,
            until,
            summary,
            decisions: decisions
                .into_iter()
                .filter(|line| !line.equal || line.skipped.is_some())
                .collect(),
            evaluations: evaluations
                .into_iter()
                .filter(|line| !line.equal || line.missing_row)
                .collect(),
            readiness,
        }
    }

    /// Der Bericht als Markdown
    pub fn markdown(&self) -> String {
        let mut out = String::from("# Schattenbetrieb: Node gegen Rust\n\n");
        out.push_str(&format!("Zeitraum: {}\n\n", self.period()));
        out.push_str("## Zusammenfassung\n\n");
        let summary = &self.summary;
        out.push_str(&format!("- Läufe: {}\n", summary.runs));
        out.push_str(&format!(
            "- Entscheidungen: {} gleich, {} abweichend, {} ohne Urteil\n",
            summary.decisions_equal, summary.decisions_different, summary.decisions_skipped
        ));
        out.push_str(&format!(
            "- Auswertungen: {} gleich, {} abweichend, {} ohne Zeile\n",
            summary.evaluations_equal, summary.evaluations_different, summary.evaluations_missing
        ));
        out.push_str(&format!(
            "- Bereitschaft: {} Abweichungen (davon {} zeitnah)\n",
            summary.readiness_different, summary.readiness_timing
        ));
        self.decision_section(&mut out);
        self.evaluation_section(&mut out);
        self.readiness_section(&mut out);
        out
    }

    /// Der Zeitraum, wie der Kopf ihn nennt
    fn period(&self) -> String {
        match (&self.from, &self.until) {
            (Some(from), Some(until)) => format!("{from} bis {until}"),
            (Some(from), None) => format!("ab {from}"),
            (None, Some(until)) => format!("bis {until}"),
            (None, None) => "alle Tage".to_string(),
        }
    }

    fn decision_section(&self, out: &mut String) {
        if self.decisions.is_empty() {
            return;
        }
        out.push_str("\n## Abweichende Entscheidungen\n");
        for line in &self.decisions {
            out.push_str(&format!(
                "\n### {} {} ({})\n\n",
                date_of(&line.at),
                line.ticket_id,
                line.kind
            ));
            if let Some(reason) = &line.skipped {
                out.push_str(&format!("Ohne Urteil: {reason}\n"));
                continue;
            }
            table(out, &line.diffs);
            if let Some(diff) = &line.prompt_diff {
                out.push_str("\n<details><summary>Arbeitsanweisung</summary>\n\n```diff\n");
                out.push_str(diff);
                if !diff.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("```\n\n</details>\n");
            }
        }
    }

    fn evaluation_section(&self, out: &mut String) {
        if self.evaluations.is_empty() {
            return;
        }
        out.push_str("\n## Abweichende Auswertungen\n");
        for line in &self.evaluations {
            out.push_str(&format!(
                "\n### {} {} ({})\n\n",
                date_of(&line.at),
                line.ticket_id,
                line.kind
            ));
            if line.missing_row {
                out.push_str(&format!(
                    "Keine Zeile in board.DispatchRuns zum Lauf {}.\n",
                    line.run_id
                ));
                continue;
            }
            table(out, &line.diffs);
        }
    }

    fn readiness_section(&self, out: &mut String) {
        if self.readiness.is_empty() {
            return;
        }
        out.push_str("\n## Abweichende Bereitschaft\n\n");
        out.push_str("| Datum | Ticket | Node | Rust | zeitnah |\n|---|---|---|---|---|\n");
        for line in &self.readiness {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                date_of(&line.at),
                line.ticket_id,
                cell(line.node.as_deref()),
                cell(line.rust.as_deref()),
                if line.timing { "ja" } else { "nein" },
            ));
        }
    }
}

/// Die Abweichungen als Tabelle
fn table(out: &mut String, diffs: &[crate::dispatch::shadow::types::ShadowDiff]) {
    out.push_str("| Feld | Node | Rust |\n|---|---|---|\n");
    for diff in diffs {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            diff.field,
            cell(diff.node.as_deref()),
            cell(diff.rust.as_deref())
        ));
    }
}

/// Ein Wert in einer Tabellenzelle: gekürzt, ohne Zeilenumbrüche und ohne senkrechten Strich
fn cell(value: Option<&str>) -> String {
    let Some(value) = value else {
        return "–".to_string();
    };
    let flat = value.replace(['\r', '\n'], " ").replace('|', "\\|");
    if flat.chars().count() <= MAX_VALUE {
        return flat;
    }
    format!("{}…", flat.chars().take(MAX_VALUE).collect::<String>())
}

/// Der Tag eines Zeitpunkts, wie die Überschrift ihn nennt
fn date_of(at: &str) -> &str {
    at.split('T').next().unwrap_or(at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::shadow::types::ShadowDiff;

    fn decision(equal: bool) -> ShadowDecision {
        ShadowDecision {
            at: "2026-11-01T10:00:00.000Z".into(),
            run_id: "r1".into(),
            ticket_id: "ARC-0001".into(),
            kind: "work".into(),
            equal,
            diffs: if equal {
                Vec::new()
            } else {
                vec![ShadowDiff::new(
                    "run.profile",
                    Some("leicht".into()),
                    Some("schwer".into()),
                )]
            },
            prompt_diff: None,
            skipped: None,
        }
    }

    #[test]
    fn die_zusammenfassung_zaehlt_gleich_und_abweichend() {
        let report = ShadowReport::build(
            Some("2026-11-01".into()),
            None,
            vec![decision(true), decision(true), decision(false)],
            vec![ShadowEvaluation {
                at: "2026-11-01T11:00:00.000Z".into(),
                run_id: "r1".into(),
                row_id: None,
                ticket_id: "ARC-0001".into(),
                kind: "work".into(),
                equal: false,
                missing_row: true,
                diffs: Vec::new(),
            }],
            vec![ShadowReadiness {
                at: "2026-11-01T11:00:00.000Z".into(),
                ticket_id: "ARC-0002".into(),
                node: Some("bereit".into()),
                rust: Some("wartet – Worktree belegt durch ARC-0003".into()),
                timing: true,
            }],
        );
        assert_eq!(report.summary.runs, 3);
        assert_eq!(report.summary.decisions_equal, 2);
        assert_eq!(report.summary.decisions_different, 1);
        assert_eq!(report.summary.evaluations_missing, 1);
        assert_eq!(report.summary.readiness_timing, 1);
        // Gleiche Entscheidungen stehen nur in den Zahlen, nicht in der Liste
        assert_eq!(report.decisions.len(), 1);
    }

    #[test]
    fn das_markdown_nennt_zeitraum_zahlen_und_jede_abweichung() {
        let report = ShadowReport::build(
            Some("2026-11-01".into()),
            Some("2026-11-07".into()),
            vec![decision(false)],
            Vec::new(),
            Vec::new(),
        );
        let text = report.markdown();
        assert!(
            text.contains("Zeitraum: 2026-11-01 bis 2026-11-07"),
            "{text}"
        );
        assert!(text.contains("- Entscheidungen: 0 gleich, 1 abweichend, 0 ohne Urteil"));
        assert!(text.contains("### 2026-11-01 ARC-0001 (work)"));
        assert!(text.contains("| run.profile | leicht | schwer |"));
    }

    #[test]
    fn ein_prompt_diff_steht_vollstaendig_in_einem_details_block() {
        let mut line = decision(false);
        line.prompt_diff = Some("@@ -1,1 +1,1 @@\n-alt\n+neu\n".into());
        let text = ShadowReport::build(None, None, vec![line], Vec::new(), Vec::new()).markdown();
        assert!(text.contains("<details><summary>Arbeitsanweisung</summary>"));
        assert!(text.contains("-alt\n+neu\n"));
        assert!(text.contains("</details>"));
    }

    #[test]
    fn lange_werte_werden_gekuerzt_und_bleiben_in_einer_zeile() {
        let long = "x".repeat(MAX_VALUE + 20);
        let cut = cell(Some(&long));
        assert_eq!(cut.chars().count(), MAX_VALUE + 1);
        assert!(cut.ends_with('…'));
        assert_eq!(cell(Some("a|b\nc")), "a\\|b c");
        assert_eq!(cell(None), "–");
    }

    #[test]
    fn ohne_abweichungen_bleibt_der_bericht_bei_den_zahlen() {
        let text = ShadowReport::build(None, None, vec![decision(true)], Vec::new(), Vec::new())
            .markdown();
        assert!(text.contains("Zeitraum: alle Tage"));
        assert!(!text.contains("## Abweichende"));
    }
}
