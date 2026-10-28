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

/// Ein Knoten mit dem Namen seines Boards
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct GraphNodeView {
    #[serde(flatten)]
    #[ts(flatten)]
    pub node: GraphNode,
    pub board_name: String,
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
            Ok(DependencyGraphView {
                scope,
                nodes: result
                    .nodes
                    .into_iter()
                    .map(|node| GraphNodeView {
                        board_name: board_names
                            .get(&node.board_id)
                            .map_or("?", String::as_str)
                            .to_string(),
                        node,
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
