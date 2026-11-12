//! Einzelne Dateien aus einem ZIP/JAR (z. B. einem Client-JAR) für den Asset-Proxy der
//! Themes (D7).
//!
//! Sicherheitsregeln:
//! - Der Pfad des Eintrags ist relativ, ohne `..`, `\`, `:`, Steuerzeichen und höchstens
//!   [`MAX_ENTRY_PATH`] Zeichen lang; Verzeichnisse und symbolische Links werden abgelehnt.
//! - Die Einzeldatei ist auf ein Limit begrenzt, auch wenn das Archiv eine falsche Größe
//!   angibt (Lesen mit `take(limit + 1)`).
//! - Entpackt wird nur in den Speicher; auf die Platte schreibt erst der Aufrufer.

use std::io::{Cursor, Read};

/// Höchstlänge eines Eintragspfads
pub const MAX_ENTRY_PATH: usize = 512;

/// Warum eine Datei nicht aus dem Archiv kommt
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ArchiveEntryError {
    #[error("Unsicherer Pfad im Archiv: {path}")]
    UnsafePath { path: String },
    #[error("Die Daten sind kein ZIP-Archiv.")]
    NotArchive,
    #[error("Eintrag „{path}“ fehlt im Archiv.")]
    NotFound { path: String },
    #[error("Der Eintrag ist größer als {max} Bytes.")]
    TooLarge { max: u64 },
}

/// Prüft den angefragten Pfad im Archiv, bevor irgendetwas geladen wird
pub fn check_archive_entry_path(entry: &str) -> Result<&str, ArchiveEntryError> {
    let unsafe_path = || ArchiveEntryError::UnsafePath {
        path: entry.to_string(),
    };
    if entry.is_empty()
        || entry.len() > MAX_ENTRY_PATH
        || entry.starts_with('/')
        || entry.contains(['\\', ':'])
        || entry.chars().any(char::is_control)
        || entry
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(unsafe_path());
    }
    Ok(entry)
}

/// Liest genau einen Eintrag aus dem Archiv, höchstens `max_bytes` groß
pub fn extract_entry(
    archive: &[u8],
    entry: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, ArchiveEntryError> {
    let entry = check_archive_entry_path(entry)?;
    let mut zip =
        zip::ZipArchive::new(Cursor::new(archive)).map_err(|_| ArchiveEntryError::NotArchive)?;
    let not_found = || ArchiveEntryError::NotFound {
        path: entry.to_string(),
    };
    let file = match zip.by_name(entry) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Err(not_found()),
        Err(_) => return Err(ArchiveEntryError::NotArchive),
    };
    if file.is_dir() || file.is_symlink() || file.enclosed_name().is_none() {
        return Err(ArchiveEntryError::UnsafePath {
            path: entry.to_string(),
        });
    }
    let too_large = ArchiveEntryError::TooLarge { max: max_bytes };
    if file.size() > max_bytes {
        return Err(too_large);
    }
    let mut bytes = Vec::new();
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ArchiveEntryError::NotArchive)?;
    if bytes.len() as u64 > max_bytes {
        return Err(too_large);
    }
    Ok(bytes)
}

/// Inhaltstyp nach Endung; ausführbare Typen (HTML, JS) gehen als Binärdaten raus
pub fn content_type_for(entry: &str) -> &'static str {
    let extension = entry
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "json" | "mcmeta" => "application/json",
        "txt" | "lang" => "text/plain; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_mit(eintraege: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for (name, data) in eintraege {
            if name.ends_with('/') {
                writer.add_directory(*name, options).unwrap();
            } else {
                writer.start_file(*name, options).unwrap();
                writer.write_all(data).unwrap();
            }
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn eintrag_wird_geliefert() {
        let zip = zip_mit(&[("assets/a.png", b"PNG"), ("pack.mcmeta", b"{}")]);
        assert_eq!(extract_entry(&zip, "assets/a.png", 100).unwrap(), b"PNG");
        assert_eq!(extract_entry(&zip, "pack.mcmeta", 100).unwrap(), b"{}");
    }

    #[test]
    fn fehlender_eintrag_ist_not_found() {
        let zip = zip_mit(&[("a.png", b"PNG")]);
        assert!(matches!(
            extract_entry(&zip, "b.png", 100),
            Err(ArchiveEntryError::NotFound { .. })
        ));
    }

    #[test]
    fn unsichere_pfade_werden_abgelehnt() {
        for pfad in [
            "",
            "../x",
            "a/../b",
            "/etc/passwd",
            "a\\b",
            "C:/x",
            "a//b",
            "./a",
            "a\0b",
        ] {
            assert!(
                matches!(
                    check_archive_entry_path(pfad),
                    Err(ArchiveEntryError::UnsafePath { .. })
                ),
                "{pfad:?}"
            );
        }
        assert!(check_archive_entry_path(&"a".repeat(MAX_ENTRY_PATH + 1)).is_err());
        assert!(check_archive_entry_path("assets/minecraft/x.png").is_ok());
    }

    #[test]
    fn verzeichnis_wird_nicht_geliefert() {
        let zip = zip_mit(&[("assets/", b""), ("assets/a.png", b"PNG")]);
        assert!(extract_entry(&zip, "assets", 100).is_err());
        assert!(extract_entry(&zip, "assets/", 100).is_err());
    }

    #[test]
    fn zu_grosser_eintrag_ist_too_large() {
        let zip = zip_mit(&[("a.bin", &[7u8; 50])]);
        assert_eq!(
            extract_entry(&zip, "a.bin", 10),
            Err(ArchiveEntryError::TooLarge { max: 10 })
        );
        assert!(extract_entry(&zip, "a.bin", 50).is_ok());
    }

    #[test]
    fn kein_zip_ist_not_archive() {
        assert_eq!(
            extract_entry(b"kein zip", "a.png", 100),
            Err(ArchiveEntryError::NotArchive)
        );
    }

    #[test]
    fn inhaltstyp_nach_endung() {
        assert_eq!(content_type_for("a/b.PNG"), "image/png");
        assert_eq!(content_type_for("pack.mcmeta"), "application/json");
        assert_eq!(content_type_for("x.js"), "application/octet-stream");
        assert_eq!(content_type_for("x.html"), "application/octet-stream");
        assert_eq!(content_type_for("ohne"), "application/octet-stream");
    }
}
