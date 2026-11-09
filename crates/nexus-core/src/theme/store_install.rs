//! Prüft ein aus dem Store geladenes Theme-Paket, bevor es installiert wird: sha256 gegen den
//! Index-Eintrag (Pflicht), danach eine optionale minisign-Signatur gegen den Schlüssel des
//! Repos und zuletzt, ob `id` und `version` im Manifest zum Eintrag passen. Alles läuft im
//! Speicher; installiert wird erst danach über `themes::install_theme`.

use std::io::{Cursor, Read};

use minisign_verify::{PublicKey, Signature};
use sha2::{Digest, Sha256};

use super::store_index::StoreIndexEntry;
use crate::error::HarnessError;
use crate::themes::parse_manifest;

/// Fehlercode: Prüfsumme des Pakets passt nicht zum Index
pub const STORE_CHECKSUM_MISMATCH: &str = "store-checksum-mismatch";
/// Fehlercode: Signatur kaputt oder passt nicht zum Schlüssel des Repos
pub const STORE_SIGNATURE_INVALID: &str = "store-signature-invalid";
/// Fehlercode: `id` oder `version` im Manifest weichen vom Index ab
pub const STORE_MANIFEST_MISMATCH: &str = "store-manifest-mismatch";

/// Höchstgröße von `manifest.json` beim Abgleich
const MAX_MANIFEST_BYTES: u64 = 1 << 20;

/// Ergebnis der Prüfung
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    /// sha256 des Pakets, Hex klein
    pub sha256: String,
    /// Ob eine gültige Signatur zum Schlüssel des Repos vorlag
    pub verified: bool,
}

/// Prüft sha256 und – wenn Repo-Schlüssel und Signatur vorhanden sind – die Signatur.
/// Falsche Prüfsumme → `store-checksum-mismatch`, ungültige Signatur →
/// `store-signature-invalid`; ohne Schlüssel oder ohne Signatur ist das Paket nicht verifiziert
pub fn verify_package(
    bytes: &[u8],
    entry: &StoreIndexEntry,
    public_key: Option<&str>,
) -> Result<Verification, HarnessError> {
    let sha256 = sha256_hex(bytes);
    if !sha256.eq_ignore_ascii_case(&entry.sha256) {
        return Err(HarnessError::new(
            STORE_CHECKSUM_MISMATCH,
            format!(
                "Die Prüfsumme des Pakets stimmt nicht: erwartet {}…, erhalten {}….",
                &entry.sha256[..12.min(entry.sha256.len())],
                &sha256[..12]
            ),
        ));
    }
    let verified = match (public_key, entry.signature.as_deref()) {
        (Some(key), Some(signature)) => {
            check_signature(bytes, key, signature)?;
            true
        }
        _ => false,
    };
    Ok(Verification { sha256, verified })
}

/// Liest `manifest.json` aus dem Paket und vergleicht `id` und `version` mit dem Eintrag;
/// Abweichung → `store-manifest-mismatch`. Kaputte Pakete lässt die Funktion durch, damit
/// `install_theme` sie mit seiner genauen Meldung ablehnt.
pub fn check_manifest_matches(bytes: &[u8], entry: &StoreIndexEntry) -> Result<(), HarnessError> {
    let Some(text) = read_manifest(bytes) else {
        return Ok(());
    };
    let Ok(manifest) = parse_manifest(&text) else {
        return Ok(());
    };
    if manifest.id != entry.id || manifest.version != entry.version {
        return Err(HarnessError::new(
            STORE_MANIFEST_MISMATCH,
            format!(
                "Das Paket passt nicht zum Store-Eintrag: erwartet {} {}, im Paket {} {}.",
                entry.id, entry.version, manifest.id, manifest.version
            ),
        ));
    }
    Ok(())
}

/// sha256 als Hex klein
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn check_signature(bytes: &[u8], key: &str, signature: &str) -> Result<(), HarnessError> {
    let invalid = |detail: &str| {
        HarnessError::new(
            STORE_SIGNATURE_INVALID,
            format!("Die Signatur des Pakets ist ungültig: {detail}"),
        )
    };
    let key = PublicKey::from_base64(key)
        .map_err(|_| invalid("der Schlüssel des Repos ist unlesbar."))?;
    let signature =
        Signature::decode(signature).map_err(|_| invalid("die Signatur ist unlesbar."))?;
    key.verify(bytes, &signature, false)
        .map_err(|_| invalid("sie passt nicht zum Schlüssel des Repos."))
}

