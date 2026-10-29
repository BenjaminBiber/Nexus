//! Findet und paart die Dokumente unter `<ordner>/docs/superpowers/{specs,plans}`; portiert
//! aus `packages/core/src/docs/SuperpowersDocScanner.ts`.
//!
//! `<ordner>` ist der Repo-Pfad des Boards. Die Dateien sind nicht versioniert, deshalb liest
//! der Scanner das Dateisystem und nicht Git.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::docs::markdown_sections::split_lines;

/// Welche der beiden Sorten von Dokumenten vorliegt: die Spec (`…-design.md`) oder der Plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SuperpowersDocKind {
    Spec,
    Plan,
}

impl SuperpowersDocKind {
    /// Name der Sorte, wie ihn die API ausgibt
    pub fn as_str(self) -> &'static str {
        match self {
            SuperpowersDocKind::Spec => "spec",
            SuperpowersDocKind::Plan => "plan",
        }
    }
}

/// Ein gefundenes Markdown-Dokument samt der Angaben, die Anzeige und Cache-Prüfung brauchen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuperpowersDoc {
    pub kind: SuperpowersDocKind,
    /// Pfad relativ zum Repo-Wurzelordner, immer mit `/` –
    /// z. B. `docs/superpowers/specs/2026-10-22-x-design.md`
    pub path: String,
    /// Dateiname ohne `.md`
    pub slug: String,
    /// Thema: Dateiname ohne Datum und ohne Endung `-design`
    pub topic: String,
    /// JJJJ-MM-TT aus dem Dateinamen oder `None`
    pub date: Option<String>,
    /// erste H1-Überschrift, sonst der Dateiname ohne Endung
    pub title: String,
    pub mtime_ms: f64,
    pub size: u64,
}

/// Spec und Plan eines Themas als ein Eintrag; fehlt eine der beiden Seiten, bleibt sie `None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuperpowersTopic {
    pub topic: String,
    pub date: Option<String>,
    pub spec: Option<SuperpowersDoc>,
    pub plan: Option<SuperpowersDoc>,
}

/// Die beiden Ordner und ihre Sorte, in der Reihenfolge des Scanners
const DIRS: [(SuperpowersDocKind, &str); 2] = [
    (SuperpowersDocKind::Spec, "specs"),
    (SuperpowersDocKind::Plan, "plans"),
];

/// Erlaubte Dateinamen der Dokumente: nur diese lassen sich über die Docs-Seite auch lesen
/// und bearbeiten (`DOC_FILE_NAME` in TypeScript)
pub const DOC_FILE_NAME: &str = r"^[A-Za-z0-9._-]+\.md$";

/// Passt der Dateiname zu [`DOC_FILE_NAME`]?
pub fn is_doc_file_name(name: &str) -> bool {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| Regex::new(DOC_FILE_NAME).expect("festes Muster"))
        .is_match(name)
}

/// Datum und Thema im Dateinamen
fn slug_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^(\d{4}-\d{2}-\d{2})-(.+)$").expect("festes Muster"))
}

/// Erste H1-Zeile
fn h1_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^#\s+(.+)$").expect("festes Muster"))
}

/// Findet und paart die Dokumente der beiden Superpowers-Ordner.
#[derive(Debug, Clone, Copy, Default)]
pub struct SuperpowersDocScanner;

impl SuperpowersDocScanner {
    /// Neuer Scanner (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Alle Dokumente beider Ordner; fehlende Ordner ergeben eine leere Liste. Sortiert nach
    /// Datum absteigend (ohne Datum zuletzt), dann Thema
    pub fn scan(&self, root: &Path) -> Vec<SuperpowersDoc> {
        let mut docs: Vec<SuperpowersDoc> = Vec::new();
        for (kind, dir) in DIRS {
            for entry in entries(root, dir) {
                let slug = entry
                    .name
                    .strip_suffix(".md")
                    .unwrap_or(&entry.name)
                    .to_string();
                let (date, topic) = parse_slug(&slug);
                let (mtime_ms, size) = stat(&entry.full);
                docs.push(SuperpowersDoc {
                    kind,
                    path: format!("docs/superpowers/{dir}/{}", entry.name),
                    title: first_heading(&entry.full).unwrap_or_else(|| slug.clone()),
                    slug,
                    date,
                    topic,
                    mtime_ms,
                    size,
                });
            }
        }
        docs.sort_by(compare_docs);
        docs
    }

