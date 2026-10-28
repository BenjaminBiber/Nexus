//! Blöcke und Parameterlisten für lange ID-Listen; portiert aus `packages/db/src/SqlChunks.ts`.

/// Höchstzahl der Einträge je Block; so bleibt eine Abfrage unter der Parametergrenze von
/// SQL Server (2100)
pub const SQL_CHUNK_SIZE: usize = 500;

/// Teilt lange ID-Listen in Blöcke, damit eine Abfrage unter der Parametergrenze von
/// SQL Server (2100) bleibt.
#[derive(Debug, Clone, Copy, Default)]
pub struct SqlChunks;

impl SqlChunks {
    /// Blöcke zu höchstens [`SQL_CHUNK_SIZE`] Einträgen; eine leere Liste ergibt keine Blöcke
    pub fn of<T>(items: &[T]) -> Vec<&[T]> {
        Self::of_size(items, SQL_CHUNK_SIZE)
    }

    /// Blöcke zu höchstens `size` Einträgen; eine leere Liste ergibt keine Blöcke
    pub fn of_size<T>(items: &[T], size: usize) -> Vec<&[T]> {
        if items.is_empty() {
            return Vec::new();
        }
        items.chunks(size.max(1)).collect()
    }

    /// Parameter `@<prefix>0`, `@<prefix>1` … und die passende IN-Liste für einen Block.
    /// Die Paare stehen in der Reihenfolge der Werte, damit der Treiber sie der Reihe nach bindet.
    pub fn params<T: Clone>(prefix: &str, values: &[T]) -> (String, Vec<(String, T)>) {
        let list = values
            .iter()
            .enumerate()
            .map(|(index, _)| format!("@{prefix}{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let params = values
            .iter()
            .enumerate()
            .map(|(index, value)| (format!("{prefix}{index}"), value.clone()))
            .collect();
        (list, params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eine_leere_liste_ergibt_keine_blocke() {
        let leer: [i32; 0] = [];
        assert!(SqlChunks::of(&leer).is_empty());
        assert!(SqlChunks::of_size(&leer, 3).is_empty());
    }

    #[test]
    fn genau_500_eintrage_bleiben_ein_block() {
        let ids: Vec<i32> = (0..500).collect();
        let blocke = SqlChunks::of(&ids);
        assert_eq!(blocke.len(), 1);
        assert_eq!(blocke[0].len(), 500);
    }

    #[test]
    fn der_501_eintrag_beginnt_einen_zweiten_block() {
        let ids: Vec<i32> = (0..501).collect();
        let blocke = SqlChunks::of(&ids);
        assert_eq!(blocke.len(), 2);
        assert_eq!(blocke[0].len(), 500);
        assert_eq!(blocke[1], [500]);
    }

    #[test]
    fn blocke_behalten_die_reihenfolge_der_liste() {
        let ids = ["a", "b", "c", "d", "e"];
        let blocke = SqlChunks::of_size(&ids, 2);
        assert_eq!(blocke, [&["a", "b"][..], &["c", "d"][..], &["e"][..]]);
    }

    #[test]
    fn parameter_zahlen_ab_null_und_passen_zur_in_liste() {
        let (list, params) = SqlChunks::params("id", &["NXS-0001", "NXS-0002"]);
        assert_eq!(list, "@id0, @id1");
        assert_eq!(
            params,
            [
                ("id0".to_string(), "NXS-0001"),
                ("id1".to_string(), "NXS-0002")
            ]
        );
    }

    #[test]
    fn eine_leere_liste_ergibt_eine_leere_in_liste() {
        let leer: [i32; 0] = [];
        let (list, params) = SqlChunks::params("b", &leer);
        assert_eq!(list, "");
        assert!(params.is_empty());
    }
}
