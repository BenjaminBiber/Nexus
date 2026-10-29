//! Wendet die gespeicherte, manuelle Reihenfolge auf die Einträge an; portiert aus
//! `packages/core/src/board/inbox/InboxOrder.ts`.

use std::collections::HashMap;

use crate::board::inbox::types::InboxItem;

/// Sortiert „Meine Aufgaben“ nach der gespeicherten Reihenfolge.
#[derive(Debug, Clone, Copy, Default)]
pub struct InboxOrder;

impl InboxOrder {
    /// Einträge ohne gespeicherte Position kommen zuerst (in der gegebenen Reihenfolge, also
    /// nach Dringlichkeit), damit Neues sichtbar ist; danach die einsortierten Einträge in
    /// gespeicherter Reihenfolge. Unbekannte Schlüssel fallen weg.
    pub fn apply(items: Vec<InboxItem>, keys: &[String]) -> Vec<InboxItem> {
        let mut position: HashMap<String, usize> = HashMap::new();
        for (index, key) in keys.iter().enumerate() {
            position.entry(key.clone()).or_insert(index);
        }
        let (mut placed, fresh): (Vec<InboxItem>, Vec<InboxItem>) = items
            .into_iter()
            .partition(|item| position.contains_key(&item.key));
        placed.sort_by_key(|item| position[&item.key]);
        fresh.into_iter().chain(placed).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::inbox::types::InboxKind;

    /// Ein blockierter Eintrag mit diesem Schlüssel
    fn item(key: &str) -> InboxItem {
        InboxItem {
            key: key.to_string(),
            kind: InboxKind::Blocked,
            title: key.to_string(),
            detail: None,
            board_id: Some(1),
            board_name: Some("B".into()),
            project_key: "AIH".into(),
            ticket_id: None,
            question_id: None,
            note_id: None,
            waiting_tickets: 0,
            priority: None,
            since: String::new(),
            rank: 5,
        }
    }

    /// Nur die Schlüssel der Einträge
    fn keys(items: &[InboxItem]) -> Vec<&str> {
        items.iter().map(|item| item.key.as_str()).collect()
    }

    /// Die gespeicherten Schlüssel als Liste
    fn saved(values: &[&str]) -> Vec<String> {
        values.iter().map(|key| key.to_string()).collect()
    }

    #[test]
    fn lasst_die_reihenfolge_ohne_gespeicherte_schlussel_unverandert() {
        let items = vec![item("ticket:A-1"), item("ticket:A-2")];
        assert_eq!(
            keys(&InboxOrder::apply(items, &[])),
            ["ticket:A-1", "ticket:A-2"]
        );
    }

    #[test]
    fn setzt_neue_eintrage_oben_gespeicherte_danach_in_datei_reihenfolge() {
        let items = vec![
            item("ticket:A-1"),
            item("note:1"),
            item("ticket:A-2"),
            item("question:A-F1"),
        ];
        assert_eq!(
            keys(&InboxOrder::apply(
                items,
                &saved(&["ticket:A-2", "ticket:A-1"])
            )),
            ["note:1", "question:A-F1", "ticket:A-2", "ticket:A-1"]
        );
    }

    #[test]
    fn ubergeht_schlussel_ohne_eintrag_und_doppelte_schlussel() {
        let items = vec![item("ticket:A-1"), item("ticket:A-2")];
        assert_eq!(
            keys(&InboxOrder::apply(
                items,
                &saved(&["ticket:A-9", "ticket:A-2", "ticket:A-1", "ticket:A-2"])
            )),
            ["ticket:A-2", "ticket:A-1"]
        );
    }
}
