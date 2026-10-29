//! Liest und schreibt Kommentare zu Dokumenten in Sidecar-Dateien (`*.comments.json`) neben
//! dem Dokument; portiert aus `packages/core/src/docs/DocCommentStore.ts`.
//!
//! Der Store prüft nicht, ob ein Dokument freigegeben ist: Der Aufrufer (Server) muss
//! `doc_path` gegen die erlaubten Dokumente prüfen. Lesen-Ändern-Schreiben läuft ohne Sperre
//! und ist für einen Einzelnutzer gedacht.

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::clock::Clock;
use crate::docs::doc_comment::{
    DocComment, DocCommentPatch, DocCommentStatus, NewDocComment, OpenDocComment,
};
use crate::error::{Error, HarnessError, Result};
use crate::locking::AtomicWriter;
use crate::random;

/// Endung der Datei neben dem Dokument
const SUFFIX: &str = ".comments.json";
/// Im Projektstamm trägt nur das Glossar Kommentare
const ROOT_SIDECAR: &str = "GLOSSARY.comments.json";
/// Ordner der Superpowers-Dokumente (Spec und Plan je Thema), wie der Scanner sie liest
const SUPERPOWERS_DIRS: [&str; 2] = ["docs/superpowers/specs", "docs/superpowers/plans"];

/// Datum am Anfang eines Dateinamens
fn dated_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^\d{4}-\d{2}-\d{2}-(.+)$").expect("festes Muster"))
}

/// Thema eines Dokuments: Dateiname ohne Datum, ohne Endung `-design` und ohne `.md`
/// (wie [`crate::docs::SuperpowersDocScanner`])
fn topic_of(doc_path: &str) -> String {
    let slug = doc_path
        .rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(doc_path);
    let slug = slug.strip_suffix(".md").unwrap_or(slug);
    let rest = dated_pattern()
        .captures(slug)
        .map(|found| found[1].to_string())
        .unwrap_or_else(|| slug.to_string());
    rest.strip_suffix("-design").unwrap_or(&rest).to_string()
}

/// Liest und schreibt Kommentare zu Dokumenten in Sidecar-Dateien neben dem Dokument.
#[derive(Debug, Clone)]
pub struct DocCommentStore {
    root: PathBuf,
    clock: Clock,
    writer: AtomicWriter,
}

