//! Baut Abhängigkeitsgraphen aus Tickets, bricht Kreise auf und bestimmt den kritischen Pfad
//! (reine Funktionen). Portiert aus `packages/core/src/board/DependencyGraph.ts`.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::board::basics::TicketStatus;
use crate::board::dependency_rules::DependencyRules;
use crate::board::ticket_ordering::{BoardColumn, TicketOrdering};
use crate::board::types::Ticket;

/// Umfang eines Abhängigkeitsgraphen: ein Ticket, eine Spec im Projekt oder ein Board
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphScope {
    Ticket { id: String },
    Spec { project_id: i32, spec: String },
    Board { board_id: i32 },
}

/// Einstellungen des Graphen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphOptions {
    /// Erledigte und nicht mehr nötige Tickets zeigen (Standard: an; das Ticket der
    /// Ticket-Ansicht bleibt immer)
    pub include_done: bool,
}

impl Default for GraphOptions {
    fn default() -> Self {
        Self { include_done: true }
    }
}

/// Ein Knoten des Graphen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct GraphNode {
    pub id: String,
    pub title: String,
    pub status: TicketStatus,
    pub column: BoardColumn,
    pub board_id: i32,
    pub spec: Option<String>,
    /// Direkter Nachbar außerhalb des Umfangs (andere Spec oder anderes Board)
    pub external: bool,
}

/// Kante „to setzt from voraus“
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
}

/// Das Ergebnis eines Graphen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct DependencyGraphResult {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// Offene Tickets der längsten Kette, von der ersten Voraussetzung bis zum Ziel
    pub critical_path: Vec<String>,
    /// Kanten, die einen Kreis schließen; sie fehlen in `edges`
    pub cycle_edges: Vec<GraphEdge>,
    /// Tickets des Boards ohne jede Kante (nur Board-Ansicht)
    pub hidden_without_edges: usize,
    /// Ausgeblendete erledigte oder nicht mehr nötige Tickets
    pub hidden_done: usize,
}

/// Status, die als abgeschlossen gelten (`CLOSED`)
const CLOSED: [TicketStatus; 2] = [TicketStatus::Done, TicketStatus::NotNeeded];

/// Status, in denen ein Ticket durch ein Erledigt bereit wird
const UNLOCKABLE: [TicketStatus; 3] = [
    TicketStatus::Backlog,
    TicketStatus::InPlanning,
    TicketStatus::Planned,
];

/// Baut Graphen aus Tickets.
pub struct DependencyGraph;

