//! Standardwerte des Harness-Servers.

/// Standardport des Harness-Servers (selten belegt); änderbar mit `--port` oder `HARNESS_PORT`
pub const DEFAULT_SERVER_PORT: u16 = 4710;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standardport_bleibt_wie_in_typescript() {
        assert_eq!(DEFAULT_SERVER_PORT, 4710);
    }
}
