//! Installieren, Auflisten und Entfernen von Theme-Paketen (`.nexus` = ZIP mit `manifest.json`,
//! `theme.css`, optional `main.js` und `assets/**`).
//!
//! Ein Paket wird vollständig im Speicher geprüft, bevor etwas auf die Platte kommt: sichere
//! Pfade, erlaubte Dateitypen, Größenlimits, gültiges Manifest und ein `theme.css` ohne
//! `@import` und ohne externe `url()`. Danach wird es in einen temporären Ordner unter
//! `themes/` entpackt und erst dann an seinen Platz umbenannt; eine vorhandene Installation
//! bleibt bis dahin unverändert.

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::manifest::{parse_manifest, ManifestErrors, ThemeManifest, THEME_ID_PATTERN};
use crate::NexusHome;

/// Höchstgröße eines Pakets und Höchstsumme seiner entpackten Dateien
pub const MAX_PACKAGE_BYTES: u64 = 20 * 1024 * 1024;
/// Höchstgröße einer entpackten Einzeldatei
pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// Höchstzahl der Einträge eines Pakets
pub const MAX_ENTRIES: usize = 500;

/// Erlaubte Dateiendungen (klein geschrieben): Code, Stil, Daten, Bilder, Schriften, Klänge
const ALLOWED_EXTENSIONS: &[&str] = &[
    "css", "js", "mjs", "json", "png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "ico", "woff",
    "woff2", "ttf", "otf", "mp3", "ogg", "wav",
];

/// Pflichtdateien im Wurzelordner eines Pakets
const MANIFEST_FILE: &str = "manifest.json";
const THEME_CSS_FILE: &str = "theme.css";