impl DependencyGraph {
    /// Graph für den Umfang; `tickets` sind alle bekannten Tickets (über Boards und Projekte)
    pub fn build(
        tickets: &[Ticket],
        scope: &GraphScope,
        options: GraphOptions,
    ) -> DependencyGraphResult {
        let by_id: HashMap<String, Ticket> = tickets
            .iter()
            .map(|ticket| (ticket.id.clone(), ticket.clone()))
            .collect();
        let mut up: HashMap<String, Vec<String>> = HashMap::new();
        let mut down: HashMap<String, Vec<String>> = HashMap::new();
        for ticket in tickets {
            for dep in &ticket.depends_on {
                if !by_id.contains_key(dep) || dep == &ticket.id {
                    continue;
                }
                push(&mut up, &ticket.id, dep.clone());
                push(&mut down, dep, ticket.id.clone());
            }
        }

        let (members, hidden_without_edges) = Self::members(tickets, scope, &by_id, &up, &down);
        let mut external: HashSet<String> = HashSet::new();
        if !matches!(scope, GraphScope::Ticket { .. }) {
            for id in &members {
                let neighbours = up
                    .get(id)
                    .into_iter()
                    .flatten()
                    .chain(down.get(id).into_iter().flatten());
                for neighbour in neighbours {
                    if !members.contains(neighbour) {
                        external.insert(neighbour.clone());
                    }
                }
            }
        }

        let keep = |id: &String| {
            options.include_done
                || !CLOSED.contains(&by_id[id].status)
                || matches!(scope, GraphScope::Ticket { id: target } if target == id)
        };
        let all: Vec<String> = members.iter().chain(external.iter()).cloned().collect();
        let mut visible: Vec<String> = all.iter().filter(|id| keep(id)).cloned().collect();
        visible.sort();
        let shown: HashSet<&String> = visible.iter().collect();

        let mut raw: Vec<GraphEdge> = Vec::new();
        for to in &visible {
            for from in up.get(to).into_iter().flatten() {
                if shown.contains(from) {
                    raw.push(GraphEdge {
                        from: from.clone(),
                        to: to.clone(),
                    });
                }
            }
        }

        let mut isolated = 0;
        if matches!(scope, GraphScope::Board { .. }) {
            // Board-Tickets, deren Kanten nur zu ausgeblendeten Erledigten führten, zählen wie
            // kantenlose
            let linked: HashSet<&str> = raw
                .iter()
                .flat_map(|edge| [edge.from.as_str(), edge.to.as_str()])
                .collect();
            isolated = visible.iter().filter(|id| !linked.contains(id.as_str())).count();
            visible.retain(|id| linked.contains(id.as_str()));
        }

        let (edges, cycle_edges) = Self::break_cycles(&visible, raw);
        let target = match scope {
            GraphScope::Ticket { id } => Some(id.as_str()),
            _ => None,
        };
        let visible_tickets: Vec<Ticket> = visible.iter().map(|id| by_id[id].clone()).collect();
        let critical_path = Self::critical_path(&visible_tickets, &edges, target);

        DependencyGraphResult {
            nodes: visible
                .iter()
                .map(|id| Self::node(&by_id[id], &by_id, external.contains(id)))
                .collect(),
            edges: sort_edges(edges),
            critical_path,
            cycle_edges: sort_edges(cycle_edges),
            hidden_without_edges: hidden_without_edges + isolated,
            hidden_done: all.len() - visible.len() - isolated,
        }
    }

    /// Entfernt per Tiefensuche (iterativ, nach ID sortiert) jede Kante, die zu einem Knoten auf
    /// dem aktuellen Weg zurückführt
    pub fn break_cycles(
        nodes: &[String],
        edges: Vec<GraphEdge>,
    ) -> (Vec<GraphEdge>, Vec<GraphEdge>) {
        let mut out: HashMap<String, Vec<GraphEdge>> = HashMap::new();
        for edge in sort_edges(edges.clone()) {
            out.entry(edge.from.clone()).or_default().push(edge);
        }
        /// Zustand eines Knotens in der Tiefensuche
        #[derive(PartialEq, Eq, Clone, Copy)]
        enum State {
            Active,
            Done,
        }
        let mut state: HashMap<&str, State> = HashMap::new();
        let mut cycle_edges: Vec<GraphEdge> = Vec::new();
        let mut starts: Vec<&String> = nodes.iter().collect();
        starts.sort();
        for start in starts {
            if state.contains_key(start.as_str()) {
                continue;
            }
            let mut stack: Vec<(&str, usize)> = vec![(start.as_str(), 0)];
            state.insert(start.as_str(), State::Active);
            while !stack.is_empty() {
                let top = stack.len() - 1;
                let (id, next) = stack[top];
                stack[top].1 += 1;
                let Some(edge) = out.get(id).and_then(|list| list.get(next)) else {
                    state.insert(id, State::Done);
                    stack.pop();
                    continue;
                };
                match state.get(edge.to.as_str()) {
                    Some(State::Active) => cycle_edges.push(edge.clone()),
                    Some(State::Done) => {}
                    None => {
                        state.insert(edge.to.as_str(), State::Active);
                        stack.push((edge.to.as_str(), 0));
                    }
                }
            }
        }
        let cut: HashSet<&GraphEdge> = cycle_edges.iter().collect();
        let kept: Vec<GraphEdge> = edges
            .iter()
            .filter(|edge| !cut.contains(edge))
            .cloned()
            .collect();
        (kept, cycle_edges)
    }

