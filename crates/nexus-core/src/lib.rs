//! Fachkern von Nexus: Typen und Regeln des Boards, unabhängig von Datenbank,
//! HTTP-Schicht und Oberfläche. Die Inhalte kommen in den folgenden Tickets.

/// Name des Crates, wie er in Protokollen und Diagnoseausgaben erscheint.
pub const CRATE_NAME: &str = "nexus-core";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_ist_gesetzt() {
        assert_eq!(CRATE_NAME, "nexus-core");
    }
}