/// CSS-Kommentare
static CSS_COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)/\*.*?\*/").unwrap());
/// `@import` in jeder Schreibweise
static CSS_IMPORT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)@import").unwrap());
/// `url(...)` mit oder ohne Anführungszeichen
static CSS_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)url\(\s*("[^"]*"|'[^']*'|[^)]*?)\s*\)"#).unwrap());

/// Fehler beim Installieren oder Entfernen eines Theme-Pakets
#[derive(Debug, thiserror::Error)]
pub enum ThemePackageError {
    #[error("Das Paket ist zu groß (höchstens {max} Bytes, entpackt wie gepackt).")]
    TooLarge { max: u64 },
    #[error("Die Datei {path} ist zu groß (höchstens {max} Bytes).")]
    FileTooLarge { path: String, max: u64 },
    #[error("Das Paket hat zu viele Einträge (höchstens {max}).")]
    TooManyEntries { max: usize },
    #[error("Unsicherer Pfad im Paket: {path}")]
    UnsafePath { path: String },
    #[error("Dateityp nicht erlaubt: {path}")]
    ForbiddenFileType { path: String },
    #[error("Im Paket fehlt {name}.")]
    MissingFile { name: String },
    #[error("Das Manifest ist ungültig: {0}")]
    Manifest(#[from] ManifestErrors),
    #[error("theme.css ist nicht erlaubt: {reason}")]
    ForbiddenCss { reason: String },
    #[error("Das Paket ist kein gültiges ZIP-Archiv: {0}")]
    NotZip(String),
    #[error("Das Theme {id} ist nicht installiert.")]
    NotInstalled { id: String },
    #[error("Ungültige Theme-ID: {id}")]
    InvalidId { id: String },
    #[error("manifest.json ist ungültig: {0}")]
    ManifestFile(ManifestErrors),
    #[error("Versteckte Dateien und Ordner sind nicht erlaubt: {path}")]
    HiddenFile { path: String },
    #[error("Symbolische Links sind nicht erlaubt: {path}")]
    Symlink { path: String },
    #[error("Ordner nicht gefunden: {path}")]
    DirNotFound { path: String },
    #[error("Dateifehler: {0}")]
    Io(#[from] std::io::Error),
}

/// Ein aus einem Ordner gebautes Paket
#[derive(Debug, Clone)]
pub struct PackedTheme {
    pub manifest: ThemeManifest,
    /// Inhalt der `.nexus`-Datei
    pub bytes: Vec<u8>,
    /// Zahl der Dateien im Paket
    pub files: usize,
}

/// Ein installiertes Theme mit seinem Ordner
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledTheme {
    pub manifest: ThemeManifest,
    /// `<NEXUS_HOME>/themes/<id>/`
    pub dir: PathBuf,
}

/// Grenzen eines Pakets; in Tests kleiner, damit keine Megabytes an Testdaten nötig sind
#[derive(Debug, Clone, Copy)]
struct Limits {
    package: u64,
    file: u64,
    entries: usize,
}

const DEFAULT_LIMITS: Limits = Limits {
    package: MAX_PACKAGE_BYTES,
    file: MAX_FILE_BYTES,
    entries: MAX_ENTRIES,
};

/// Installiert ein Paket aus einer Datei, siehe [`install_theme`]
pub fn install_theme_file(
    home: &NexusHome,
    path: &Path,
) -> Result<ThemeManifest, ThemePackageError> {
    let size = fs::metadata(path)?.len();
    if size > MAX_PACKAGE_BYTES {
        return Err(ThemePackageError::TooLarge {
            max: MAX_PACKAGE_BYTES,
        });
    }
    install_theme(home, &fs::read(path)?)
}

/// Prüft ein `.nexus`-Paket und installiert es nach `<NEXUS_HOME>/themes/<id>/`. Eine
/// vorhandene Installation derselben id wird vollständig ersetzt; schlägt etwas fehl, bleibt
/// sie unverändert und unter `themes/` bleibt nichts zurück.
pub fn install_theme(home: &NexusHome, package: &[u8]) -> Result<ThemeManifest, ThemePackageError> {
    install_with_limits(home, package, DEFAULT_LIMITS)
}

fn install_with_limits(
    home: &NexusHome,
    package: &[u8],
    limits: Limits,
) -> Result<ThemeManifest, ThemePackageError> {
    let files = read_package(package, limits)?;

    let file = |name: &str| {
        files
            .iter()
            .find(|(path, _)| path == Path::new(name))
            .map(|(_, data)| data)
            .ok_or_else(|| ThemePackageError::MissingFile {
                name: name.to_string(),
            })
    };
    let manifest = parse_manifest(&String::from_utf8_lossy(file(MANIFEST_FILE)?))?;
    let css = std::str::from_utf8(file(THEME_CSS_FILE)?).map_err(|_| {
        ThemePackageError::ForbiddenCss {
            reason: "kein gültiges UTF-8".to_string(),
        }
    })?;
    check_theme_css(css)?;

    let themes_dir = home.themes_dir();
    fs::create_dir_all(&themes_dir)?;
    let staging = tempfile::Builder::new()
        .prefix(&format!(".tmp-{}-", manifest.id))
        .tempdir_in(&themes_dir)?;
    for (path, data) in &files {
        let target = staging.path().join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, data)?;
    }

    let target = themes_dir.join(&manifest.id);
    // Die alte Installation wandert in einen eigenen temporären Ordner, der sie beim Drop
    // löscht; scheitert das Umbenennen des neuen Pakets, kommt sie zurück.
    let old = if target.exists() {
        let old = tempfile::Builder::new()
            .prefix(&format!(".old-{}-", manifest.id))
            .tempdir_in(&themes_dir)?;
        fs::rename(&target, old.path().join("theme"))?;
        Some(old)
    } else {
        None
    };
    if let Err(error) = fs::rename(staging.path(), &target) {
        if let Some(old) = &old {
            fs::rename(old.path().join("theme"), &target)?;
        }
        return Err(error.into());
    }
    drop(old);
    Ok(manifest)
}

/// Baut aus einem Theme-Ordner ein `.nexus`-Paket mit denselben Prüfungen wie bei der
/// Installation. Versteckte Dateien und Ordner sowie symbolische Links werden abgelehnt,
/// nicht übersprungen. Reihenfolge, Zeitstempel und das eingetragene System sind fest, damit
/// derselbe Ordner auf jeder Plattform byte-gleiche Pakete ergibt.
pub fn pack_theme(dir: &Path) -> Result<PackedTheme, ThemePackageError> {
    pack_with_limits(dir, DEFAULT_LIMITS)
}

fn pack_with_limits(dir: &Path, limits: Limits) -> Result<PackedTheme, ThemePackageError> {
    if !dir.is_dir() {
        return Err(ThemePackageError::DirNotFound {
            path: dir.display().to_string(),
        });
    }
    let mut files = Vec::new();
    collect_files(dir, "", limits, &mut files)?;

    let file = |name: &str| {
        files
            .iter()
            .find(|(path, _)| path == name)
            .map(|(_, data)| data)
            .ok_or_else(|| ThemePackageError::MissingFile {
                name: name.to_string(),
            })
    };
    let manifest = parse_manifest(&String::from_utf8_lossy(file(MANIFEST_FILE)?))
        .map_err(ThemePackageError::ManifestFile)?;
    let css = std::str::from_utf8(file(THEME_CSS_FILE)?).map_err(|_| {
        ThemePackageError::ForbiddenCss {
            reason: "kein gültiges UTF-8".to_string(),
        }
    })?;
    check_theme_css(css)?;

    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    // Ohne `system` trägt zip das System ein, auf dem gepackt wird (Windows: DOS, sonst Unix)
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .system(zip::System::Unix)
        .unix_permissions(0o644);
    let zip_error = |error: zip::result::ZipError| ThemePackageError::NotZip(error.to_string());
    for (path, data) in &files {
        writer
            .start_file(path.as_str(), options)
            .map_err(zip_error)?;
        std::io::Write::write_all(&mut writer, data)?;
    }
    let bytes = writer.finish().map_err(zip_error)?.into_inner();
    // Selbstprüfung: das Paket muss die Prüfung der Installation bestehen
    read_package(&bytes, limits)?;
    Ok(PackedTheme {
        manifest,
        bytes,
        files: files.len(),
    })
}

/// Sammelt die Dateien eines Ordners rekursiv und sortiert, mit relativem Pfad (`/`-getrennt)
fn collect_files(
    dir: &Path,
    prefix: &str,
    limits: Limits,
    files: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), ThemePackageError> {
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(ThemePackageError::Symlink { path });
        }
        if name.starts_with('.') {
            return Err(ThemePackageError::HiddenFile { path });
        }
        check_entry_path(&path)?;
        if file_type.is_dir() {
            collect_files(&entry.path(), &format!("{path}/"), limits, files)?;
            continue;
        }
        if !has_allowed_extension(Path::new(&path)) {
            return Err(ThemePackageError::ForbiddenFileType { path });
        }
        if files.len() >= limits.entries {
            return Err(ThemePackageError::TooManyEntries {
                max: limits.entries,
            });
        }
        if entry.metadata()?.len() > limits.file {
            return Err(ThemePackageError::FileTooLarge {
                path,
                max: limits.file,
            });
        }
        let data = fs::read(entry.path())?;
        let total: u64 = files.iter().map(|(_, d)| d.len() as u64).sum::<u64>() + data.len() as u64;
        if total > limits.package {
            return Err(ThemePackageError::TooLarge {
                max: limits.package,
            });
        }
        files.push((path, data));
    }
    Ok(())
}

