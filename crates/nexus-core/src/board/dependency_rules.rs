//! Regeln für Voraussetzungen: Liste bereinigen, Selbstbezug/Unbekannt/Kreis ablehnen,
//! offene ermitteln. Portiert aus `packages/core/src/board/DependencyRules.ts`.

use std::collections::{HashMap, HashSet};

use crate::board::basics::TicketStatus;
use crate::board::ticket_ids::TicketIds;
use crate::board::types::Ticket;
use crate::error::{HarnessError, Result};

/// Status, in denen ein Ticket als „wartet“ gilt, wenn eine Voraussetzung offen ist
const WAITING_STATUSES: [TicketStatus; 3] = [
    TicketStatus::Backlog,
    TicketStatus::InPlanning,
    TicketStatus::Planned,
];

/// Jede ID mit ihren Voraussetzungen (`Map<string, string[]>` in TypeScript)
pub type DependencyGraphMap = HashMap<String, Vec<String>>;

/// Prüft und bereinigt Voraussetzungen.
pub struct DependencyRules;

impl DependencyRules {
    /// Trimmt, vereinheitlicht die Schreibweise und entfernt Doppelte (Reihenfolge bleibt)
    pub fn normalize(list: &[String]) -> Result<Vec<String>> {
        let mut result: Vec<String> = Vec::new();
        for raw in list {
            if raw.trim().is_empty() {
                continue;
            }
            let id = TicketIds::canonical(raw)?;
            if !result.contains(&id) {
                result.push(id);
            }
        }
        Ok(result)
    }

    /// Prüft die neue Liste gegen alle bekannten Tickets; `graph` enthält jede ID mit ihren
    /// Voraussetzungen
    pub fn validate(ticket_id: &str, deps: &[String], graph: &DependencyGraphMap) -> Result<()> {
        if deps.iter().any(|id| id == ticket_id) {
            return Err(HarnessError::new(
                "dependency-self",
                format!("{ticket_id} kann sich nicht selbst voraussetzen."),
            )
            .into());
        }
        for dep in deps {
            if !graph.contains_key(dep) {
                return Err(HarnessError::new(
                    "dependency-unknown",
                    format!("Voraussetzung {dep} gibt es nicht."),
                )
                .into());
            }
        }
        let mut next = graph.clone();
        next.insert(ticket_id.to_string(), deps.to_vec());
        if let Some(path) = Self::cycle_from(ticket_id, &next) {
            return Err(HarnessError::new(
                "dependency-cycle",
                format!("Abhängigkeiten liefen im Kreis: {}.", path.join(" -> ")),
            )
            .into());
        }
        Ok(())
    }

    /// Voraussetzungen, die weder erledigt noch „nicht mehr nötig“ sind; gelöschte gelten als erfüllt
    pub fn open_dependencies(ticket: &Ticket, by_id: &HashMap<String, Ticket>) -> Vec<String> {
        ticket
            .depends_on
            .iter()
            .filter(|id| match by_id.get(id.as_str()) {
                Some(dep) => !matches!(dep.status, TicketStatus::Done | TicketStatus::NotNeeded),
                None => false,
            })
            .cloned()
            .collect()
    }

    /// Erledigte Voraussetzungen, die noch einen Branch tragen (noch nicht gemergt); „nicht mehr
    /// nötig“ zählt als erfüllt, weil der Dispatcher solche Tickets nie mergt
    pub fn unmerged_dependencies(ticket: &Ticket, by_id: &HashMap<String, Ticket>) -> Vec<String> {
        ticket
            .depends_on
            .iter()
            .filter(|id| match by_id.get(id.as_str()) {
                Some(dep) => dep.status == TicketStatus::Done && dep.branch.is_some(),
                None => false,
            })
            .cloned()
            .collect()
    }

    /// Wartet: in Backlog, In Planung oder Geplant mit mindestens einer offenen Voraussetzung
    pub fn is_waiting(ticket: &Ticket, by_id: &HashMap<String, Ticket>) -> bool {
        WAITING_STATUSES.contains(&ticket.status)
            && !Self::open_dependencies(ticket, by_id).is_empty()
    }

    /// Tiefensuche: Pfad zurück zum Start oder `None`
    fn cycle_from(start: &str, graph: &DependencyGraphMap) -> Option<Vec<String>> {
        let mut seen: HashSet<String> = HashSet::from([start.to_string()]);
        Self::visit(start, &[start.to_string()], start, graph, &mut seen)
    }

