//! Datenzugriff von Nexus: liest und schreibt die unveränderten Schemata von
//! SQLite (`board.db`) und SQL Server. Die Inhalte kommen in den folgenden Tickets.

/// Name des Crates, wie er in Protokollen und Diagnoseausgaben erscheint.
pub const CRATE_NAME: &str = "nexus-db";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_ist_gesetzt() {
        assert_eq!(CRATE_NAME, "nexus-db");
    }

    #[test]
    fn fachkern_ist_eingebunden() {
        assert_eq!(nexus_core::CRATE_NAME, "nexus-core");
    }
}