    /// Die Dokumente nach Thema gruppiert, in derselben Reihenfolge wie [`Self::scan`]
    pub fn topics(&self, root: &Path) -> Vec<SuperpowersTopic> {
        let mut by_topic: BTreeMap<String, SuperpowersTopic> = BTreeMap::new();
        for doc in self.scan(root) {
            let entry = by_topic
                .entry(doc.topic.clone())
                .or_insert_with(|| SuperpowersTopic {
                    topic: doc.topic.clone(),
                    date: None,
                    spec: None,
                    plan: None,
                });
            match doc.kind {
                SuperpowersDocKind::Spec => entry.spec = Some(doc),
                SuperpowersDocKind::Plan => entry.plan = Some(doc),
            }
            entry.date = entry
                .spec
                .as_ref()
                .and_then(|spec| spec.date.clone())
                .or_else(|| entry.plan.as_ref().and_then(|plan| plan.date.clone()));
        }
        // Dieselbe Reihenfolge wie in TypeScript: neueres Datum zuerst, dann Thema
        let mut topics: Vec<SuperpowersTopic> = by_topic.into_values().collect();
        topics.sort_by(|a, b| {
            compare_date(a.date.as_deref(), b.date.as_deref()).then_with(|| a.topic.cmp(&b.topic))
        });
        topics
    }

    /// Kurzer Hash über Pfad, Änderungszeit und Größe aller Dokumente – erkennt Änderungen,
    /// ohne die Dateien zu lesen
    pub fn signature(&self, root: &Path) -> String {
        let mut lines: Vec<String> = Vec::new();
        for (_, dir) in DIRS {
            for entry in entries(root, dir) {
                let (mtime_ms, size) = stat(&entry.full);
                lines.push(format!(
                    "docs/superpowers/{dir}/{}|{mtime_ms}|{size}",
                    entry.name
                ));
            }
        }
        lines.sort();
        let digest = Sha256::digest(lines.join("\n").as_bytes());
        digest
            .iter()
            .take(8)
            .fold(String::with_capacity(16), |mut text, byte| {
                use std::fmt::Write;
                let _ = write!(text, "{byte:02x}");
                text
            })
    }
}

/// Ein Dokument eines der beiden Ordner
struct DocEntry {
    name: String,
    full: PathBuf,
}

