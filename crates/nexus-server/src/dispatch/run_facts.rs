//! Ergänzt die Protokolle eines Tickets um das, was die Board-Datenbank über den Lauf weiß:
//! Ergebnis, Abbruchgrund, Ende, Dauer und Modell. Ohne Board, ohne Treffer oder bei
//! Zeitüberschreitung bleibt die Liste, wie sie ist.

use std::sync::Arc;
use std::time::Duration;

use super::status::DispatchRunLog;
use crate::sessions::session_details::started_at;
use crate::state::AppState;

/// Länger wartet die Liste nicht auf das Board
const BOARD_TIMEOUT: Duration = Duration::from_secs(2);

/// Trägt zu jedem Protokoll die Fakten des aufgezeichneten Laufs ein – mit einer Abfrage für alle
pub async fn with_facts(
    state: &Arc<AppState>,
    mut runs: Vec<DispatchRunLog>,
) -> Vec<DispatchRunLog> {
    let ids: Vec<String> = runs
        .iter()
        .map(|run| {
            run.name
                .strip_suffix(".log")
                .unwrap_or(&run.name)
                .to_string()
        })
        .collect();
    // Das Zeitfenster reicht vom frühesten bis zum spätesten Start, je einen Tag großzügiger
    let starts: Vec<_> = ids.iter().filter_map(|id| started_at(id)).collect();
    let (Some(first), Some(last)) = (starts.iter().min(), starts.iter().max()) else {
        return runs;
    };
    let from = *first - chrono::Duration::days(1);
    let to = *last + chrono::Duration::days(1);
    let lookup = state.board.run(move |backend| async move {
        backend.services().dispatch_runs.list_all(from, to).await
    });
    let Ok(Ok(recorded)) = tokio::time::timeout(BOARD_TIMEOUT, lookup).await else {
        return runs;
    };
    for (run, id) in runs.iter_mut().zip(&ids) {
        // Der genaue Name gewinnt vor einem Lauf, dessen ID nur damit beginnt
        let prefix = format!("{id}-");
        let fact = recorded
            .iter()
            .find(|fact| fact.id == *id)
            .or_else(|| recorded.iter().find(|fact| fact.id.starts_with(&prefix)));
        if let Some(fact) = fact {
            run.outcome = Some(fact.outcome.as_str().to_string());
            run.abort_reason = fact.abort_reason.clone();
            run.ended_at = fact.ended_at.clone();
            run.duration_ms = fact.duration_ms.map(i64::from);
            run.model = fact.model.clone();
        }
    }
    runs
}
