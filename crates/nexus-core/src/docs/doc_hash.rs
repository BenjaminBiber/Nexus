//! Stabiler Fingerabdruck eines Dokumentinhalts; portiert aus
//! `packages/core/src/docs/DocHash.ts`.
//!
//! Zeilenenden und BOM hängen davon ab, wie ein Editor gespeichert hat – ohne
//! Normalisierung meldete das Speichern sonst falsche Konflikte.

use sha2::{Digest, Sha256};

/// Fingerabdruck eines Dokumentinhalts.
#[derive(Debug, Clone, Copy, Default)]
pub struct DocHash;

impl DocHash {
    /// Voller SHA-256 als Hex; voll statt gekürzt, weil der Wert Konflikte erkennen soll
    pub fn of(content: &str) -> String {
        let without_bom = content.strip_prefix('\u{feff}').unwrap_or(content);
        let normalized = without_bom.replace("\r\n", "\n");
        let digest = Sha256::digest(normalized.as_bytes());
        digest
            .iter()
            .fold(String::with_capacity(64), |mut text, byte| {
                use std::fmt::Write;
                let _ = write!(text, "{byte:02x}");
                text
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behandelt_crlf_wie_lf() {
        assert_eq!(DocHash::of("# A\r\neins\r\n"), DocHash::of("# A\neins\n"));
    }

    #[test]
    fn unterscheidet_verschiedene_inhalte() {
        assert_ne!(DocHash::of("eins"), DocHash::of("zwei"));
    }

    #[test]
    fn ubergeht_ein_fuhrendes_bom() {
        assert_eq!(DocHash::of("\u{feff}# A"), DocHash::of("# A"));
    }

    #[test]
    fn liefert_64_hex_zeichen() {
        let hash = DocHash::of("eins");
        assert_eq!(hash.len(), 64);
        assert!(hash
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
    }

    /// Gegenprobe zu TypeScript: derselbe Inhalt wie im Vertrag
    /// (`contract/api/docs/tree-file-search.golden.json`, Schritt „Glossar lesen“) muss
    /// denselben Fingerabdruck ergeben wie `DocHash.of` in TypeScript
    #[test]
    fn liefert_denselben_wert_wie_typescript() {
        assert_eq!(
            DocHash::of("# Glossar\n\n**Passkey**: Schlüsselpaar statt Kennwort.\n"),
            "541efc5a5bc2158f0cc1f4b15bd1383a40ca6f0ae328927a3d2eef14811cd46e"
        );
        assert_eq!(
            DocHash::of(
                "# Speicher der Schlüssel\n\nDie Schlüssel liegen im Schlüsselbund des Systems.\n"
            ),
            "eafe44d4d889c45ee39d8a5ad106ec269031a8a1eb7f54c10af820ca3c122fff"
        );
    }
}
