//! Liest und durchsucht die freigegebenen Dokumente im Repo des Boards und ersetzt einzelne
//! Abschnitte; portiert aus `packages/server/src/docs/DocsService.ts`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use nexus_core::docs::{
    is_doc_file_name, DocHash, MarkdownSection, MarkdownSections, SuperpowersDocKind,
    SuperpowersDocScanner,
};
use nexus_core::locking::AtomicWriter;
use nexus_core::{Error, HarnessError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as AsyncMutex;

use crate::docs::docs_root_resolver::DocsSource;

/// Gruppe eines Dokuments in der Liste der Docs-Seite
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocGroup {
    Spec,
    Plan,
    Glossary,
    Adr,
}

/// Ein Dokument in der Liste der Docs-Seite
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocFileDto {
    pub path: String,
    pub title: String,
    pub group: DocGroup,
    /// Sorte des Superpowers-Dokuments; bei Glossar und ADRs `None`
    pub kind: Option<SuperpowersDocKind>,
    pub topic: Option<String>,
    pub date: Option<String>,
    pub mtime_ms: f64,
    /// bleibt erhalten, damit Web und Liste ohne Umbau weiterlaufen; neue Quellen liefern
    /// immer `true`
    pub exists: bool,
    /// Zur Datei sind Ticket-Entwürfe gespeichert (gesetzt von der Docs-Route)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_drafts: Option<bool>,
}

/// Die Dokumente samt Quelle
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocsListDto {
    pub source: DocsSource,
    pub docs: Vec<DocFileDto>,
}

/// Inhalt eines Dokuments mit Fingerabdruck und Abschnitten
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocFileContentDto {
    pub path: String,
    pub content: String,
    pub hash: String,
    pub sections: Vec<MarkdownSection>,
}

/// Der geprüfte Körper der Schreibroute
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteSectionInput {
    pub path: String,
    /// 1-basiert und einschließlich, wie [`MarkdownSection`] und `DocComment.lines`
    pub start_line: i64,
    pub end_line: i64,
    pub content: String,
    /// Fingerabdruck aus dem letzten Lesen; weicht er ab, hat jemand anders geschrieben
    pub expected_hash: String,
}

/// Ergebnis des Schreibens
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WriteSectionResult {
    pub path: String,
    pub hash: String,
    pub sections: Vec<MarkdownSection>,
}

/// Ein Treffer der Suche
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHitDto {
    pub path: String,
    pub line: usize,
    pub excerpt: String,
}

/// Höchstzahl der Treffer einer Suche
const MAX_HITS: usize = 100;
/// Höchstlänge eines Auszugs in der Trefferliste
const MAX_EXCERPT: usize = 160;

/// Nur diese Dateien sind lesbar (Superpowers-Specs und -Pläne, Glossar, ADRs); alles andere
/// ist gesperrt. Dasselbe Muster, nach dem der Scanner auflistet, damit jedes gelistete
/// Dokument auch lesbar ist.
fn allowed() -> &'static [Regex; 3] {
    static PATTERNS: OnceLock<[Regex; 3]> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let name = r"[A-Za-z0-9._-]+\.md";
        [
            Regex::new(&format!(r"^docs/superpowers/(specs|plans)/{name}$"))
                .expect("festes Muster"),
            Regex::new(r"^GLOSSARY\.md$").expect("festes Muster"),
            Regex::new(&format!(r"^docs/adr/{name}$")).expect("festes Muster"),
        ]
    })
}

/// Geschrieben werden nur Superpowers-Specs und -Pläne; Glossar und ADRs sind lesbar, aber
/// nicht bearbeitbar.
fn writable(path: &str) -> bool {
    allowed()[0].is_match(path)
}

/// Liest, durchsucht und beschreibt die freigegebenen Dokumente im Repo des Boards.
#[derive(Debug, Default)]
pub struct DocsService {
    scanner: SuperpowersDocScanner,
    sections: MarkdownSections,
    writer: AtomicWriter,
    /// Laufende Schreibvorgänge je Datei: Prüfung des Fingerabdrucks und Schreiben einer Datei
    /// laufen nacheinander, sonst bestünden zwei gleichzeitige Änderungen gegen denselben
    /// Stand beide die Prüfung und die zweite überschriebe die erste
    writes: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}

impl DocsService {
    /// Neuer Dienst
    pub fn new() -> Self {
        Self::default()
    }

