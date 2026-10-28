//! Gruppierung von Kind-Zeilen; portiert aus `packages/db/src/RowGroups.ts`.

use std::collections::HashMap;

/// Ordnet Kind-Zeilen (Labels, Voraussetzungen) ihrem Ticket zu – einmal gruppiert, statt je
/// Ticket alle Zeilen zu filtern.
#[derive(Debug, Clone, Copy, Default)]
pub struct RowGroups;

impl RowGroups {
    /// Werte je Schlüssel in der Reihenfolge der Zeilen
    pub fn by<R, V, K, W>(rows: &[R], key: K, value: W) -> HashMap<String, Vec<V>>
    where
        K: Fn(&R) -> String,
        W: Fn(&R) -> V,
    {
        let mut groups: HashMap<String, Vec<V>> = HashMap::new();
        for row in rows {
            groups.entry(key(row)).or_default().push(value(row));
        }
        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eine Kind-Zeile, wie sie aus der DB kommt: Ticket und ein Wert
    struct Zeile(&'static str, &'static str);

    #[test]
    fn gruppiert_die_werte_nach_ihrem_schlussel() {
        let zeilen = [
            Zeile("NXS-0001", "rust"),
            Zeile("NXS-0002", "db"),
            Zeile("NXS-0001", "phase-2"),
        ];
        let gruppen = RowGroups::by(&zeilen, |z| z.0.to_string(), |z| z.1);
        assert_eq!(gruppen.len(), 2);
        assert_eq!(gruppen["NXS-0001"], ["rust", "phase-2"]);
        assert_eq!(gruppen["NXS-0002"], ["db"]);
    }

    #[test]
    fn behalt_die_reihenfolge_der_zeilen_auch_bei_vielen_werten() {
        let zeilen: Vec<Zeile> = (0..3).map(|_| Zeile("NXS-0001", "a")).collect();
        let gruppen = RowGroups::by(&zeilen, |z| z.0.to_string(), |z| z.1);
        assert_eq!(gruppen["NXS-0001"], ["a", "a", "a"]);

        let gemischt = [
            Zeile("b", "1"),
            Zeile("a", "2"),
            Zeile("b", "3"),
            Zeile("a", "4"),
        ];
        let gruppen = RowGroups::by(&gemischt, |z| z.0.to_string(), |z| z.1);
        assert_eq!(gruppen["b"], ["1", "3"]);
        assert_eq!(gruppen["a"], ["2", "4"]);
    }

    #[test]
    fn ohne_zeilen_gibt_es_keine_gruppen() {
        let leer: [Zeile; 0] = [];
        let gruppen = RowGroups::by(&leer, |z| z.0.to_string(), |z| z.1);
        assert!(gruppen.is_empty());
    }
}