    /// Längste Kette offener Tickets (Gewicht 1 je offenem Ticket) im kreisfreien Graphen;
    /// mit `target` die längste Kette in dieses Ticket, sonst zum offenen Ticket mit der
    /// längsten Kette (Gleichstand: kleinste ID)
    pub fn critical_path(
        tickets: &[Ticket],
        edges: &[GraphEdge],
        target: Option<&str>,
    ) -> Vec<String> {
        let by_id: HashMap<&str, &Ticket> = tickets
            .iter()
            .map(|ticket| (ticket.id.as_str(), ticket))
            .collect();
        let open = |id: &str| !CLOSED.contains(&by_id[id].status);
        let mut preds: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut succs: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut indegree: HashMap<&str, usize> =
            tickets.iter().map(|ticket| (ticket.id.as_str(), 0)).collect();
        for edge in edges {
            let (from, to) = (edge.from.as_str(), edge.to.as_str());
            if !by_id.contains_key(from) || !by_id.contains_key(to) {
                continue;
            }
            preds.entry(to).or_default().push(from);
            succs.entry(from).or_default().push(to);
            *indegree.get_mut(to).expect("Ticket steht im Eingangsgrad") += 1;
        }

        let mut queue: Vec<&str> = tickets
            .iter()
            .map(|ticket| ticket.id.as_str())
            .filter(|id| indegree[id] == 0)
            .collect();
        queue.sort_unstable();
        let mut best: HashMap<&str, usize> = HashMap::new();
        let mut prev: HashMap<&str, Option<&str>> = HashMap::new();
        let mut index = 0;
        while index < queue.len() {
            let id = queue[index];
            index += 1;
            let mut sorted_preds: Vec<&str> = preds.get(id).cloned().unwrap_or_default();
            sorted_preds.sort_unstable();
            let mut pick: Option<&str> = None;
            for p in sorted_preds {
                if pick.is_none_or(|current| best[p] > best[current]) {
                    pick = Some(p);
                }
            }
            let base = pick.map_or(0, |p| best[p]);
            best.insert(id, base + usize::from(open(id)));
            prev.insert(id, pick);
            for successor in succs.get(id).cloned().unwrap_or_default() {
                let degree = indegree
                    .get_mut(successor)
                    .expect("Ticket steht im Eingangsgrad");
                *degree -= 1;
                if *degree == 0 {
                    queue.push(successor);
                }
            }
        }

        let mut end: Option<&str> = match target {
            Some(target) if best.contains_key(target) => Some(target),
            Some(_) => None,
            None => None,
        };
        if target.is_none() {
            let mut sorted: Vec<&Ticket> = tickets.iter().collect();
            sorted.sort_by(|a, b| a.id.cmp(&b.id));
            for ticket in sorted {
                let id = ticket.id.as_str();
                if open(id) && end.is_none_or(|current| best[id] > best[current]) {
                    end = Some(id);
                }
            }
        }

        let mut path: Vec<String> = Vec::new();
        let mut current = end;
        while let Some(id) = current {
            if open(id) {
                path.insert(0, id.to_string());
            }
            current = prev.get(id).copied().flatten();
        }
        path
    }

    /// Abhängige von `id`, deren einzige offene Voraussetzung `id` ist und die in Backlog,
    /// In Planung oder Geplant stehen
    pub fn unlocked_by(
        id: &str,
        tickets: &[Ticket],
        by_id: &HashMap<String, Ticket>,
    ) -> Vec<String> {
        let mut result: Vec<String> = tickets
            .iter()
            .filter(|ticket| {
                ticket.depends_on.iter().any(|dep| dep == id)
                    && UNLOCKABLE.contains(&ticket.status)
            })
            .filter(|ticket| {
                let open = DependencyRules::open_dependencies(ticket, by_id);
                open.len() == 1 && open[0] == id
            })
            .map(|ticket| ticket.id.clone())
            .collect();
        result.sort();
        result
    }