    /// Liste der Dokumente samt Quelle; ohne Quelle bleibt die Liste leer und
    /// `source.reason` nennt den Grund
    pub fn list(&self, source: &DocsSource) -> DocsListDto {
        let Some(root) = source.root.as_deref() else {
            return DocsListDto {
                source: source.clone(),
                docs: Vec::new(),
            };
        };
        let root = Path::new(root);
        let mut docs: Vec<DocFileDto> = Vec::new();
        for doc in self.scanner.scan(root) {
            docs.push(DocFileDto {
                path: doc.path,
                title: doc.title,
                group: match doc.kind {
                    SuperpowersDocKind::Spec => DocGroup::Spec,
                    SuperpowersDocKind::Plan => DocGroup::Plan,
                },
                kind: Some(doc.kind),
                topic: Some(doc.topic),
                date: doc.date,
                mtime_ms: doc.mtime_ms,
                exists: true,
                has_drafts: None,
            });
        }
        if root.join("GLOSSARY.md").exists() {
            docs.push(plain(root, "GLOSSARY.md", "Glossar", DocGroup::Glossary));
        }
        for name in files(&root.join("docs").join("adr")) {
            let title = name.strip_suffix(".md").unwrap_or(&name).to_string();
            docs.push(plain(
                root,
                &format!("docs/adr/{name}"),
                &title,
                DocGroup::Adr,
            ));
        }
        DocsListDto {
            source: source.clone(),
            docs,
        }
    }

    /// Inhalt einer freigegebenen Datei mit Fingerabdruck und Abschnitten; alles andere (auch
    /// Ausbruchsversuche) ergibt „nicht gefunden“
    pub fn read(&self, source: &DocsSource, path: Option<&str>) -> Result<DocFileContentDto> {
        let full = self.full_path(source, path)?;
        let content = fs::read_to_string(&full).map_err(Error::unexpected)?;
        Ok(DocFileContentDto {
            path: path.unwrap_or_default().to_string(),
            hash: DocHash::of(&content),
            sections: self.sections.parse(&content),
            content,
        })
    }

    /// Voller Pfad einer freigegebenen, vorhandenen Datei (zum Lesen und zum Öffnen in
    /// Explorer oder VS Code)
    pub fn full_path(&self, source: &DocsSource, path: Option<&str>) -> Result<PathBuf> {
        let (Some(root), Some(path)) = (source.root.as_deref(), path) else {
            return Err(not_found());
        };
        if !is_allowed(path) {
            return Err(not_found());
        }
        let full = Path::new(root).join(path);
        if !full.is_file() {
            return Err(HarnessError::new(
                "doc-not-found",
                format!("Die Datei {path} existiert noch nicht."),
            )
            .into());
        }
        Ok(full)
    }

    /// Ersetzt einen Zeilenbereich einer Spec oder eines Plans, wenn der Fingerabdruck noch
    /// zum gelesenen Stand passt; je Datei läuft das nur einmal gleichzeitig
    pub async fn write_section(
        &self,
        source: &DocsSource,
        input: &WriteSectionInput,
    ) -> Result<WriteSectionResult> {
        let key = format!(
            "{}|{}",
            source.root.as_deref().unwrap_or_default(),
            input.path
        )
        .to_lowercase();
        let lock = self
            .writes
            .lock()
            .expect("Sperre der Schreibvorgänge")
            .entry(key)
            .or_default()
            .clone();
        let _held = lock.lock().await;
        self.replace_section(source, input)
    }

    /// Prüft den Fingerabdruck gegen den Stand auf der Platte und schreibt
    fn replace_section(
        &self,
        source: &DocsSource,
        input: &WriteSectionInput,
    ) -> Result<WriteSectionResult> {
        // Erst lesen: ein nicht freigegebener oder fehlender Pfad bleibt so „nicht gefunden“
        // und verrät nichts über das Dateisystem
        let current = self.read(source, Some(&input.path))?;
        if !writable(&current.path) {
            return Err(HarnessError::new(
                "doc-not-writable",
                "Glossar und ADRs lassen sich hier nicht bearbeiten.",
            )
            .into());
        }
        if input.expected_hash.is_empty() || input.expected_hash != current.hash {
            // Der aktuelle Fingerabdruck geht mit, damit der Client den Unterschied anzeigen
            // kann, ohne erneut zu lesen
            return Err(HarnessError::new(
                "doc-changed",
                "Das Dokument wurde zwischenzeitlich geändert. Lade es neu und wiederhole die Änderung.",
            )
            .with_details(serde_json::json!({ "hash": current.hash }))
            .into());
        }
        // replace_lines prüft den Bereich und behält Zeilenende-Stil und abschließenden
        // Umbruch bei
        let next = self.sections.replace_lines(
            &current.content,
            input.start_line,
            input.end_line,
            &input.content,
        )?;
        let root = source.root.as_deref().unwrap_or_default();
        self.writer
            .write(&Path::new(root).join(&current.path), &next)?;
        Ok(WriteSectionResult {
            path: current.path,
            hash: DocHash::of(&next),
            sections: self.sections.parse(&next),
        })
    }