/// Liest und prüft alle Einträge eines Pakets; Ergebnis sind relative Pfade mit Inhalt
fn read_package(
    package: &[u8],
    limits: Limits,
) -> Result<Vec<(PathBuf, Vec<u8>)>, ThemePackageError> {
    if package.len() as u64 > limits.package {
        return Err(ThemePackageError::TooLarge {
            max: limits.package,
        });
    }
    let not_zip = |error: zip::result::ZipError| ThemePackageError::NotZip(error.to_string());
    let mut archive = zip::ZipArchive::new(Cursor::new(package)).map_err(not_zip)?;
    if archive.len() > limits.entries {
        return Err(ThemePackageError::TooManyEntries {
            max: limits.entries,
        });
    }

    let mut files = Vec::new();
    let mut total: u64 = 0;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(not_zip)?;
        let name = entry.name().map_err(not_zip)?.to_string();
        let unsafe_path = || ThemePackageError::UnsafePath { path: name.clone() };
        if entry.is_symlink() || entry.enclosed_name().is_none() {
            return Err(unsafe_path());
        }
        let path = check_entry_path(&name)?;
        if entry.is_dir() {
            continue;
        }
        if !has_allowed_extension(&path) {
            return Err(ThemePackageError::ForbiddenFileType { path: name });
        }
        let too_large = || ThemePackageError::FileTooLarge {
            path: name.clone(),
            max: limits.file,
        };
        if entry.size() > limits.file {
            return Err(too_large());
        }
        // Die angegebene Größe kann lügen (Zip-Bombe): tatsächlich höchstens limit+1 lesen.
        let mut data = Vec::new();
        (&mut entry)
            .take(limits.file + 1)
            .read_to_end(&mut data)
            .map_err(|error| ThemePackageError::NotZip(error.to_string()))?;
        if data.len() as u64 > limits.file {
            return Err(too_large());
        }
        total += data.len() as u64;
        if total > limits.package {
            return Err(ThemePackageError::TooLarge {
                max: limits.package,
            });
        }
        if files.iter().any(|(known, _)| known == &path) {
            return Err(unsafe_path());
        }
        files.push((path, data));
    }
    Ok(files)
}

/// Prüft den Pfad eines Eintrags: nur relative, normale Bestandteile, keine Laufwerksbuchstaben,
/// Backslashes oder Steuerzeichen
fn check_entry_path(name: &str) -> Result<PathBuf, ThemePackageError> {
    let unsafe_path = || ThemePackageError::UnsafePath {
        path: name.to_string(),
    };
    let trimmed = name.strip_suffix('/').unwrap_or(name);
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.contains(['\\', ':'])
        || trimmed.chars().any(char::is_control)
    {
        return Err(unsafe_path());
    }
    let path = Path::new(trimmed);
    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            _ => return Err(unsafe_path()),
        }
    }
    Ok(safe)
}

