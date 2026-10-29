//! Liest und schreibt Kommentare zu Dokumenten in Sidecar-Dateien (`….comments.json`) neben dem
//! Dokument. Portiert aus `packages/core/src/docs/DocCommentStore.ts`.
//!
//! Der Store prüft nicht, ob ein Dokument freigegeben ist: Der Aufrufer (Server) muss `doc_path`
//! gegen die erlaubten Dokumente prüfen. Lesen-Ändern-Schreiben läuft ohne Sperre und ist für
//! einen Einzelnutzer gedacht.

use std::path::PathBuf;

use crate::clock::Clock;
use crate::docs::doc_comment::{
    DocComment, DocCommentPatch, DocCommentStatus, NewDocComment, OpenDocComment,
};
use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;
use crate::random;

/// Endung der Sidecar-Dateien
const SUFFIX: &str = ".comments.json";

/// Die einzige Sidecar-Datei, die im Projektstamm zählt
const ROOT_SIDECAR: &str = "GLOSSARY.comments.json";

/// Ordner der Superpowers-Dokumente (Spec und Plan je Thema), wie `SuperpowersDocScanner` sie liest
const SUPERPOWERS_DIRS: [&str; 2] = ["docs/superpowers/specs", "docs/superpowers/plans"];

/// Thema eines Dokuments: Dateiname ohne Datum, ohne Endung `-design` und ohne `.md`
/// (wie `SuperpowersDocScanner`)
fn topic_of(doc_path: &str) -> String {
    let slug = match doc_path.rfind('/') {
        Some(index) => &doc_path[index + 1..],
        None => doc_path,
    };
    let slug = slug.strip_suffix(".md").unwrap_or(slug);
    let rest = strip_date(slug).unwrap_or(slug);
    rest.strip_suffix("-design").unwrap_or(rest).to_string()
}

/// Der Teil hinter einem führenden `JJJJ-MM-TT-`, falls der Name so anfängt
fn strip_date(slug: &str) -> Option<&str> {
    let bytes = slug.as_bytes();
    if bytes.len() < 11 {
        return None;
    }
    let digits = [0, 1, 2, 3, 5, 6, 8, 9];
    let striche = [4, 7, 10];
    if digits.iter().all(|i| bytes[*i].is_ascii_digit())
        && striche.iter().all(|i| bytes[*i] == b'-')
    {
        Some(&slug[11..])
    } else {
        None
    }
}

/// Kommentare zu Dokumenten in Sidecar-Dateien neben dem Dokument.
#[derive(Debug, Clone)]
pub struct DocCommentStore {
    root: PathBuf,
    clock: Clock,
    writer: AtomicWriter,
}