impl DocCommentStore {
    /// Store über dem Repo-Ordner, in dem die Dokumente liegen
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
            || drive_letter(doc_path);
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
        let raw = fs::read_to_string(&file).map_err(Error::unexpected)?;
        // Nicht lesbares JSON und eine unbrauchbare Kommentarliste sind derselbe Fall: die
        // Datei ist beschädigt. In TypeScript fällt nur `JSON.parse` auf, weil es dort keine
        // Typprüfung beim Lesen gibt.
        let broken = || -> Error {
            HarnessError::new(
                "comments-corrupt",
                format!("Die Kommentardatei {sidecar} ist beschädigt."),
            )
            .into()
        };
        let data: Value = serde_json::from_str(&raw).map_err(|_| broken())?;
        match data.get("comments") {
            Some(Value::Array(items)) => {
                serde_json::from_value(Value::Array(items.clone())).map_err(|_| broken())
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
        let index = index_of(&comments, id)?;
        let comment = &mut comments[index];
        if let Some(text) = &patch.text {
            comment.text = text.clone();
        }
        if let Some(status) = patch.status {
            comment.status = status;
        }
        if let Some(resolution) = &patch.resolution {
            comment.resolution = resolution.clone();
        }
        let changed = comment.clone();
        self.save(doc_path, &comments)?;
        Ok(changed)
    }

    /// Löscht einen Kommentar
    pub fn remove(&self, doc_path: &str, id: &str) -> Result<()> {
        let mut comments = self.list(doc_path)?;
        let index = index_of(&comments, id)?;
        comments.remove(index);
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

    /// Dokumentpfade (mit `/`), zu denen eine Sidecar-Datei existiert: die Superpowers-Specs
    /// und -Pläne, ohne `scope` zusätzlich das Glossar im Stamm und die ADRs. `scope` ist
    /// entweder der Pfad eines Dokuments (genau dieses) oder ein Thema – der Dateiname ohne
    /// Datum und ohne die Endung `-design`, der Spec und Plan desselben Themas trifft.
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

    /// Dokumentpfade der Sidecar-Dateien in einem Ordner (relativ zum Projekt); im Stamm nur
    /// das Glossar. Die Namen sind sortiert, damit die Reihenfolge nicht am Dateisystem hängt.
    fn sidecars_in(&self, dir: &str) -> Vec<String> {
        let full = if dir.is_empty() {
            self.root.clone()
        } else {
            self.root.join(dir)
        };
        let Ok(entries) = fs::read_dir(&full) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(SUFFIX))
            .filter(|name| !dir.is_empty() || name == ROOT_SIDECAR)
            .collect();
        names.sort();
        names
            .into_iter()
            .map(|name| {
                let stem = name.strip_suffix(SUFFIX).unwrap_or(&name);
                if dir.is_empty() {
                    format!("{stem}.md")
                } else {
                    format!("{dir}/{stem}.md")
                }
            })
            .collect()
    }

    /// Schreibt die Sidecar-Datei atomar
    fn save(&self, doc_path: &str, comments: &[DocComment]) -> Result<()> {
        let file = self.root.join(Self::sidecar_of(doc_path)?);
        let body = serde_json::json!({ "comments": comments });
        let text = serde_json::to_string_pretty(&body).map_err(Error::unexpected)?;
        self.writer.write(&file, &format!("{text}\n"))
    }
}

/// Beginnt der Pfad mit einem Laufwerksbuchstaben (`C:`)?
fn drive_letter(path: &str) -> bool {
    let mut chars = path.chars();
    matches!((chars.next(), chars.next()), (Some(letter), Some(':')) if letter.is_ascii_alphabetic())
}

/// Position eines Kommentars oder Fehler
fn index_of(comments: &[DocComment], id: &str) -> Result<usize> {
    comments
        .iter()
        .position(|comment| comment.id == id)
        .ok_or_else(|| not_found(id))
}

/// „Kommentar gibt es nicht“ – wortgleich zu TypeScript
fn not_found(id: &str) -> Error {
    HarnessError::new(
        "comment-not-found",
        format!("Kommentar {id} gibt es nicht."),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Die Angaben aus `DocCommentStore.test.ts`
    fn new_comment() -> NewDocComment {
        NewDocComment {
            text: "Bitte präzisieren".into(),
            quote: "Das Ticket".into(),
            heading: Some("Ziel".into()),
            lines: Some([3, 3]),
        }
    }

    const SPEC: &str = "docs/superpowers/specs/2026-10-22-login-design.md";
    const PLAN: &str = "docs/superpowers/plans/2026-10-22-login.md";

    /// Projekt mit den drei Ordnern, in denen Kommentare liegen dürfen
    fn repo() -> (tempfile::TempDir, DocCommentStore) {
        let dir = tempfile::Builder::new()
            .prefix("harness-comments-")
            .tempdir()
            .unwrap();
        for sub in [
            "docs/superpowers/specs",
            "docs/superpowers/plans",
            "docs/adr",
        ] {
            fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        let store = DocCommentStore::new(dir.path());
        (dir, store)
    }

    /// Die Pfade einer Liste offener Kommentare, sortiert
    fn paths(open: Vec<OpenDocComment>) -> Vec<String> {
        let mut paths: Vec<String> = open.into_iter().map(|entry| entry.path).collect();
        paths.sort();
        paths
    }

    #[test]
    fn leitet_den_sidecar_namen_ab() {
        assert_eq!(
            DocCommentStore::sidecar_of(SPEC).unwrap(),
            "docs/superpowers/specs/2026-10-22-login-design.comments.json"
        );
        assert_eq!(
            DocCommentStore::sidecar_of("GLOSSARY.md").unwrap(),
            "GLOSSARY.comments.json"
        );
    }

    #[test]
    fn liefert_ohne_datei_eine_leere_liste_und_legt_beim_hinzufugen_die_datei_an() {
        let (dir, store) = repo();
        assert_eq!(store.list(SPEC).unwrap(), Vec::new());

        let added = store.add(SPEC, &new_comment()).unwrap();

        assert_eq!(added.status, DocCommentStatus::Open);
        assert_eq!(added.resolution, None);
        assert_eq!(added.text, "Bitte präzisieren");
        assert_eq!(added.lines, Some([3, 3]));
        assert!(added.id.starts_with("K-"), "{}", added.id);
        assert_eq!(added.id.len(), 10);
        let file = dir
            .path()
            .join("docs/superpowers/specs/2026-10-22-login-design.comments.json");
        let data: Value = serde_json::from_str(&fs::read_to_string(file).unwrap()).unwrap();
        assert_eq!(data["comments"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn andert_text_status_und_antwort() {
        let (_dir, store) = repo();
        let added = store.add("GLOSSARY.md", &new_comment()).unwrap();

        let changed = store
            .update(
                "GLOSSARY.md",
                &added.id,
                &DocCommentPatch {
                    status: Some(DocCommentStatus::Resolved),
                    resolution: Some(Some("Ergänzt in Abschnitt Ziel".into())),
                    ..DocCommentPatch::default()
                },
            )
            .unwrap();

        assert_eq!(changed.status, DocCommentStatus::Resolved);
        assert_eq!(
            changed.resolution.as_deref(),
            Some("Ergänzt in Abschnitt Ziel")
        );
        assert_eq!(changed.text, "Bitte präzisieren");
    }

    #[test]
    fn entfernt_einen_kommentar_und_meldet_unbekannte_ids() {
        let (_dir, store) = repo();
        let added = store.add("GLOSSARY.md", &new_comment()).unwrap();

        store.remove("GLOSSARY.md", &added.id).unwrap();

        assert_eq!(store.list("GLOSSARY.md").unwrap(), Vec::new());
        let error = store
            .update(
                "GLOSSARY.md",
                "K-00000000",
                &DocCommentPatch {
                    text: Some("x".into()),
                    ..DocCommentPatch::default()
                },
            )
            .unwrap_err();
        assert_eq!(error.code(), Some("comment-not-found"));
    }

    #[test]
    fn listet_ohne_scope_dokumente_glossar_und_adrs_und_lasst_erledigte_weg() {
        let (_dir, store) = repo();
        store.add(SPEC, &new_comment()).unwrap();
        store.add("GLOSSARY.md", &new_comment()).unwrap();
        store.add("docs/adr/0001-eins.md", &new_comment()).unwrap();
        let done = store.add(PLAN, &new_comment()).unwrap();
        store
            .update(
                PLAN,
                &done.id,
                &DocCommentPatch {
                    status: Some(DocCommentStatus::Resolved),
                    ..DocCommentPatch::default()
                },
            )
            .unwrap();

        assert_eq!(
            paths(store.list_open(None).unwrap()),
            ["GLOSSARY.md", "docs/adr/0001-eins.md", SPEC]
        );
    }

    #[test]
    fn nimmt_als_scope_ein_thema_oder_genau_einen_dokumentpfad() {
        let (_dir, store) = repo();
        store.add(SPEC, &new_comment()).unwrap();
        store.add(PLAN, &new_comment()).unwrap();
        store
            .add(
                "docs/superpowers/specs/2026-10-20-export-design.md",
                &new_comment(),
            )
            .unwrap();
        store.add("GLOSSARY.md", &new_comment()).unwrap();

        assert_eq!(paths(store.list_open(Some("login")).unwrap()), [PLAN, SPEC]);
        assert_eq!(paths(store.list_open(Some(SPEC)).unwrap()), [SPEC]);
        assert!(store.list_open(Some("gibt-es-nicht")).unwrap().is_empty());
    }

    #[test]
    fn erledigt_einen_kommentar_uber_die_id_ohne_das_dokument_zu_kennen() {
        let (_dir, store) = repo();
        let added = store.add(PLAN, &new_comment()).unwrap();

        let resolved = store.resolve(&added.id, "Schritt 3 ergänzt").unwrap();

        assert_eq!(resolved.path, PLAN);
        assert_eq!(resolved.comment.status, DocCommentStatus::Resolved);
        assert_eq!(
            resolved.comment.resolution.as_deref(),
            Some("Schritt 3 ergänzt")
        );
    }

    #[test]
    fn meldet_eine_beschadigte_sidecar_datei_verstandlich() {
        let (dir, store) = repo();
        fs::write(dir.path().join("GLOSSARY.comments.json"), "{kaputt").unwrap();

        let error = store.list("GLOSSARY.md").unwrap_err();

        assert_eq!(error.code(), Some("comments-corrupt"));
    }

    #[test]
    fn lehnt_pfade_ohne_md_oder_ausserhalb_des_projekts_ab_und_legt_nichts_an() {
        let (dir, store) = repo();
        assert_eq!(
            store.list("README.txt").unwrap_err().code(),
            Some("doc-not-found")
        );
        for bad in [
            "../x.md",
            "/abs.md",
            "docs/superpowers/specs/2026-10-22-login-design.txt",
        ] {
            assert_eq!(
                store.add(bad, &new_comment()).unwrap_err().code(),
                Some("doc-not-found"),
                "{bad}"
            );
        }
        assert!(!dir
            .path()
            .join("docs/superpowers/specs/2026-10-22-login-design.txt")
            .exists());
        assert!(!dir.path().join("x.comments.json").exists());
    }

    #[test]
    fn lehnt_pfade_mit_backslash_ab_und_schreibt_nie_ausserhalb_des_projekts() {
        let (dir, store) = repo();
        for bad in [r"docs\..\..\x.md", r"..\x.md", r"\\server\share\x.md"] {
            assert_eq!(
                store.add(bad, &new_comment()).unwrap_err().code(),
                Some("doc-not-found"),
                "{bad}"
            );
        }
        assert!(!dir
            .path()
            .parent()
            .unwrap()
            .join("x.comments.json")
            .exists());
        assert!(!dir.path().join("x.comments.json").exists());
    }

    #[test]
    fn ignoriert_fremde_comments_dateien_im_projektstamm() {
        let (_dir, store) = repo();
        store.add("GLOSSARY.md", &new_comment()).unwrap();
        store.add("README.md", &new_comment()).unwrap();

        assert_eq!(paths(store.list_open(None).unwrap()), ["GLOSSARY.md"]);
    }

    #[test]
    fn meldet_beim_erledigen_einer_unbekannten_id_comment_not_found() {
        let (_dir, store) = repo();
        assert_eq!(
            store.resolve("K-00000000", "x").unwrap_err().code(),
            Some("comment-not-found")
        );
    }

    #[test]
    fn behandelt_json_ohne_comments_array_als_leer() {
        let (dir, store) = repo();
        fs::write(dir.path().join("GLOSSARY.comments.json"), "{}").unwrap();
        assert_eq!(store.list("GLOSSARY.md").unwrap(), Vec::new());
    }

    #[test]
    fn ignoriert_fehlende_werte_im_patch() {
        let (_dir, store) = repo();
        let added = store.add("GLOSSARY.md", &new_comment()).unwrap();

        let changed = store
            .update(
                "GLOSSARY.md",
                &added.id,
                &DocCommentPatch {
                    text: None,
                    status: Some(DocCommentStatus::Resolved),
                    resolution: None,
                },
            )
            .unwrap();

        assert_eq!(changed.text, "Bitte präzisieren");
        assert_eq!(changed.status, DocCommentStatus::Resolved);
    }

    #[test]
    fn liest_das_thema_aus_dem_dateinamen() {
        assert_eq!(topic_of(SPEC), "login");
        assert_eq!(topic_of(PLAN), "login");
        assert_eq!(topic_of("GLOSSARY.md"), "GLOSSARY");
    }
}