fn has_allowed_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| ALLOWED_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Lehnt `@import` und jede `url()` ab, die nicht relativ ist oder eine `data:`-URL
fn check_theme_css(css: &str) -> Result<(), ThemePackageError> {
    let css = CSS_COMMENT.replace_all(css, "");
    if CSS_IMPORT.is_match(&css) {
        return Err(ThemePackageError::ForbiddenCss {
            reason: "@import ist nicht erlaubt".to_string(),
        });
    }
    for capture in CSS_URL.captures_iter(&css) {
        let raw = capture[1].trim();
        let value = raw.trim_matches(|c| c == '"' || c == '\'').trim();
        let lower = value.to_ascii_lowercase();
        if lower.starts_with("data:") {
            continue;
        }
        let before_slash = lower.split('/').next().unwrap_or_default();
        if lower.starts_with('/') || before_slash.contains(':') || lower.contains('\\') {
            return Err(ThemePackageError::ForbiddenCss {
                reason: format!("externe oder absolute url() nicht erlaubt: {value}"),
            });
        }
    }
    Ok(())
}

/// Installierte Themes, sortiert nach id. Ordner mit führendem Punkt (temporär) und Ordner
/// ohne lesbares, gültiges Manifest werden übersprungen; fehlt `themes/`, ist die Liste leer.
pub fn list_themes(home: &NexusHome) -> Result<Vec<InstalledTheme>, ThemePackageError> {
    let themes_dir = home.themes_dir();
    let entries = match fs::read_dir(&themes_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut themes = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with('.') || !entry.path().is_dir() {
            continue;
        }
        let Ok(json) = fs::read_to_string(entry.path().join(MANIFEST_FILE)) else {
            continue;
        };
        if let Ok(manifest) = parse_manifest(&json) {
            themes.push(InstalledTheme {
                manifest,
                dir: entry.path(),
            });
        }
    }
    themes.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    Ok(themes)
}

/// Ergebnis eines mitgelieferten Pakets beim Start
#[derive(Debug)]
pub struct BundledResult {
    /// Die `.nexus`-Datei im Ordner der mitgelieferten Themes
    pub file: PathBuf,
    pub outcome: Result<BundledOutcome, ThemePackageError>,
}

/// Was mit einem mitgelieferten Paket geschehen ist
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundledOutcome {
    /// Theme fehlte und wurde installiert
    Installed { id: String, version: String },
    /// Ältere Version wurde ersetzt
    Updated {
        id: String,
        from: String,
        to: String,
    },
    /// Gleiche oder neuere Version (oder unlesbare Installation) bleibt unverändert
    Kept {
        id: String,
        installed: String,
        bundled: String,
    },
}

/// Installiert jedes `*.nexus` aus `dir`, dessen Theme fehlt oder in älterer Version
/// installiert ist. Gleiche oder neuere Versionen und Themes mit anderen IDs bleiben
/// unangetastet; ein fehlerhaftes Paket hält die übrigen nicht auf. Fehlt `dir`, ist das
/// Ergebnis leer.
pub fn install_bundled_themes(home: &NexusHome, dir: &Path) -> Vec<BundledResult> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("nexus"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|file| {
            let outcome = install_bundled(home, &file);
            BundledResult { file, outcome }
        })
        .collect()
}

fn install_bundled(home: &NexusHome, file: &Path) -> Result<BundledOutcome, ThemePackageError> {
    if fs::metadata(file)?.len() > MAX_PACKAGE_BYTES {
        return Err(ThemePackageError::TooLarge {
            max: MAX_PACKAGE_BYTES,
        });
    }
    let bytes = fs::read(file)?;
    let bundled = read_manifest(&bytes)?;
    let installed_dir = home.themes_dir().join(&bundled.id);
    if !installed_dir.exists() {
        let manifest = install_theme(home, &bytes)?;
        return Ok(BundledOutcome::Installed {
            id: manifest.id,
            version: manifest.version,
        });
    }
    let installed = fs::read_to_string(installed_dir.join(MANIFEST_FILE))
        .ok()
        .and_then(|json| parse_manifest(&json).ok())
        .map(|manifest| manifest.version);
    let older = installed
        .as_deref()
        .and_then(|v| semver::Version::parse(v).ok());
    match (older, semver::Version::parse(&bundled.version)) {
        (Some(old), Ok(new)) if old < new => {
            let manifest = install_theme(home, &bytes)?;
            Ok(BundledOutcome::Updated {
                id: manifest.id,
                from: old.to_string(),
                to: manifest.version,
            })
        }
        _ => Ok(BundledOutcome::Kept {
            id: bundled.id,
            installed: installed.unwrap_or_else(|| "?".to_string()),
            bundled: bundled.version,
        }),
    }
}

/// Liest das Manifest eines Pakets mit den Prüfungen der Installation
fn read_manifest(package: &[u8]) -> Result<ThemeManifest, ThemePackageError> {
    let files = read_package(package, DEFAULT_LIMITS)?;
    let json = files
        .iter()
        .find(|(path, _)| path == Path::new(MANIFEST_FILE))
        .map(|(_, data)| data)
        .ok_or_else(|| ThemePackageError::MissingFile {
            name: MANIFEST_FILE.to_string(),
        })?;
    Ok(parse_manifest(&String::from_utf8_lossy(json))?)
}