/// Liest `manifest.json` aus dem Paket; kein ZIP oder kein Manifest → `None`
pub(crate) fn read_manifest(bytes: &[u8]) -> Option<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let file = archive.by_name("manifest.json").ok()?;
    let mut text = String::new();
    file.take(MAX_MANIFEST_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// Erzeugt minisign-Schlüssel und -Signaturen für Tests (Ed25519 über BLAKE2b-512 des
/// Pakets, Format wie `minisign -S`). Nur mit dem Merkmal `testing`.
#[cfg(any(test, feature = "testing"))]
pub mod test_signing {
    use base64::Engine;
    use blake2::{Blake2b512, Digest};
    use ed25519_dalek::{Signer, SigningKey};

    /// Ein Testschlüssel aus einem festen Startwert
    pub struct TestSigner {
        key: SigningKey,
        key_id: [u8; 8],
    }

    impl TestSigner {
        /// Schlüssel aus dem Startwert `seed`; gleicher Startwert, gleicher Schlüssel
        pub fn new(seed: u8) -> Self {
            Self {
                key: SigningKey::from_bytes(&[seed; 32]),
                key_id: [seed; 8],
            }
        }

        /// Öffentlicher Schlüssel als Base64-Zeile (`RW…`)
        pub fn public_key(&self) -> String {
            let mut raw = b"Ed".to_vec();
            raw.extend_from_slice(&self.key_id);
            raw.extend_from_slice(self.key.verifying_key().as_bytes());
            b64(&raw)
        }

        /// Inhalt einer `.minisig`-Datei für `bytes`
        pub fn sign(&self, bytes: &[u8]) -> String {
            let hash = Blake2b512::digest(bytes);
            let signature = self.key.sign(&hash).to_bytes();
            let mut raw = b"ED".to_vec();
            raw.extend_from_slice(&self.key_id);
            raw.extend_from_slice(&signature);
            let trusted = "timestamp:0\tfile:theme.nexus";
            let mut global = signature.to_vec();
            global.extend_from_slice(trusted.as_bytes());
            let global = self.key.sign(&global).to_bytes();
            format!(
                "untrusted comment: Testsignatur\n{}\ntrusted comment: {trusted}\n{}\n",
                b64(&raw),
                b64(&global)
            )
        }
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::test_signing::TestSigner;
    use super::*;

    fn paket(id: &str, version: &str) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("manifest.json", options).unwrap();
        let manifest = serde_json::json!({
            "id": id, "name": "Ozean", "version": version, "author": "Test",
            "sdkVersion": "^1.0", "colorScheme": "dark", "startColor": "#123456",
            "slots": [], "permissions": { "network": [] }
        });
        zip.write_all(manifest.to_string().as_bytes()).unwrap();
        zip.start_file("theme.css", options).unwrap();
        zip.write_all(b":root{}").unwrap();
        zip.finish().unwrap().into_inner()
    }

    fn eintrag(bytes: &[u8]) -> StoreIndexEntry {
        StoreIndexEntry {
            id: "ozean".into(),
            name: "Ozean".into(),
            version: "1.0.0".into(),
            sdk_version: "^1.0".into(),
            download_url: "https://example.com/ozean.nexus".into(),
            sha256: sha256_hex(bytes),
            preview: None,
            description: None,
            signature: None,
            compatible: true,
        }
    }

    #[test]
    fn store_install_richtige_pruefsumme_ohne_signatur() {
        let bytes = paket("ozean", "1.0.0");
        let mut entry = eintrag(&bytes);
        entry.sha256 = entry.sha256.to_uppercase();
        let result = verify_package(&bytes, &entry, None).unwrap();
        assert!(!result.verified);
        assert_eq!(result.sha256, sha256_hex(&bytes));
    }

    #[test]
    fn store_install_falsche_pruefsumme() {
        let bytes = paket("ozean", "1.0.0");
        let mut entry = eintrag(&bytes);
        entry.sha256 = "0".repeat(64);
        let error = verify_package(&bytes, &entry, None).unwrap_err();
        assert_eq!(error.code, STORE_CHECKSUM_MISMATCH);
        assert!(error.message.contains("erwartet 000000000000"));
    }

    #[test]
    fn store_install_gueltige_signatur() {
        let signer = TestSigner::new(1);
        let bytes = paket("ozean", "1.0.0");
        let mut entry = eintrag(&bytes);
        entry.signature = Some(signer.sign(&bytes));
        let key = signer.public_key();
        assert!(verify_package(&bytes, &entry, Some(&key)).unwrap().verified);
        // Ohne Schlüssel des Repos zählt die Signatur nicht
        assert!(!verify_package(&bytes, &entry, None).unwrap().verified);
    }

    #[test]
    fn store_install_ungueltige_signatur() {
        let signer = TestSigner::new(1);
        let bytes = paket("ozean", "1.0.0");
        let mut entry = eintrag(&bytes);
        entry.signature = Some(signer.sign(b"anderes paket"));
        let key = signer.public_key();
        let error = verify_package(&bytes, &entry, Some(&key)).unwrap_err();
        assert_eq!(error.code, STORE_SIGNATURE_INVALID);

        entry.signature = Some(signer.sign(&bytes));
        let fremd = TestSigner::new(2).public_key();
        let error = verify_package(&bytes, &entry, Some(&fremd)).unwrap_err();
        assert_eq!(error.code, STORE_SIGNATURE_INVALID);

        entry.signature = Some("kaputt".into());
        let error = verify_package(&bytes, &entry, Some(&key)).unwrap_err();
        assert_eq!(error.code, STORE_SIGNATURE_INVALID);
    }

    #[test]
    fn store_install_manifest_abgleich() {
        let bytes = paket("ozean", "1.0.0");
        let entry = eintrag(&bytes);
        check_manifest_matches(&bytes, &entry).unwrap();
        for (id, version) in [("wald", "1.0.0"), ("ozean", "2.0.0")] {
            let error = check_manifest_matches(&paket(id, version), &entry).unwrap_err();
            assert_eq!(error.code, STORE_MANIFEST_MISMATCH);
        }
        // Kein ZIP: die Installation meldet den genauen Fehler
        check_manifest_matches(b"kein zip", &entry).unwrap();
    }
}