    /// Meldet „nicht gefunden“, wenn das Dokument nicht freigegeben ist oder fehlt
    /// (für Kommentare)
    pub fn assert_document(&self, source: &DocsSource, path: Option<&str>) -> Result<String> {
        self.read(source, path).map(|doc| doc.path)
    }

    /// Teilstringsuche ohne Beachtung der Schreibweise über alle freigegebenen Dateien
    pub fn search(&self, source: &DocsSource, query: &str) -> Result<Vec<SearchHitDto>> {
        let needle = query.trim().to_lowercase();
        if needle.chars().count() < 2 {
            return Err(HarnessError::new(
                "query-short",
                "Bitte mindestens zwei Zeichen eingeben.",
            )
            .into());
        }
        let mut hits: Vec<SearchHitDto> = Vec::new();
        let Some(root) = source.root.as_deref() else {
            return Ok(hits);
        };
        for doc in self.list(source).docs {
            let text =
                fs::read_to_string(Path::new(root).join(&doc.path)).map_err(Error::unexpected)?;
            for (index, line) in nexus_core::docs::markdown_sections::split_lines(&text)
                .iter()
                .enumerate()
            {
                if hits.len() < MAX_HITS && line.to_lowercase().contains(&needle) {
                    hits.push(SearchHitDto {
                        path: doc.path.clone(),
                        line: index + 1,
                        excerpt: line.trim().chars().take(MAX_EXCERPT).collect(),
                    });
                }
            }
        }
        Ok(hits)
    }
}

/// Eintrag für eine Datei ohne Thema und Datum (Glossar, ADR)
fn plain(root: &Path, path: &str, title: &str, group: DocGroup) -> DocFileDto {
    let mtime_ms = fs::metadata(root.join(path))
        .ok()
        .and_then(|data| data.modified().ok())
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|since| since.as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    DocFileDto {
        path: path.to_string(),
        title: title.to_string(),
        group,
        kind: None,
        topic: None,
        date: None,
        mtime_ms,
        exists: true,
        has_drafts: None,
    }
}

/// Erlaubt nur genau passende Pfade ohne Rückwärtsstriche und ohne „..“
fn is_allowed(path: &str) -> bool {
    if path.contains('\\') || path.contains("..") || path.starts_with('/') {
        return false;
    }
    allowed().iter().any(|pattern| pattern.is_match(path))
}

/// Markdown-Dateien eines Ordners, deren Name auch lesbar ist
fn files(dir: &Path) -> Vec<String> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = read
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
        })
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| is_doc_file_name(name))
        .collect();
    names.sort();
    names
}

/// „Diese Datei gibt es nicht oder sie ist nicht freigegeben.“ – wortgleich zu TypeScript
fn not_found() -> Error {
    HarnessError::new(
        "doc-not-found",
        "Diese Datei gibt es nicht oder sie ist nicht freigegeben.",
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gibt_nur_freigegebene_pfade_frei() {
        for path in [
            "docs/superpowers/specs/2026-10-22-x-design.md",
            "docs/superpowers/plans/2026-10-22-x.md",
            "GLOSSARY.md",
            "docs/adr/0001-eins.md",
        ] {
            assert!(is_allowed(path), "{path}");
        }
        for path in [
            "README.md",
            "geheim.md",
            "specs/001-login/spec.md",
            "../geheim.md",
            "../../etc/passwd",
            "/etc/passwd",
            "C:/Windows/win.ini",
            "docs/superpowers/specs/../../../geheim.md",
            r"docs\superpowers\specs\..\..\..\geheim.md",
            "%2e%2e/geheim.md",
            ".git/config",
            "docs/adr/../../geheim.md",
            "docs/adr/",
            "docs/superpowers/specs/",
            "docs/superpowers/specs/2026-10-22 mit Leerzeichen.md",
        ] {
            assert!(!is_allowed(path), "{path}");
        }
    }

    #[test]
    fn lasst_nur_specs_und_plane_schreiben() {
        assert!(writable("docs/superpowers/specs/2026-10-22-x-design.md"));
        assert!(writable("docs/superpowers/plans/2026-10-22-x.md"));
        assert!(!writable("GLOSSARY.md"));
        assert!(!writable("docs/adr/0001-eins.md"));
    }
}