impl DocCommentStore {
    /// Store für die Dokumente unterhalb dieses Projektordners
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_clock(root, Clock::system())
    }

    /// Wie [`DocCommentStore::new`], mit eigener Uhr (Tests)
    pub fn with_clock(root: impl Into<PathBuf>, clock: Clock) -> Self {
        Self {
            root: root.into(),
            clock,
            writer: AtomicWriter::new(),
        }
    }

    /// Sidecar-Pfad zu einem Dokumentpfad (`spec.md` → `spec.comments.json`); andere Pfade
    /// werden abgelehnt
    pub fn sidecar_of(doc_path: &str) -> Result<String> {
        let escapes = doc_path.starts_with('/')
            || doc_path.contains('\\')
            || doc_path.split('/').any(|part| part == "..")
            || starts_with_drive(doc_path);
        match doc_path.strip_suffix(".md") {
            Some(stem) if !escapes => Ok(format!("{stem}{SUFFIX}")),
            _ => Err(HarnessError::new(
                "doc-not-found",
                format!("Das Dokument {doc_path} gibt es nicht (nur .md-Dateien im Projekt sind erlaubt)."),
            )
            .into()),
        }
    }

    /// Alle Kommentare eines Dokuments (leer, wenn es noch keine Datei gibt)
    pub fn list(&self, doc_path: &str) -> Result<Vec<DocComment>> {
        let sidecar = Self::sidecar_of(doc_path)?;
        let file = self.root.join(&sidecar);
        if !file.exists() {
            return Ok(Vec::new());
        }
        let raw = std::fs::read_to_string(&file).map_err(Error::unexpected)?;
        let data: serde_json::Value = serde_json::from_str(&raw).map_err(|_| corrupt(&sidecar))?;
        match data.get("comments") {
            Some(serde_json::Value::Array(entries)) => {
                serde_json::from_value(serde_json::Value::Array(entries.clone()))
                    .map_err(|_| corrupt(&sidecar))
            }
            _ => Ok(Vec::new()),
        }
    }

    /// Fügt einen offenen Kommentar hinzu
    pub fn add(&self, doc_path: &str, input: &NewDocComment) -> Result<DocComment> {
        let mut comments = self.list(doc_path)?;
        let comment = DocComment {
            id: format!("K-{}", random::hex(4)),
            status: DocCommentStatus::Open,
            created_at: self.clock.iso(),
            resolution: None,
            text: input.text.clone(),
            quote: input.quote.clone(),
            heading: input.heading.clone(),
            lines: input.lines,
        };
        comments.push(comment.clone());
        self.save(doc_path, &comments)?;
        Ok(comment)
    }

    /// Ändert Text, Status oder Antwort eines Kommentars
    pub fn update(&self, doc_path: &str, id: &str, patch: &DocCommentPatch) -> Result<DocComment> {
        let mut comments = self.list(doc_path)?;
        let index = Self::index_of(&comments, id)?;
        if let Some(text) = &patch.text {
            comments[index].text = text.clone();
        }
        if let Some(status) = patch.status {
            comments[index].status = status;
        }
        if let Some(resolution) = &patch.resolution {
            comments[index].resolution = resolution.clone();
        }
        self.save(doc_path, &comments)?;
        Ok(comments[index].clone())
    }

    /// Löscht einen Kommentar
    pub fn remove(&self, doc_path: &str, id: &str) -> Result<()> {
        let mut comments = self.list(doc_path)?;
        comments.remove(Self::index_of(&comments, id)?);
        self.save(doc_path, &comments)
    }

    /// Offene Kommentare aller Dokumente; `scope` begrenzt auf ein Dokument (Pfad) oder ein
    /// Thema (Spec und Plan)
    pub fn list_open(&self, scope: Option<&str>) -> Result<Vec<OpenDocComment>> {
        let mut result = Vec::new();
        for doc_path in self.documents_with_comments(scope) {
            for comment in self.list(&doc_path)? {
                if comment.status == DocCommentStatus::Open {
                    result.push(OpenDocComment {
                        path: doc_path.clone(),
                        comment,
                    });
                }
            }
        }
        Ok(result)
    }

    /// Erledigt einen Kommentar über seine ID in beliebigem Dokument
    pub fn resolve(&self, id: &str, resolution: &str) -> Result<OpenDocComment> {
        for doc_path in self.documents_with_comments(None) {
            if self.list(&doc_path)?.iter().any(|c| c.id == id) {
                let comment = self.update(
                    &doc_path,
                    id,
                    &DocCommentPatch {
                        status: Some(DocCommentStatus::Resolved),
                        resolution: Some(Some(resolution.to_string())),
                        ..DocCommentPatch::default()
                    },
                )?;
                return Ok(OpenDocComment {
                    path: doc_path,
                    comment,
                });
            }
        }
        Err(not_found(id))
    }

    /// Dokumentpfade (mit „/“), zu denen eine Sidecar-Datei existiert: die Superpowers-Specs und
    /// -Pläne, ohne `scope` zusätzlich das Glossar im Stamm und die ADRs. `scope` ist entweder der
    /// Pfad eines Dokuments (genau dieses) oder ein Thema – der Dateiname ohne Datum und ohne die
    /// Endung `-design`, der Spec und Plan desselben Themas trifft.
    fn documents_with_comments(&self, scope: Option<&str>) -> Vec<String> {
        let superpowers: Vec<String> = SUPERPOWERS_DIRS
            .iter()
            .flat_map(|dir| self.sidecars_in(dir))
            .collect();
        let Some(scope) = scope else {
            let mut all = superpowers;
            all.extend(self.sidecars_in(""));
            all.extend(self.sidecars_in("docs/adr"));
            return all;
        };
        let normalized = scope.replace('\\', "/");
        superpowers
            .into_iter()
            .filter(|doc_path| *doc_path == normalized || topic_of(doc_path) == normalized)
            .collect()
    }

    /// Dokumentpfade der Sidecar-Dateien in einem Ordner (relativ zum Projekt); im Stamm nur das
    /// Glossar
    fn sidecars_in(&self, dir: &str) -> Vec<String> {
        let full = if dir.is_empty() {
            self.root.clone()
        } else {
            self.root.join(dir)
        };
        let Ok(entries) = std::fs::read_dir(&full) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.ends_with(SUFFIX))
            .filter(|name| !dir.is_empty() || name == ROOT_SIDECAR)
            .collect();
        names.sort();
        names
            .into_iter()
            .map(|name| {
                let doc = format!("{}.md", &name[..name.len() - SUFFIX.len()]);
                if dir.is_empty() {
                    doc
                } else {
                    format!("{dir}/{doc}")
                }
            })
            .collect()
    }

    /// Position eines Kommentars oder Fehler
    fn index_of(comments: &[DocComment], id: &str) -> Result<usize> {
        comments
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| not_found(id))
    }

    /// Schreibt die Sidecar-Datei atomar
    fn save(&self, doc_path: &str, comments: &[DocComment]) -> Result<()> {
        let file = self.root.join(Self::sidecar_of(doc_path)?);
        let json = serde_json::to_string_pretty(&serde_json::json!({ "comments": comments }))
            .map_err(Error::unexpected)?;
        self.writer.write(&file, &(json + "\n"))
    }
}

/// Beginnt der Pfad mit einem Laufwerksbuchstaben wie `C:`?
fn starts_with_drive(doc_path: &str) -> bool {
    let bytes = doc_path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Fehler einer unlesbaren Sidecar-Datei
fn corrupt(sidecar: &str) -> Error {
    HarnessError::new(
        "comments-corrupt",
        format!("Die Kommentardatei {sidecar} ist beschädigt."),
    )
    .into()
}

/// Fehler einer unbekannten Kommentar-ID
fn not_found(id: &str) -> Error {
    HarnessError::new(
        "comment-not-found",
        format!("Kommentar {id} gibt es nicht."),
    )
    .into()
}

#[cfg(test)]
mod tests;
