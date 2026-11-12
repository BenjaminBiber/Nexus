//! Liest Tickets aller Boards und liefert Abhängigkeitsgraphen und Fortschritt je Spec
//! (nur lesend). Portiert aus `packages/core/src/board/DependencyGraphService.ts`.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::board::board_service::BoardService;
use crate::board::database::{BoardDatabase, BoardFilter, TicketFilter};
use crate::board::dependency_graph::{
    DependencyGraph, GraphEdge, GraphNode, GraphOptions, GraphScope,
};
use crate::board::project_service::ProjectService;
use crate::board::spec_progress::{SpecProgress, SpecProgressCount};
use crate::board::ticket_ids::TicketIds;
use crate::clock::Clock;
use crate::error::{HarnessError, Result};
use crate::tx;

/// Gewünschter Umfang: ein Ticket, eine Spec eines Projekts (Key) oder ein Board
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "board.ts")]
pub enum GraphRequest {
    Ticket {
        id: String,
    },
    Spec {
        project: String,
        spec: String,
    },
    #[serde(rename_all = "camelCase")]
    Board {
        board_id: i32,
    },
}

/// Ein Knoten mit dem Namen seines Boards und dem Fortschritt seiner Schritte
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct GraphNodeView {
    #[serde(flatten)]
    #[ts(flatten)]
    pub node: GraphNode,
    pub board_name: String,
    /// Erledigte und alle Schritte (0/0 ohne Schritte), wie auf der Board-Karte
    pub steps_done: i32,
    pub steps_total: i32,
}

/// Ein Graph, wie die Oberfläche ihn bekommt
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct DependencyGraphView {
    pub scope: GraphRequest,
    pub nodes: Vec<GraphNodeView>,
    pub edges: Vec<GraphEdge>,
    pub critical_path: Vec<String>,
    pub cycle_edges: Vec<GraphEdge>,
    pub hidden_without_edges: usize,
    pub hidden_done: usize,
}

/// Graphen und Spec-Fortschritt über alle Boards.
#[derive(Debug, Clone)]
pub struct DependencyGraphService<D: BoardDatabase> {
    db: Arc<D>,
}

impl<D: BoardDatabase> DependencyGraphService<D> {
    /// Dienst auf dieser Datenbank; die Uhr wird nur der Einheitlichkeit halber genommen
    pub fn new(db: Arc<D>, _clock: Clock) -> Self {
        Self { db }
    }

    /// Graph des Umfangs; `include_done` ohne Angabe: Board aus, Ticket und Spec an
    pub async fn build(
        &self,
        request: &GraphRequest,
        include_done: Option<bool>,
    ) -> Result<DependencyGraphView> {
        let request = request.clone();
        tx!(self.db, [request], |u| {
            let tickets = u
                .tickets()
                .list(&TicketFilter {
                    without_texts: true,
                    ..TicketFilter::default()
                })
                .await?;
            let board_names: HashMap<i32, String> = u
                .boards()
                .list(&BoardFilter::default())
                .await?
                .into_iter()
                .map(|board| (board.id, board.name))
                .collect();
            let options = GraphOptions {
                include_done: include_done
                    .unwrap_or(!matches!(request, GraphRequest::Board { .. })),
            };
            let (scope, result) = match &request {
                GraphRequest::Ticket { id } => {
                    let id = TicketIds::canonical(id)?;
                    if !tickets.iter().any(|ticket| ticket.id == id) {
                        return Err(HarnessError::new(
                            "ticket-not-found",
                            format!("Ticket {id} gibt es nicht."),
                        )
                        .into());
                    }
                    let scope = GraphScope::Ticket { id: id.clone() };
                    let result = DependencyGraph::build(&tickets, &scope, options);
                    (GraphRequest::Ticket { id }, result)
                }
                GraphRequest::Spec { project, spec } => {
                    let project = ProjectService::<D>::require_in(u, project).await?;
                    let scope = GraphScope::Spec {
                        project_id: project.id,
                        spec: spec.clone(),
                    };
                    let result = DependencyGraph::build(&tickets, &scope, options);
                    (
                        GraphRequest::Spec {
                            project: project.key,
                            spec: spec.clone(),
                        },
                        result,
                    )
                }
                GraphRequest::Board { board_id } => {
                    BoardService::<D>::require_in(u, *board_id).await?;
                    let scope = GraphScope::Board {
                        board_id: *board_id,
                    };
                    let result = DependencyGraph::build(&tickets, &scope, options);
                    (
                        GraphRequest::Board {
                            board_id: *board_id,
                        },
                        result,
                    )
                }
            };
            let ids: Vec<String> = result.nodes.iter().map(|node| node.id.clone()).collect();
            let progress = u.steps().progress(&ids).await?;
            Ok(DependencyGraphView {
                scope,
                nodes: result
                    .nodes
                    .into_iter()
                    .map(|node| {
                        let steps = progress.get(&node.id);
                        GraphNodeView {
                            board_name: board_names
                                .get(&node.board_id)
                                .map_or("?", String::as_str)
                                .to_string(),
                            steps_done: steps.map_or(0, |steps| steps.done),
                            steps_total: steps.map_or(0, |steps| steps.total),
                            node,
                        }
                    })
                    .collect(),
                edges: result.edges,
                critical_path: result.critical_path,
                cycle_edges: result.cycle_edges,
                hidden_without_edges: result.hidden_without_edges,
                hidden_done: result.hidden_done,
            })
        })
        .await
    }