    /// Knoten des Umfangs (ohne externe Nachbarn) und Zahl der kantenlos ausgeblendeten
    /// Board-Tickets
    fn members(
        tickets: &[Ticket],
        scope: &GraphScope,
        by_id: &HashMap<String, Ticket>,
        up: &HashMap<String, Vec<String>>,
        down: &HashMap<String, Vec<String>>,
    ) -> (HashSet<String>, usize) {
        match scope {
            GraphScope::Ticket { id } => {
                if !by_id.contains_key(id) {
                    return (HashSet::new(), 0);
                }
                let mut members = reach(id, up);
                members.extend(reach(id, down));
                (members, 0)
            }
            GraphScope::Spec { project_id, spec } => (
                tickets
                    .iter()
                    .filter(|ticket| {
                        ticket.project_id == *project_id && ticket.spec.as_deref() == Some(spec)
                    })
                    .map(|ticket| ticket.id.clone())
                    .collect(),
                0,
            ),
            GraphScope::Board { board_id } => {
                let on_board: Vec<&Ticket> = tickets
                    .iter()
                    .filter(|ticket| ticket.board_id == *board_id)
                    .collect();
                let linked: HashSet<String> = on_board
                    .iter()
                    .filter(|ticket| up.contains_key(&ticket.id) || down.contains_key(&ticket.id))
                    .map(|ticket| ticket.id.clone())
                    .collect();
                let hidden = on_board.len() - linked.len();
                (linked, hidden)
            }
        }
    }

    /// Anzeigeknoten mit Spalte (wartet wie auf dem Board)
    fn node(ticket: &Ticket, by_id: &HashMap<String, Ticket>, external: bool) -> GraphNode {
        GraphNode {
            id: ticket.id.clone(),
            title: ticket.title.clone(),
            status: ticket.status,
            column: TicketOrdering::column(ticket, DependencyRules::is_waiting(ticket, by_id)),
            board_id: ticket.board_id,
            spec: ticket.spec.clone(),
            external,
        }
    }
}

/// Hängt einen Wert an die Liste des Schlüssels an
fn push<T>(map: &mut HashMap<String, Vec<T>>, key: &str, value: T) {
    map.entry(key.to_string()).or_default().push(value);
}

/// Alle von `start` aus erreichbaren IDs (`start` eingeschlossen), iterativ
fn reach(start: &str, next: &HashMap<String, Vec<String>>) -> HashSet<String> {
    let mut seen: HashSet<String> = HashSet::from([start.to_string()]);
    let mut stack: Vec<String> = vec![start.to_string()];
    while let Some(id) = stack.pop() {
        for neighbour in next.get(&id).into_iter().flatten() {
            if seen.insert(neighbour.clone()) {
                stack.push(neighbour.clone());
            }
        }
    }
    seen
}

