//! Start-Token des Servers: damit weist sich die Oberfläche der Desktop-App aus.
//!
//! Das Token entsteht bei jedem Serverstart neu, steht in `server.json` (Feld `appToken`)
//! und gehört damit zu den Angaben des laufenden Servers. Es darf nirgends im Klartext
//! auftauchen – weder im Protokoll noch in einer Fehlermeldung –, deshalb zeigt auch
//! `Debug` nur einen Platzhalter.

use serde::{Deserialize, Serialize};

use crate::random;

/// So viele Zufallsbytes stecken im Token
const BYTES: usize = 32;

/// Start-Token eines Servers
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AppToken(String);

impl AppToken {
    /// Ein frisches Token aus der Zufallsquelle des Systems (32 Byte als Base64url)
    pub fn new() -> Self {
        Self(base64url(&random::bytes(BYTES)))
    }

    /// Ein vorhandenes Token übernehmen (aus `server.json`)
    pub fn from_text(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// Das Token als Text; nur zum Weitergeben, nie zum Protokollieren
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Stimmt das mitgeschickte Token? Der Vergleich braucht immer gleich lange, damit
    /// niemand das Token Zeichen für Zeichen erraten kann.
    pub fn matches(&self, candidate: &str) -> bool {
        gleich_lang(self.0.as_bytes(), candidate.as_bytes())
    }
}

impl Default for AppToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Zeigt nie den Wert: sonst stünde das Token in jeder Ausgabe der Einstellungen
impl std::fmt::Debug for AppToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AppToken(…)")
    }
}

/// Vergleicht zwei Byte-Folgen ohne vorzeitigen Abbruch; unterschiedliche Längen sind
/// sofort falsch, aber alle gleich langen Kandidaten kosten dieselbe Zeit
fn gleich_lang(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut unterschied = 0u8;
    for (a, b) in left.iter().zip(right) {
        unterschied |= a ^ b;
    }
    unterschied == 0
}

/// Alphabet von Base64url (RFC 4648 §5): statt `+` und `/` stehen `-` und `_`
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Bytes als Base64url ohne Auffüllzeichen; der Wert steht in einer Adresse und in einem
/// Kopf, darum keine Zeichen, die dort erst kodiert werden müssten
fn base64url(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for block in bytes.chunks(3) {
        let mut puffer = [0u8; 3];
        puffer[..block.len()].copy_from_slice(block);
        let zahl =
            (u32::from(puffer[0]) << 16) | (u32::from(puffer[1]) << 8) | u32::from(puffer[2]);
        // Je drei Bytes werden vier Zeichen; ein angebrochener Block liefert entsprechend weniger
        for index in 0..block.len() + 1 {
            let sechs = (zahl >> (18 - index * 6)) & 0b11_1111;
            text.push(char::from(ALPHABET[sechs as usize]));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erzeugt_jedes_mal_ein_anderes_token() {
        let erstes = AppToken::new();
        let zweites = AppToken::new();
        assert_ne!(erstes.as_str(), zweites.as_str());
    }

    #[test]
    fn nutzt_nur_zeichen_aus_base64url() {
        let token = AppToken::new();
        // 32 Byte sind 43 Zeichen ohne Auffüllzeichen
        assert_eq!(token.as_str().len(), 43);
        assert!(token
            .as_str()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn kodiert_wie_base64url_ohne_auffuellzeichen() {
        // Vergleichswerte aus `Buffer.from(…).toString('base64url')`
        assert_eq!(base64url(b""), "");
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url(b"foob"), "Zm9vYg");
        assert_eq!(base64url(b"fooba"), "Zm9vYmE");
        assert_eq!(base64url(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64url(&[0xfb, 0xff, 0xfe]), "-__-");
    }

    #[test]
    fn erkennt_nur_das_gleiche_token() {
        let token = AppToken::from_text("abc");
        assert!(token.matches("abc"));
        assert!(!token.matches("abd"));
        assert!(!token.matches("ab"));
        assert!(!token.matches("abcd"));
        assert!(!token.matches(""));
    }

    #[test]
    fn zeigt_den_wert_nicht_in_der_debug_ausgabe() {
        let token = AppToken::from_text("geheim");
        assert_eq!(format!("{token:?}"), "AppToken(…)");
        assert!(!format!("{token:?}").contains("geheim"));
    }
}