/// Die Dokumente eines der beiden Ordner (Namen nach [`DOC_FILE_NAME`]), ohne Rekursion;
/// fehlt der Ordner, ist die Liste leer. Die Namen sind sortiert, damit die Reihenfolge nicht
/// am Dateisystem hängt.
fn entries(root: &Path, dir: &str) -> Vec<DocEntry> {
    let abs = root.join("docs").join("superpowers").join(dir);
    let Ok(read) = fs::read_dir(&abs) else {
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
        .into_iter()
        .map(|name| DocEntry {
            full: abs.join(&name),
            name,
        })
        .collect()
}

/// Änderungszeit in Millisekunden und Größe einer Datei, wie `statSync` sie liefert
fn stat(file: &Path) -> (f64, u64) {
    let Ok(data) = fs::metadata(file) else {
        return (0.0, 0);
    };
    let mtime = data
        .modified()
        .ok()
        .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
        .map(|since| since.as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    (mtime, data.len())
}

/// Zerlegt den Dateinamen ohne Endung in Datum und Thema; das abschließende `-design` der Spec
/// gehört nicht zum Thema
fn parse_slug(slug: &str) -> (Option<String>, String) {
    let found = slug_pattern().captures(slug);
    let date = found.as_ref().map(|found| found[1].to_string());
    let rest = found
        .as_ref()
        .map(|found| found[2].to_string())
        .unwrap_or_else(|| slug.to_string());
    let topic = rest.strip_suffix("-design").unwrap_or(&rest).to_string();
    (date, topic)
}

/// Erste H1-Zeile der Datei; gibt es keine, entscheidet der Aufrufer über den Ersatz
fn first_heading(file: &Path) -> Option<String> {
    let text = fs::read_to_string(file).ok()?;
    split_lines(&text)
        .into_iter()
        .find_map(|line| {
            h1_pattern()
                .captures(&line)
                .map(|found| found[1].trim().to_string())
        })
        .filter(|heading| !heading.is_empty())
}

/// Neueres Datum zuerst, Dokumente ohne Datum ganz hinten
fn compare_date(a: Option<&str>, b: Option<&str>) -> Ordering {
    match (a, b) {
        (a, b) if a == b => Ordering::Equal,
        (None, _) => Ordering::Greater,
        (_, None) => Ordering::Less,
        (Some(a), Some(b)) => b.cmp(a),
    }
}

/// Reihenfolge der Dokumente: Datum, Thema, Sorte
fn compare_docs(a: &SuperpowersDoc, b: &SuperpowersDoc) -> Ordering {
    compare_date(a.date.as_deref(), b.date.as_deref())
        .then_with(|| a.topic.cmp(&b.topic))
        .then_with(|| a.kind.as_str().cmp(b.kind.as_str()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    /// Projekt mit den Superpowers-Ordnern
    struct Repo(tempfile::TempDir);

    impl Repo {
        fn new() -> Self {
            Self(
                tempfile::Builder::new()
                    .prefix("harness-docs-")
                    .tempdir()
                    .unwrap(),
            )
        }

        fn path(&self) -> &Path {
            self.0.path()
        }

        /// Legt eine Datei unter `docs/superpowers/<ordner>/` an
        fn write(&self, kind: &str, name: &str, content: &str) -> PathBuf {
            let dir = self.path().join("docs").join("superpowers").join(kind);
            fs::create_dir_all(&dir).unwrap();
            let file = dir.join(name);
            fs::write(&file, content).unwrap();
            file
        }
    }

    #[test]
    fn liest_datum_und_thema_aus_dem_dateinamen() {
        let repo = Repo::new();
        repo.write("specs", "2026-01-02-login-design.md", "");
        repo.write("plans", "2026-01-02-login.md", "");

        let docs = SuperpowersDocScanner::new().scan(repo.path());

        assert_eq!(
            docs.iter()
                .map(|doc| (
                    doc.kind.as_str(),
                    doc.date.as_deref(),
                    doc.topic.as_str(),
                    doc.slug.as_str()
                ))
                .collect::<Vec<_>>(),
            [
                ("plan", Some("2026-01-02"), "login", "2026-01-02-login"),
                (
                    "spec",
                    Some("2026-01-02"),
                    "login",
                    "2026-01-02-login-design"
                ),
            ]
        );
        assert_eq!(docs[0].path, "docs/superpowers/plans/2026-01-02-login.md");
        assert_eq!(docs[0].size, 0);
        assert!(docs[0].mtime_ms > 0.0);
    }

    #[test]
    fn nimmt_den_titel_aus_der_ersten_h1_sonst_den_dateinamen() {
        let repo = Repo::new();
        repo.write(
            "specs",
            "2026-01-02-login-design.md",
            "Vorspann\n\n# Anmeldung neu\n\n# Zweite\n",
        );
        repo.write("plans", "2026-01-02-login.md", "ohne Überschrift\n");

        let docs = SuperpowersDocScanner::new().scan(repo.path());

        let title = |kind: SuperpowersDocKind| {
            docs.iter()
                .find(|doc| doc.kind == kind)
                .map(|doc| doc.title.as_str())
        };
        assert_eq!(title(SuperpowersDocKind::Spec), Some("Anmeldung neu"));
        assert_eq!(title(SuperpowersDocKind::Plan), Some("2026-01-02-login"));
    }

    #[test]
    fn paart_spec_und_plan_uber_das_thema_und_lasst_fehlendes_leer() {
        let repo = Repo::new();
        repo.write("specs", "2026-01-02-login-design.md", "");
        repo.write("plans", "2026-01-02-login.md", "");
        repo.write("specs", "2026-01-03-suche-design.md", "");

        let topics = SuperpowersDocScanner::new().topics(repo.path());

        assert_eq!(
            topics.iter().map(|t| t.topic.as_str()).collect::<Vec<_>>(),
            ["suche", "login"]
        );
        assert!(topics[0].plan.is_none());
        assert_eq!(
            topics[0].spec.as_ref().map(|spec| spec.slug.as_str()),
            Some("2026-01-03-suche-design")
        );
        assert_eq!(
            topics[1].spec.as_ref().map(|spec| spec.kind),
            Some(SuperpowersDocKind::Spec)
        );
        assert_eq!(
            topics[1].plan.as_ref().map(|plan| plan.kind),
            Some(SuperpowersDocKind::Plan)
        );
        assert_eq!(topics[1].date.as_deref(), Some("2026-01-02"));
    }

    #[test]
    fn nimmt_dateien_ohne_datum_auf_und_sortiert_sie_nach_hinten() {
        let repo = Repo::new();
        repo.write("specs", "notizen.md", "");
        repo.write("specs", "2026-01-02-login-design.md", "");
        let scanner = SuperpowersDocScanner::new();

        let docs = scanner.scan(repo.path());

        assert_eq!(
            docs.iter()
                .map(|doc| doc.topic.as_str())
                .collect::<Vec<_>>(),
            ["login", "notizen"]
        );
        assert_eq!(docs[1].date, None);
        assert_eq!(
            scanner
                .topics(repo.path())
                .iter()
                .map(|t| t.topic.as_str())
                .collect::<Vec<_>>(),
            ["login", "notizen"]
        );
    }

    #[test]
    fn kommt_ohne_die_ordner_aus() {
        let repo = Repo::new();
        let scanner = SuperpowersDocScanner::new();

        assert!(scanner.scan(repo.path()).is_empty());
        assert!(scanner.topics(repo.path()).is_empty());
        assert_eq!(scanner.signature(repo.path()).len(), 16);
    }

    #[test]
    fn andert_die_signatur_bei_geanderter_und_bei_neuer_datei() {
        let repo = Repo::new();
        let file = repo.write("specs", "2026-01-02-login-design.md", "a");
        let scanner = SuperpowersDocScanner::new();
        let first = scanner.signature(repo.path());
        assert_eq!(scanner.signature(repo.path()), first);

        fs::write(&file, "anderer Inhalt").unwrap();
        let second = scanner.signature(repo.path());
        assert_ne!(second, first);

        repo.write("plans", "2026-01-02-login.md", "");
        assert_ne!(scanner.signature(repo.path()), second);
    }

    #[test]
    fn ubergeht_sidecars_und_andere_endungen() {
        let repo = Repo::new();
        repo.write("specs", "2026-01-02-login-design.md", "");
        repo.write("specs", "2026-01-02-login-design.comments.json", "");

        assert_eq!(
            SuperpowersDocScanner::new()
                .scan(repo.path())
                .iter()
                .map(|doc| doc.slug.as_str())
                .collect::<Vec<_>>(),
            ["2026-01-02-login-design"]
        );
    }

    #[test]
    fn listet_nur_dateinamen_die_sich_auch_lesen_lassen() {
        let repo = Repo::new();
        repo.write("specs", "2026-01-02-login-design.md", "");
        repo.write("specs", "2026-01-03 mit Leerzeichen.md", "");
        repo.write("specs", "2026-01-04-groß.md", "");
        repo.write("plans", "2026-01-05-laut.MD", "");

        assert_eq!(
            SuperpowersDocScanner::new()
                .scan(repo.path())
                .iter()
                .map(|doc| doc.slug.as_str())
                .collect::<Vec<_>>(),
            ["2026-01-02-login-design"]
        );
    }

    #[test]
    fn liest_im_angegebenen_ordner_auch_unterhalb_der_repo_wurzel() {
        let repo = Repo::new();
        repo.write("specs", "2026-01-02-oben-design.md", "");
        let sub = repo.path().join("app");
        fs::create_dir_all(sub.join("docs/superpowers/plans")).unwrap();
        fs::write(sub.join("docs/superpowers/plans/2026-01-03-unten.md"), "").unwrap();
        let scanner = SuperpowersDocScanner::new();

        assert_eq!(
            scanner
                .scan(&sub)
                .iter()
                .map(|doc| doc.path.as_str())
                .collect::<Vec<_>>(),
            ["docs/superpowers/plans/2026-01-03-unten.md"]
        );

        let plain = tempfile::Builder::new()
            .prefix("harness-docs-leer-")
            .tempdir()
            .unwrap();
        assert!(scanner.scan(plain.path()).is_empty());
        let signature = scanner.signature(plain.path());
        assert_eq!(signature.len(), 16);
        assert!(signature.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