    /// Ein Schritt der Tiefensuche; `path` ist der Weg vom Start bis `id`
    fn visit(
        id: &str,
        path: &[String],
        start: &str,
        graph: &DependencyGraphMap,
        seen: &mut HashSet<String>,
    ) -> Option<Vec<String>> {
        for dep in graph.get(id).map(Vec::as_slice).unwrap_or_default() {
            let mut next = path.to_vec();
            next.push(dep.clone());
            if dep == start {
                return Some(next);
            }
            if !seen.insert(dep.clone()) {
                continue;
            }
            if let Some(found) = Self::visit(dep, &next, start, graph, seen) {
                return Some(found);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ticket_fixture;

    /// Ticket mit Status und Voraussetzungen, sonst wie die Vorlage
    fn t(id: &str, status: TicketStatus, depends_on: &[&str]) -> Ticket {
        let number: i32 = id[4..].parse().unwrap();
        Ticket {
            status,
            depends_on: depends_on.iter().map(|x| x.to_string()).collect(),
            ..ticket_fixture(id, 1, 1, number)
        }
    }

    /// Alle Tickets nach ID
    fn by_id(tickets: &[Ticket]) -> HashMap<String, Ticket> {
        tickets
            .iter()
            .map(|t| (t.id.clone(), t.clone()))
            .collect::<HashMap<_, _>>()
    }

    /// Ein Graph aus Paaren
    fn graph(entries: &[(&str, &[&str])]) -> DependencyGraphMap {
        entries
            .iter()
            .map(|(id, deps)| {
                (
                    id.to_string(),
                    deps.iter().map(|x| x.to_string()).collect::<Vec<_>>(),
                )
            })
            .collect()
    }

    /// Eine Liste als `Vec<String>`
    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn normalisiert_trimmt_vereinheitlicht_entfernt_doppelte_behalt_die_reihenfolge() {
        assert_eq!(
            DependencyRules::normalize(&list(&[" arc-2 ", "ARC-0001", "ARC-0002", ""])).unwrap(),
            ["ARC-0002", "ARC-0001"]
        );
    }

    #[test]
    fn lehnt_selbstbezug_ab() {
        let error = DependencyRules::validate(
            "ARC-0001",
            &list(&["ARC-0001"]),
            &graph(&[("ARC-0001", &[])]),
        )
        .unwrap_err();
        assert_eq!(error.code(), Some("dependency-self"));
    }

    #[test]
    fn lehnt_unbekannte_ids_ab() {
        let error = DependencyRules::validate(
            "ARC-0001",
            &list(&["ARC-0099"]),
            &graph(&[("ARC-0001", &[])]),
        )
        .unwrap_err();
        assert_eq!(error.code(), Some("dependency-unknown"));
    }

    #[test]
    fn lehnt_einen_kreis_mit_pfad_in_der_meldung_ab() {
        let error = DependencyRules::validate(
            "ARC-0002",
            &list(&["ARC-0001"]),
            &graph(&[("ARC-0001", &["ARC-0002"]), ("ARC-0002", &[])]),
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Abhängigkeiten liefen im Kreis: ARC-0002 -> ARC-0001 -> ARC-0002."
        );
    }

    #[test]
    fn nimmt_eine_gultige_liste_an() {
        assert!(DependencyRules::validate(
            "ARC-0002",
            &list(&["ARC-0001"]),
            &graph(&[("ARC-0001", &[]), ("ARC-0002", &[])]),
        )
        .is_ok());
    }

    #[test]
    fn erledigt_nicht_mehr_notig_und_geloscht_gelten_als_erfullt() {
        let tickets = [
            t("ARC-0001", TicketStatus::Done, &[]),
            t("ARC-0002", TicketStatus::NotNeeded, &[]),
            t("ARC-0003", TicketStatus::InProgress, &[]),
            t(
                "ARC-0004",
                TicketStatus::Backlog,
                &["ARC-0001", "ARC-0002", "ARC-0003", "ARC-0099"],
            ),
        ];
        let by_id = by_id(&tickets);

        assert_eq!(
            DependencyRules::open_dependencies(&tickets[3], &by_id),
            ["ARC-0003"]
        );
        assert!(DependencyRules::is_waiting(&tickets[3], &by_id));
    }

    #[test]
    fn wartet_nur_in_backlog_in_planung_und_geplant() {
        let tickets = [
            t("ARC-0001", TicketStatus::Backlog, &[]),
            t("ARC-0002", TicketStatus::InProgress, &["ARC-0001"]),
            t("ARC-0003", TicketStatus::Blocked, &["ARC-0001"]),
        ];
        let by_id = by_id(&tickets);

        assert!(!DependencyRules::is_waiting(&tickets[1], &by_id));
        assert!(!DependencyRules::is_waiting(&tickets[2], &by_id));
    }

    #[test]
    fn nennt_erledigte_voraussetzungen_mit_branch_als_nicht_gemergt() {
        let mut done = t("ARC-0001", TicketStatus::Done, &[]);
        done.branch = Some("ticket/ARC-0001".into());
        let tickets = [done, t("ARC-0002", TicketStatus::Backlog, &["ARC-0001"])];
        let by_id = by_id(&tickets);

        assert_eq!(
            DependencyRules::unmerged_dependencies(&tickets[1], &by_id),
            ["ARC-0001"]
        );
    }
}
