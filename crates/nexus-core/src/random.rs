//! Zufallswerte für Sperren, temporäre Dateien und Token. Entspricht
//! `randomBytes(n).toString('hex')` aus TypeScript und kommt wie dort aus der
//! Zufallsquelle des Betriebssystems.

/// `count` Zufallsbytes als Hex-Text (zwei Zeichen je Byte)
pub fn hex(count: usize) -> String {
    let mut bytes = vec![0u8; count];
    fill(&mut bytes);
    bytes
        .iter()
        .fold(String::with_capacity(count * 2), |mut text, byte| {
            use std::fmt::Write;
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// `count` Zufallsbytes, wie `randomBytes(n)` in TypeScript
pub fn bytes(count: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; count];
    fill(&mut bytes);
    bytes
}

/// Zufallszahl zwischen 0 (einschließlich) und 1 (ausschließlich), wie `Math.random()`
/// in der wachsenden Pause beim Warten auf eine Sperre
pub(crate) fn fraction() -> f64 {
    let mut byte = [0u8; 1];
    fill(&mut byte);
    f64::from(byte[0]) / 256.0
}

/// Füllt den Puffer aus der Zufallsquelle des Systems; ohne sie sind weder Token
/// noch Sperren sicher, deshalb bricht der Prozess hier ab statt schwach zu raten
fn fill(bytes: &mut [u8]) {
    getrandom::fill(bytes).expect("Zufallsquelle des Systems ist nicht verfügbar");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liefert_hex_in_der_erwarteten_lange() {
        assert_eq!(hex(8).len(), 16);
        assert_eq!(hex(32).len(), 64);
        assert!(hex(16).chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn liefert_zwei_verschiedene_kennzeichen() {
        assert_ne!(hex(8), hex(8));
    }

    #[test]
    fn bleibt_zwischen_null_und_eins() {
        for _ in 0..50 {
            let value = fraction();
            assert!((0.0..1.0).contains(&value), "{value}");
        }
    }
}