/// Entfernt ein installiertes Theme. Die id muss eine gültige Theme-ID sein, damit kein Pfad
/// außerhalb von `themes/` getroffen werden kann.
pub fn remove_theme(home: &NexusHome, id: &str) -> Result<(), ThemePackageError> {
    if !THEME_ID_PATTERN.is_match(id) {
        return Err(ThemePackageError::InvalidId { id: id.to_string() });
    }
    let dir = home.themes_dir().join(id);
    if !dir.is_dir() {
        return Err(ThemePackageError::NotInstalled { id: id.to_string() });
    }
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;

    use super::*;

    const CSS: &[u8] = b":root { --bg: url(assets/bg.png); }";

    fn manifest(id: &str, version: &str) -> Vec<u8> {
        format!(
            r#"{{"id":"{id}","name":"Nacht","version":"{version}","author":"Ada","sdkVersion":"^1.0","colorScheme":"dark"}}"#
        )
        .into_bytes()
    }

    /// Baut ein ZIP im Speicher
    fn paket(dateien: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, inhalt) in dateien {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(inhalt).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn gueltig(id: &str, version: &str) -> Vec<u8> {
        let manifest = manifest(id, version);
        paket(&[
            ("manifest.json", &manifest),
            ("theme.css", CSS),
            ("main.js", b"export default {};"),
            ("assets/bg.png", b"png"),
        ])
    }

    fn home() -> (tempfile::TempDir, NexusHome) {
        let dir = tempfile::tempdir().unwrap();
        let home = NexusHome::at(dir.path());
        (dir, home)
    }

    /// Namen unter `themes/` (leer, wenn der Ordner fehlt)
    fn inhalt(home: &NexusHome) -> Vec<String> {
        match fs::read_dir(home.themes_dir()) {
            Ok(entries) => entries
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    fn abgelehnt(home: &NexusHome, paket: &[u8]) -> ThemePackageError {
        let fehler = install_theme(home, paket).unwrap_err();
        assert!(inhalt(home).is_empty(), "Reste: {:?}", inhalt(home));
        fehler
    }

    #[test]
    fn gueltiges_paket_wird_installiert_und_aufgelistet() {
        let (_tmp, home) = home();
        let manifest = install_theme(&home, &gueltig("nacht", "1.0.0")).unwrap();
        assert_eq!(manifest.id, "nacht");
        let dir = home.themes_dir().join("nacht");
        assert!(dir.join("theme.css").is_file());
        assert!(dir.join("main.js").is_file());
        assert!(dir.join("assets/bg.png").is_file());
        assert_eq!(inhalt(&home), vec!["nacht".to_string()]);
        let themes = list_themes(&home).unwrap();
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].manifest.id, "nacht");
        assert_eq!(themes[0].dir, dir);
    }

    #[test]
    fn list_themes_ohne_ordner_ist_leer() {
        let (_tmp, home) = home();
        assert!(list_themes(&home).unwrap().is_empty());
    }

    #[test]
    fn zip_slip_wird_abgelehnt() {
        for name in [
            "../evil.css",
            "/x.css",
            "C:/x.css",
            "a/../../x.css",
            "a\\x.css",
        ] {
            let (_tmp, home) = home();
            let manifest = manifest("nacht", "1.0.0");
            let p = paket(&[
                ("manifest.json", &manifest),
                ("theme.css", CSS),
                (name, b"x"),
            ]);
            assert!(
                matches!(abgelehnt(&home, &p), ThemePackageError::UnsafePath { .. }),
                "{name}"
            );
        }
    }

    #[test]
    fn unerlaubter_dateityp_wird_abgelehnt() {
        let (_tmp, home) = home();
        let manifest = manifest("nacht", "1.0.0");
        let p = paket(&[
            ("manifest.json", &manifest),
            ("theme.css", CSS),
            ("payload.exe", b"MZ"),
        ]);
        assert!(matches!(
            abgelehnt(&home, &p),
            ThemePackageError::ForbiddenFileType { .. }
        ));
    }

    #[test]
    fn zu_grosses_paket_und_zu_grosse_datei_werden_abgelehnt() {
        let (_tmp, home) = home();
        let p = gueltig("nacht", "1.0.0");
        let limits = Limits {
            package: 10,
            file: MAX_FILE_BYTES,
            entries: MAX_ENTRIES,
        };
        assert!(matches!(
            install_with_limits(&home, &p, limits).unwrap_err(),
            ThemePackageError::TooLarge { .. }
        ));
        assert!(inhalt(&home).is_empty());

        let limits = Limits {
            package: MAX_PACKAGE_BYTES,
            file: 20,
            entries: MAX_ENTRIES,
        };
        assert!(matches!(
            install_with_limits(&home, &p, limits).unwrap_err(),
            ThemePackageError::FileTooLarge { .. }
        ));
        assert!(inhalt(&home).is_empty());

        let limits = Limits {
            package: MAX_PACKAGE_BYTES,
            file: MAX_FILE_BYTES,
            entries: 2,
        };
        assert!(matches!(
            install_with_limits(&home, &p, limits).unwrap_err(),
            ThemePackageError::TooManyEntries { .. }
        ));
        assert!(inhalt(&home).is_empty());
    }

    #[test]
    fn verbotenes_css_wird_abgelehnt() {
        for css in [
            "@import url(x.css);",
            "@IMPORT 'x.css';",
            "a { background: url(https://evil/x.png) }",
            "a { background: url('http://evil/x.png') }",
            "a { background: url(\"//evil/x\") }",
            "a { background: url(/abs.png) }",
            "a { background: url(javascript:alert(1)) }",
        ] {
            let (_tmp, home) = home();
            let manifest = manifest("nacht", "1.0.0");
            let p = paket(&[("manifest.json", &manifest), ("theme.css", css.as_bytes())]);
            assert!(
                matches!(abgelehnt(&home, &p), ThemePackageError::ForbiddenCss { .. }),
                "{css}"
            );
        }
    }

    #[test]
    fn relative_und_data_urls_sind_erlaubt() {
        check_theme_css(
            "/* @import url(https://x) */ a { background: url(assets/a.png); } \
             b { background: url( 'data:image/png;base64,AAAA' ); } c { src: url(\"./f.woff2\") }",
        )
        .unwrap();
    }

    #[test]
    fn fehlendes_oder_ungueltiges_manifest_wird_abgelehnt() {
        let (_tmp, home) = home();
        let p = paket(&[("theme.css", CSS)]);
        assert!(matches!(
            abgelehnt(&home, &p),
            ThemePackageError::MissingFile { .. }
        ));

        let p = paket(&[
            ("manifest.json", br#"{"id":"..","version":"x"}"#),
            ("theme.css", CSS),
        ]);
        match abgelehnt(&home, &p) {
            ThemePackageError::Manifest(fehler) => {
                assert!(fehler.for_field("id").is_some());
                assert!(fehler.for_field("version").is_some());
            }
            andere => panic!("unerwartet: {andere:?}"),
        }

        let manifest = manifest("nacht", "1.0.0");
        let p = paket(&[("manifest.json", &manifest)]);
        assert!(matches!(
            abgelehnt(&home, &p),
            ThemePackageError::MissingFile { .. }
        ));
    }

    #[test]
    fn kein_zip_wird_abgelehnt() {
        let (_tmp, home) = home();
        assert!(matches!(
            abgelehnt(&home, b"kein zip"),
            ThemePackageError::NotZip(_)
        ));
    }

    #[test]
    fn neuinstallation_ersetzt_vollstaendig() {
        let (_tmp, home) = home();
        install_theme(&home, &gueltig("nacht", "1.0.0")).unwrap();
        let manifest = manifest("nacht", "2.0.0");
        let neu = paket(&[("manifest.json", &manifest), ("theme.css", CSS)]);
        install_theme(&home, &neu).unwrap();
        let dir = home.themes_dir().join("nacht");
        assert!(!dir.join("main.js").exists());
        assert!(!dir.join("assets").exists());
        assert_eq!(list_themes(&home).unwrap()[0].manifest.version, "2.0.0");
        assert_eq!(inhalt(&home), vec!["nacht".to_string()]);
    }

    #[test]
    fn fehlgeschlagene_neuinstallation_laesst_alte_unveraendert() {
        let (_tmp, home) = home();
        install_theme(&home, &gueltig("nacht", "1.0.0")).unwrap();
        let manifest = manifest("nacht", "2.0.0");
        let kaputt = paket(&[
            ("manifest.json", &manifest),
            ("theme.css", b"@import 'x.css';"),
        ]);
        install_theme(&home, &kaputt).unwrap_err();
        let dir = home.themes_dir().join("nacht");
        assert!(dir.join("main.js").is_file());
        assert_eq!(list_themes(&home).unwrap()[0].manifest.version, "1.0.0");
        assert_eq!(inhalt(&home), vec!["nacht".to_string()]);
    }

    /// Legt einen Theme-Ordner mit den angegebenen Dateien an
    fn ordner(dateien: &[(&str, &[u8])]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, inhalt) in dateien {
            let ziel = dir.path().join(name);
            fs::create_dir_all(ziel.parent().unwrap()).unwrap();
            fs::write(ziel, inhalt).unwrap();
        }
        dir
    }

    fn gueltiger_ordner() -> tempfile::TempDir {
        let manifest = manifest("nacht", "1.2.3");
        ordner(&[
            ("manifest.json", &manifest),
            ("theme.css", CSS),
            ("main.js", b"export default {};"),
            ("assets/img/a.png", b"png"),
        ])
    }

    #[test]
    fn gepackter_ordner_wird_installiert() {
        let dir = gueltiger_ordner();
        let paket = pack_theme(dir.path()).unwrap();
        assert_eq!(paket.manifest.id, "nacht");
        assert_eq!(paket.files, 4);
        let (_tmp, home) = home();
        assert_eq!(install_theme(&home, &paket.bytes).unwrap().id, "nacht");
        let ziel = home.themes_dir().join("nacht");
        for datei in ["manifest.json", "theme.css", "main.js", "assets/img/a.png"] {
            assert!(ziel.join(datei).is_file(), "{datei}");
        }
    }

    #[test]
    fn zweimal_packen_ist_byte_gleich() {
        let dir = gueltiger_ordner();
        assert_eq!(
            pack_theme(dir.path()).unwrap().bytes,
            pack_theme(dir.path()).unwrap().bytes
        );
    }

    /// Ein ZIP nennt je Eintrag das System, auf dem es entstand. Ohne feste Angabe wären Pakete
    /// von Windows und von Linux oder macOS nicht byte-gleich.
    #[test]
    fn paket_nennt_auf_jedem_system_unix() {
        let dir = gueltiger_ordner();
        let paket = pack_theme(dir.path()).unwrap();
        let mut archiv = zip::ZipArchive::new(Cursor::new(paket.bytes)).unwrap();
        for index in 0..archiv.len() {
            let datei = archiv.by_index(index).unwrap();
            assert_eq!(datei.system(), zip::System::Unix, "Eintrag {index}");
        }
    }

    #[test]
    fn packen_meldet_datei_und_grund() {
        let falsch = manifest("Nacht", "1.0.0");
        let manifest = manifest("nacht", "1.0.0");
        /// Dateien des Ordners und erwarteter Teil der Meldung
        type Fall<'a> = (Vec<(&'a str, &'a [u8])>, &'a str);
        let faelle: Vec<Fall> = vec![
            (
                vec![("manifest.json", &falsch), ("theme.css", CSS)],
                "manifest.json ist ungültig: id",
            ),
            (
                vec![
                    ("manifest.json", &manifest),
                    ("theme.css", CSS),
                    ("notes.exe", b"MZ"),
                ],
                "Dateityp nicht erlaubt: notes.exe",
            ),
            (
                vec![
                    ("manifest.json", &manifest),
                    ("theme.css", CSS),
                    (".git/config", b"x"),
                ],
                "nicht erlaubt: .git",
            ),
            (
                vec![
                    ("manifest.json", &manifest),
                    ("theme.css", CSS),
                    ("assets/.DS_Store", b"x"),
                ],
                "nicht erlaubt: assets/.DS_Store",
            ),
            (vec![("manifest.json", &manifest)], "fehlt theme.css"),
            (vec![("theme.css", CSS)], "fehlt manifest.json"),
            (
                vec![
                    ("manifest.json", &manifest),
                    ("theme.css", b"@import 'x.css';"),
                ],
                "theme.css ist nicht erlaubt: @import",
            ),
            (
                vec![
                    ("manifest.json", &manifest),
                    ("theme.css", b"a { background: url(https://x/y.png) }"),
                ],
                "theme.css ist nicht erlaubt: externe",
            ),
        ];
        for (dateien, erwartet) in faelle {
            let dir = ordner(&dateien);
            let meldung = pack_theme(dir.path()).unwrap_err().to_string();
            assert!(meldung.contains(erwartet), "{meldung} / {erwartet}");
        }
    }

    #[test]
    fn packen_prueft_limits() {
        let dir = gueltiger_ordner();
        let mit = |package, file, entries| {
            pack_with_limits(
                dir.path(),
                Limits {
                    package,
                    file,
                    entries,
                },
            )
            .unwrap_err()
        };
        assert!(matches!(
            mit(10, MAX_FILE_BYTES, MAX_ENTRIES),
            ThemePackageError::TooLarge { .. }
        ));
        match mit(MAX_PACKAGE_BYTES, 20, MAX_ENTRIES) {
            ThemePackageError::FileTooLarge { path, .. } => assert_eq!(path, "manifest.json"),
            andere => panic!("unerwartet: {andere:?}"),
        }
        assert!(matches!(
            mit(MAX_PACKAGE_BYTES, MAX_FILE_BYTES, 2),
            ThemePackageError::TooManyEntries { .. }
        ));
    }

    #[test]
    fn fehlender_ordner_wird_gemeldet() {
        let dir = tempfile::tempdir().unwrap();
        let fehler = pack_theme(&dir.path().join("gibts-nicht")).unwrap_err();
        assert!(matches!(fehler, ThemePackageError::DirNotFound { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_wird_abgelehnt() {
        let dir = gueltiger_ordner();
        std::os::unix::fs::symlink("/etc/passwd", dir.path().join("link.css")).unwrap();
        assert!(matches!(
            pack_theme(dir.path()).unwrap_err(),
            ThemePackageError::Symlink { .. }
        ));
    }

    #[test]
    fn remove_theme_loescht_und_prueft_id() {
        let (_tmp, home) = home();
        install_theme(&home, &gueltig("nacht", "1.0.0")).unwrap();
        fs::create_dir_all(home.dir().join("keep")).unwrap();

        assert!(matches!(
            remove_theme(&home, "..").unwrap_err(),
            ThemePackageError::InvalidId { .. }
        ));
        assert!(home.themes_dir().join("nacht").is_dir());
        assert!(home.dir().join("keep").is_dir());

        assert!(matches!(
            remove_theme(&home, "gibts-nicht").unwrap_err(),
            ThemePackageError::NotInstalled { .. }
        ));

        remove_theme(&home, "nacht").unwrap();
        assert!(!home.themes_dir().join("nacht").exists());
        assert!(list_themes(&home).unwrap().is_empty());
    }

    /// Ordner mit mitgelieferten Paketen
    fn mitgeliefert(pakete: &[(&str, Vec<u8>)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, bytes) in pakete {
            fs::write(dir.path().join(name), bytes).unwrap();
        }
        dir
    }

    fn version(home: &NexusHome, id: &str) -> String {
        let json = fs::read_to_string(home.themes_dir().join(id).join("manifest.json")).unwrap();
        parse_manifest(&json).unwrap().version
    }

    #[test]
    fn mitgeliefert_fehlt_wird_installiert() {
        let (_tmp, home) = home();
        let dir = mitgeliefert(&[("nacht.nexus", gueltig("nacht", "1.0.0"))]);
        let results = install_bundled_themes(&home, dir.path());
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].outcome.as_ref().unwrap(),
            &BundledOutcome::Installed {
                id: "nacht".into(),
                version: "1.0.0".into()
            }
        );
        assert_eq!(version(&home, "nacht"), "1.0.0");
    }

    #[test]
    fn mitgeliefert_aelter_installiert_wird_ersetzt() {
        let (_tmp, home) = home();
        install_theme(&home, &gueltig("nacht", "1.0.0")).unwrap();
        let dir = mitgeliefert(&[("nacht.nexus", gueltig("nacht", "1.1.0"))]);
        let results = install_bundled_themes(&home, dir.path());
        assert_eq!(
            results[0].outcome.as_ref().unwrap(),
            &BundledOutcome::Updated {
                id: "nacht".into(),
                from: "1.0.0".into(),
                to: "1.1.0".into()
            }
        );
        assert_eq!(version(&home, "nacht"), "1.1.0");
    }

    #[test]
    fn mitgeliefert_gleich_oder_aelter_bleibt_unveraendert() {
        for (installiert, geliefert) in [("1.0.0", "1.0.0"), ("2.0.0", "1.0.0")] {
            let (_tmp, home) = home();
            install_theme(&home, &gueltig("nacht", installiert)).unwrap();
            let marker = home.themes_dir().join("nacht").join("marker.css");
            fs::write(&marker, "x").unwrap();
            let dir = mitgeliefert(&[("nacht.nexus", gueltig("nacht", geliefert))]);
            let results = install_bundled_themes(&home, dir.path());
            assert!(matches!(
                results[0].outcome.as_ref().unwrap(),
                BundledOutcome::Kept { .. }
            ));
            assert!(marker.is_file(), "{installiert} / {geliefert}");
            assert_eq!(version(&home, "nacht"), installiert);
        }
    }

    #[test]
    fn mitgeliefert_kaputt_blockiert_andere_nicht_und_fremdes_bleibt() {
        let (_tmp, home) = home();
        install_theme(&home, &gueltig("eigen", "1.0.0")).unwrap();
        let dir = mitgeliefert(&[
            ("a-kaputt.nexus", b"kein zip".to_vec()),
            ("nacht.nexus", gueltig("nacht", "1.0.0")),
            ("liesmich.txt", b"x".to_vec()),
        ]);
        let results = install_bundled_themes(&home, dir.path());
        assert_eq!(results.len(), 2);
        assert!(results[0].outcome.is_err());
        assert!(results[1].outcome.is_ok());
        let ids: Vec<_> = list_themes(&home)
            .unwrap()
            .into_iter()
            .map(|t| t.manifest.id)
            .collect();
        assert_eq!(ids, vec!["eigen", "nacht"]);
    }

    #[test]
    fn mitgeliefert_ordner_fehlt_ist_leer() {
        let (tmp, home) = home();
        assert!(install_bundled_themes(&home, &tmp.path().join("fehlt")).is_empty());
    }
}