    /// Fortschritt der Specs, die auf dem Board vorkommen; gezählt wird projektweit
    /// (eine Spec gilt im ganzen Projekt)
    pub async fn for_board(&self, board_id: i32) -> Result<Vec<SpecProgressCount>> {
        tx!(self.db, |u| {
            let board = BoardService::<D>::require_in(u, board_id).await?;
            let all = u
                .tickets()
                .list(&TicketFilter {
                    without_texts: true,
                    ..TicketFilter::default()
                })
                .await?;
            let specs: Vec<String> = all
                .iter()
                .filter(|ticket| ticket.board_id == board_id)
                .filter_map(|ticket| ticket.spec.clone())
                .filter(|spec| !spec.is_empty())
                .collect();
            let of_project: Vec<_> = all
                .iter()
                .filter(|ticket| {
                    ticket.project_id == board.project_id
                        && ticket
                            .spec
                            .as_ref()
                            .is_some_and(|spec| specs.contains(spec))
                })
                .cloned()
                .collect();
            Ok(SpecProgress::count(&of_project, Some(&all)))
        })
        .await
    }

    /// Fortschritt aller Specs eines Projekts (ID)
    pub async fn for_project(&self, project_id: i32) -> Result<Vec<SpecProgressCount>> {
        tx!(self.db, |u| {
            let all = u
                .tickets()
                .list(&TicketFilter {
                    without_texts: true,
                    ..TicketFilter::default()
                })
                .await?;
            let of_project: Vec<_> = all
                .iter()
                .filter(|ticket| ticket.project_id == project_id)
                .cloned()
                .collect();
            Ok(SpecProgress::count(&of_project, Some(&all)))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::types::{Ticket, TicketStep};
    use crate::testing::BoardFixture;

    /// Board „Sprint“ mit A ← B, Board „Zweites“ mit C ← B; B und C gehören zur Spec 001-login.
    /// Die Tickets kommen unmittelbar aus den Repositories, weil `TicketService::create` erst
    /// Teil 2 des Tickets P3-2 bringt.
    async fn setup() -> (BoardFixture, i32) {
        let f = BoardFixture::create().await;
        let second = f
            .boards
            .create("ARC", "Zweites", &f.actor)
            .await
            .unwrap()
            .id;
        f.insert_ticket(&Ticket {
            title: "A".into(),
            ..f.new_ticket(1).await
        })
        .await;
        f.insert_ticket(&Ticket {
            title: "B".into(),
            depends_on: vec!["ARC-0001".into()],
            spec: Some("001-login".into()),
            ..f.new_ticket(2).await
        })
        .await;
        f.insert_ticket(&Ticket {
            title: "C".into(),
            board_id: second,
            depends_on: vec!["ARC-0002".into()],
            spec: Some("001-login".into()),
            ..f.new_ticket(3).await
        })
        .await;
        f.insert_ticket(&Ticket {
            title: "Allein".into(),
            ..f.new_ticket(4).await
        })
        .await;
        (f, second)
    }

    #[tokio::test]
    async fn liefert_den_graph_eines_tickets_uber_boards_mit_board_namen() {
        let (f, second) = setup().await;

        let g = f
            .graph
            .build(&GraphRequest::Ticket { id: "arc-2".into() }, None)
            .await
            .unwrap();

        assert_eq!(
            g.scope,
            GraphRequest::Ticket {
                id: "ARC-0002".into()
            }
        );
        assert_eq!(
            g.nodes
                .iter()
                .map(|node| (node.node.id.as_str(), node.board_name.as_str()))
                .collect::<Vec<_>>(),
            [
                ("ARC-0001", "Sprint"),
                ("ARC-0002", "Sprint"),
                ("ARC-0003", "Zweites"),
            ]
        );
        assert_eq!(g.nodes[2].node.board_id, second);
        assert_eq!(g.critical_path, ["ARC-0001", "ARC-0002"]);
    }

    #[tokio::test]
    async fn lost_den_projekt_key_der_spec_auf_und_zahlt_kantenlose_board_tickets() {
        let (f, _second) = setup().await;

        let spec = f
            .graph
            .build(
                &GraphRequest::Spec {
                    project: "arc".into(),
                    spec: "001-login".into(),
                },
                None,
            )
            .await
            .unwrap();

        assert_eq!(
            spec.scope,
            GraphRequest::Spec {
                project: "ARC".into(),
                spec: "001-login".into()
            }
        );
        assert_eq!(
            spec.nodes
                .iter()
                .map(|node| (node.node.id.as_str(), node.node.external))
                .collect::<Vec<_>>(),
            [("ARC-0001", true), ("ARC-0002", false), ("ARC-0003", false),]
        );

        let board = f
            .graph
            .build(
                &GraphRequest::Board {
                    board_id: f.board_id,
                },
                None,
            )
            .await
            .unwrap();

        assert_eq!(board.hidden_without_edges, 1);
        assert_eq!(
            board
                .nodes
                .iter()
                .map(|node| node.node.id.as_str())
                .collect::<Vec<_>>(),
            ["ARC-0001", "ARC-0002", "ARC-0003"]
        );
    }

    #[tokio::test]
    async fn knoten_tragen_den_fortschritt_ihrer_schritte() {
        let (f, _second) = setup().await;
        let step = |text: &str, done: bool| TicketStep {
            text: text.into(),
            done,
            detail: None,
        };
        let steps = vec![step("eins", true), step("zwei", true), step("drei", false)];
        crate::tx!(f.db, [steps], |u| {
            u.steps().replace("ARC-0002", &steps).await
        })
        .await
        .unwrap();

        let g = f
            .graph
            .build(
                &GraphRequest::Ticket {
                    id: "ARC-0002".into(),
                },
                None,
            )
            .await
            .unwrap();

        assert_eq!(
            g.nodes
                .iter()
                .map(|node| (node.node.id.as_str(), node.steps_done, node.steps_total))
                .collect::<Vec<_>>(),
            [("ARC-0001", 0, 0), ("ARC-0002", 2, 3), ("ARC-0003", 0, 0)]
        );
    }

    #[tokio::test]
    async fn meldet_unbekannte_tickets_projekte_und_boards() {
        let (f, _second) = setup().await;

        for (request, code) in [
            (
                GraphRequest::Ticket {
                    id: "ARC-0099".into(),
                },
                "ticket-not-found",
            ),
            (
                GraphRequest::Spec {
                    project: "XYZ".into(),
                    spec: "x".into(),
                },
                "project-not-found",
            ),
            (GraphRequest::Board { board_id: 999 }, "board-not-found"),
        ] {
            let error = f.graph.build(&request, None).await.unwrap_err();
            assert_eq!(error.code(), Some(code), "{request:?} müsste {code} melden");
        }
    }

    #[tokio::test]
    async fn zahlt_den_fortschritt_der_specs_eines_boards_projektweit() {
        let (f, second) = setup().await;
        let expected = [SpecProgressCount {
            spec: "001-login".into(),
            total: 2,
            waiting: 2,
            ..SpecProgressCount::default()
        }];

        assert_eq!(f.graph.for_board(f.board_id).await.unwrap(), expected);
        assert_eq!(f.graph.for_board(second).await.unwrap(), expected);
        assert_eq!(
            f.graph.for_project(f.project_id().await).await.unwrap(),
            expected
        );
    }
}