/// Kanten nach Ziel, dann Quelle sortiert (deterministische Ausgabe)
fn sort_edges(mut edges: Vec<GraphEdge>) -> Vec<GraphEdge> {
    edges.sort_by(|a, b| a.to.cmp(&b.to).then_with(|| a.from.cmp(&b.from)));
    edges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ticket_fixture;

    /// Minimales Ticket für reine Graph-Tests
    fn ticket(id: &str, deps: &[&str], status: TicketStatus) -> Ticket {
        let number: i32 = id[id.len() - 4..].parse().unwrap();
        Ticket {
            title: format!("Titel {id}"),
            status,
            needs_planning: false,
            depends_on: deps.iter().map(|dep| dep.to_string()).collect(),
            created_at: String::new(),
            created_by_id: String::new(),
            created_by_name: String::new(),
            updated_at: String::new(),
            ..ticket_fixture(id, 1, 1, number)
        }
    }

    /// Ticket im Status „Geplant“
    fn planned(id: &str, deps: &[&str]) -> Ticket {
        ticket(id, deps, TicketStatus::Planned)
    }

    /// Die IDs der Knoten, sortiert
    fn ids(result: &DependencyGraphResult) -> Vec<String> {
        let mut ids: Vec<String> = result.nodes.iter().map(|node| node.id.clone()).collect();
        ids.sort();
        ids
    }

    /// Eine Kante
    fn edge(from: &str, to: &str) -> GraphEdge {
        GraphEdge {
            from: from.into(),
            to: to.into(),
        }
    }

    #[test]
    fn liefert_fur_ein_ticket_alle_voraussetzungen_und_abhangigen_transitiv_auch_uber_boards() {
        let tickets = [
            planned("ARC-0001", &[]),
            Ticket {
                board_id: 2,
                ..planned("ARC-0002", &["ARC-0001"])
            },
            planned("ARC-0003", &["ARC-0002"]),
            Ticket {
                board_id: 2,
                ..ticket("ARC-0004", &["ARC-0003"], TicketStatus::Backlog)
            },
            planned("ARC-0005", &[]),
        ];

        let g = DependencyGraph::build(
            &tickets,
            &GraphScope::Ticket {
                id: "ARC-0003".into(),
            },
            GraphOptions::default(),
        );

        assert_eq!(
            ids(&g),
            ["ARC-0001", "ARC-0002", "ARC-0003", "ARC-0004"]
        );
        assert_eq!(
            g.edges,
            [
                edge("ARC-0001", "ARC-0002"),
                edge("ARC-0002", "ARC-0003"),
                edge("ARC-0003", "ARC-0004")
            ]
        );
        assert!(g.nodes.iter().all(|node| !node.external));
        assert_eq!(
            g.nodes
                .iter()
                .find(|node| node.id == "ARC-0004")
                .map(|node| node.column),
            Some(BoardColumn::Waiting)
        );
    }

    #[test]
    fn nimmt_fur_eine_spec_deren_tickets_im_projekt_und_markiert_fremde_nachbarn_als_extern() {
        let tickets = [
            planned("ARC-0001", &[]),
            Ticket {
                spec: Some("001-login".into()),
                ..planned("ARC-0002", &["ARC-0001"])
            },
            Ticket {
                spec: Some("001-login".into()),
                board_id: 2,
                ..planned("ARC-0003", &["ARC-0002"])
            },
            planned("ARC-0004", &["ARC-0003"]),
            planned("ARC-0005", &["ARC-0004"]),
            Ticket {
                spec: Some("001-login".into()),
                project_id: 2,
                ..planned("ARC-0006", &[])
            },
        ];

        let g = DependencyGraph::build(
            &tickets,
            &GraphScope::Spec {
                project_id: 1,
                spec: "001-login".into(),
            },
            GraphOptions::default(),
        );

        assert_eq!(
            ids(&g),
            ["ARC-0001", "ARC-0002", "ARC-0003", "ARC-0004"]
        );
        let mut external: Vec<&str> = g
            .nodes
            .iter()
            .filter(|node| node.external)
            .map(|node| node.id.as_str())
            .collect();
        external.sort_unstable();
        assert_eq!(external, ["ARC-0001", "ARC-0004"]);
    }

    #[test]
    fn blendet_in_der_board_ansicht_tickets_ohne_kante_aus_und_zahlt_sie() {
        let tickets = [
            planned("ARC-0001", &[]),
            planned("ARC-0002", &["ARC-0001"]),
            planned("ARC-0003", &[]),
            planned("ARC-0004", &[]),
            Ticket {
                board_id: 2,
                ..planned("ARC-0005", &["ARC-0002"])
            },
        ];

        let g = DependencyGraph::build(
            &tickets,
            &GraphScope::Board { board_id: 1 },
            GraphOptions::default(),
        );

        assert_eq!(ids(&g), ["ARC-0001", "ARC-0002", "ARC-0005"]);
        assert_eq!(
            g.nodes
                .iter()
                .find(|node| node.id == "ARC-0005")
                .map(|node| node.external),
            Some(true)
        );
        assert_eq!(g.hidden_without_edges, 2);
    }

    #[test]
    fn blendet_erledigte_auf_wunsch_aus_das_ticket_selbst_bleibt() {
        let tickets = [
            ticket("ARC-0001", &[], TicketStatus::Done),
            ticket("ARC-0002", &["ARC-0001"], TicketStatus::Done),
            planned("ARC-0003", &["ARC-0002"]),
        ];

        let g = DependencyGraph::build(
            &tickets,
            &GraphScope::Ticket {
                id: "ARC-0002".into(),
            },
            GraphOptions {
                include_done: false,
            },
        );

        assert_eq!(ids(&g), ["ARC-0002", "ARC-0003"]);
        assert_eq!(g.edges, [edge("ARC-0002", "ARC-0003")]);
        assert_eq!(g.hidden_done, 1);
    }

    #[test]
    fn blendet_auf_dem_board_tickets_aus_deren_kanten_nur_zu_erledigten_fuhren() {
        let tickets = [
            ticket("ARC-0001", &[], TicketStatus::Done),
            planned("ARC-0002", &["ARC-0001"]),
            planned("ARC-0003", &["ARC-0001"]),
            planned("ARC-0004", &[]),
            planned("ARC-0005", &["ARC-0004"]),
        ];

        let g = DependencyGraph::build(
            &tickets,
            &GraphScope::Board { board_id: 1 },
            GraphOptions {
                include_done: false,
            },
        );

        assert_eq!(ids(&g), ["ARC-0004", "ARC-0005"]);
        assert_eq!(g.hidden_without_edges, 2);
        assert_eq!(g.hidden_done, 1);
    }

    #[test]
    fn erzeugt_fur_unbekannte_geloschte_voraussetzungen_keine_kante() {
        let tickets = [planned("ARC-0001", &["ARC-0099"])];

        let g = DependencyGraph::build(
            &tickets,
            &GraphScope::Ticket {
                id: "ARC-0001".into(),
            },
            GraphOptions::default(),
        );

        assert_eq!(ids(&g), ["ARC-0001"]);
        assert!(g.edges.is_empty());
    }

    #[test]
    fn terminiert_bei_einem_kreis_und_meldet_die_schliessende_kante() {
        let tickets = [
            planned("ARC-0001", &["ARC-0002"]),
            planned("ARC-0002", &["ARC-0001"]),
        ];

        let g = DependencyGraph::build(
            &tickets,
            &GraphScope::Ticket {
                id: "ARC-0001".into(),
            },
            GraphOptions::default(),
        );

        assert_eq!(ids(&g), ["ARC-0001", "ARC-0002"]);
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.cycle_edges.len(), 1);
        let mut both: Vec<&GraphEdge> = g.edges.iter().chain(g.cycle_edges.iter()).collect();
        both.sort_by(|a, b| a.from.cmp(&b.from));
        assert_eq!(
            both,
            [&edge("ARC-0001", "ARC-0002"), &edge("ARC-0002", "ARC-0001")]
        );
    }

    #[test]
    fn kritischer_pfad_liefert_die_offene_kette_in_reihenfolge_der_voraussetzungen() {
        let tickets = [
            planned("ARC-0001", &[]),
            planned("ARC-0002", &["ARC-0001"]),
            planned("ARC-0003", &["ARC-0002"]),
        ];

        let g = DependencyGraph::build(
            &tickets,
            &GraphScope::Board { board_id: 1 },
            GraphOptions::default(),
        );

        assert_eq!(g.critical_path, ["ARC-0001", "ARC-0002", "ARC-0003"]);
    }

    #[test]
    fn kritischer_pfad_zahlt_ein_erledigtes_mittelglied_nicht() {
        let tickets = [
            planned("ARC-0001", &[]),
            ticket("ARC-0002", &["ARC-0001"], TicketStatus::Done),
            planned("ARC-0003", &["ARC-0002"]),
        ];

        let g = DependencyGraph::build(
            &tickets,
            &GraphScope::Board { board_id: 1 },
            GraphOptions::default(),
        );

        assert_eq!(g.critical_path, ["ARC-0001", "ARC-0003"]);
    }

    #[test]
    fn kritischer_pfad_wahlt_bei_gleich_langen_ketten_die_kleinste_id() {
        let tickets = [
            planned("ARC-0001", &[]),
            planned("ARC-0004", &["ARC-0001"]),
            planned("ARC-0002", &[]),
            planned("ARC-0003", &["ARC-0002"]),
        ];

        let g = DependencyGraph::build(
            &tickets,
            &GraphScope::Board { board_id: 1 },
            GraphOptions::default(),
        );

        assert_eq!(g.critical_path, ["ARC-0002", "ARC-0003"]);
    }

    #[test]
    fn kritischer_pfad_nimmt_in_der_ticket_ansicht_die_langste_kette_in_das_ticket() {
        let tickets = [
            planned("ARC-0001", &[]),
            planned("ARC-0002", &["ARC-0001"]),
            planned("ARC-0003", &["ARC-0002"]),
            planned("ARC-0004", &["ARC-0003"]),
            planned("ARC-0005", &["ARC-0004"]),
        ];

        let g = DependencyGraph::build(
            &tickets,
            &GraphScope::Ticket {
                id: "ARC-0003".into(),
            },
            GraphOptions::default(),
        );

        assert_eq!(g.critical_path, ["ARC-0001", "ARC-0002", "ARC-0003"]);
    }

    #[test]
    fn kritischer_pfad_ist_leer_wenn_alles_erledigt_ist() {
        let tickets = [
            ticket("ARC-0001", &[], TicketStatus::Done),
            ticket("ARC-0002", &["ARC-0001"], TicketStatus::NotNeeded),
        ];

        let g = DependencyGraph::build(
            &tickets,
            &GraphScope::Board { board_id: 1 },
            GraphOptions::default(),
        );

        assert!(g.critical_path.is_empty());
    }

    #[test]
    fn rechnet_200_tickets_in_einer_kette_schnell() {
        let ids: Vec<String> = (1..=200).map(|n| format!("ARC-{n:04}")).collect();
        let tickets: Vec<Ticket> = ids
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let deps: Vec<&str> = if index == 0 {
                    Vec::new()
                } else {
                    vec![ids[index - 1].as_str()]
                };
                planned(id, &deps)
            })
            .collect();

        let start = std::time::Instant::now();
        let g = DependencyGraph::build(
            &tickets,
            &GraphScope::Board { board_id: 1 },
            GraphOptions::default(),
        );

        assert!(start.elapsed() < std::time::Duration::from_millis(500));
        assert_eq!(g.critical_path.len(), 200);
    }

    #[test]
    fn nennt_abhangige_deren_einzige_offene_voraussetzung_das_ticket_ist() {
        let tickets = [
            planned("ARC-0001", &[]),
            planned("ARC-0002", &[]),
            planned("ARC-0003", &["ARC-0001"]),
            planned("ARC-0004", &["ARC-0001", "ARC-0002"]),
            ticket("ARC-0005", &["ARC-0001"], TicketStatus::InProgress),
            ticket("ARC-0006", &["ARC-0001", "ARC-0007"], TicketStatus::Backlog),
            ticket("ARC-0007", &[], TicketStatus::Done),
        ];
        let by_id: HashMap<String, Ticket> = tickets
            .iter()
            .map(|ticket| (ticket.id.clone(), ticket.clone()))
            .collect();

        assert_eq!(
            DependencyGraph::unlocked_by("ARC-0001", &tickets, &by_id),
            ["ARC-0003", "ARC-0006"]
        );
    }
}
