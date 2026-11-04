//! Änderungen einer Session aus ihrem Transcript: die erfolgreichen `Edit`, `MultiEdit`,
//! `Write` und `NotebookEdit` werden je Datei in Reihenfolge verdichtet.
//!
//! Je Datei gibt es einen Ausgangsstand (unbekannt, nicht vorhanden oder Text) und den
//! aktuellen Stand. Den Ausgangsstand liefert `toolUseResult.originalFile` bzw. `type:
//! "create"` beim ersten Werkzeug an der Datei. Ohne ihn bleiben von Edits nur Ausschnitte
//! (alt → neu), von einem Write nur der neue Inhalt („+“ mit Hinweis). Dateien außerhalb des
//! Arbeitsordners entfallen, ebenso Dateien, die wieder im Ausgangsstand sind.

use std::collections::HashMap;

use serde_json::Value;

use super::jsonl::{flag, text};
use super::line_diff::{self, Op};
use super::tool_summary::relative;
use super::types::{ChangedFile, DiffHunk, FileDiff, FileStatus};
use super::unified_diff;

/// Hinweis an Abschnitten, deren Ausgangsstand fehlt
const UNKNOWN_BASE: &str = "Ausgangsstand unbekannt";
/// Kopfzeile eines Ausschnitts ohne Zeilennummern
const FRAGMENT_HEADER: &str = "@@ Ausschnitt @@";

/// Ausgangsstand einer Datei
#[derive(Debug, Clone, PartialEq)]
enum Base {
    Unknown,
    Missing,
    Text(String),
}

/// Stand einer geänderten Datei
#[derive(Debug, Clone)]
struct FileState {
    base: Base,
    current: Option<String>,
    /// Ersetzungen, die sich auf keinen bekannten Stand anwenden ließen
    fragments: Vec<(String, String)>,
}

/// Sammelt die Änderungen Zeile für Zeile
#[derive(Debug, Clone)]
pub struct TranscriptChanges {
    cwd: String,
    files: HashMap<String, FileState>,
    /// Aufrufe ändernder Tools ohne Ergebnis: Kennung → (Name, Eingabe)
    pending: HashMap<String, (String, Value)>,
}

impl TranscriptChanges {
    /// Neue Sammlung für den Arbeitsordner `cwd`
    pub fn new(cwd: &str) -> Self {
        Self {
            cwd: cwd.replace('\\', "/"),
            files: HashMap::new(),
            pending: HashMap::new(),
        }
    }

    /// Verarbeitet eine Zeile des Transcripts (auch von Subagenten: ihre Änderungen zählen)
    pub fn feed(&mut self, value: &Value) {
        match text(value, "type") {
            Some("assistant") => self.calls(value),
            Some("user") => self.results(value),
            _ => {}
        }
    }

    /// Die geänderten Dateien, nach Pfad sortiert
    pub fn files(&self) -> Vec<ChangedFile> {
        let mut list: Vec<ChangedFile> = self
            .files
            .iter()
            .filter_map(|(path, state)| changed(path, state))
            .collect();
        list.sort_by(|a, b| a.path.cmp(&b.path));
        list
    }

    /// Diff einer geänderten Datei; nichts bei unbekanntem Pfad
    pub fn diff(&self, path: &str) -> Option<FileDiff> {
        let state = self.files.get(path)?;
        changed(path, state)?;
        let mut hunks = Vec::new();
        match (&state.base, &state.current) {
            (Base::Text(base), Some(current)) => {
                hunks = line_diff::hunks(&line_diff::diff(
                    &line_diff::split(base),
                    &line_diff::split(current),
                ));
            }
            (Base::Missing, Some(current)) => {
                hunks = unified_diff::all_added(path, current, None).hunks;
            }
            (Base::Unknown, Some(current)) => {
                hunks = unified_diff::all_added(path, current, Some(UNKNOWN_BASE)).hunks;
            }
            _ => {}
        }
        hunks.extend(state.fragments.iter().map(|(old, new)| fragment(old, new)));
        Some(unified_diff::limit(path, hunks))
    }

    /// Merkt sich die Aufrufe ändernder Tools
    fn calls(&mut self, value: &Value) {
        let Some(blocks) = value.pointer("/message/content").and_then(Value::as_array) else {
            return;
        };
        for block in blocks {
            let name = text(block, "name").unwrap_or("");
            if text(block, "type") == Some("tool_use")
                && matches!(name, "Edit" | "MultiEdit" | "Write" | "NotebookEdit")
            {
                if let Some(id) = text(block, "id") {
                    let input = block.get("input").cloned().unwrap_or(Value::Null);
                    self.pending
                        .insert(id.to_string(), (name.to_string(), input));
                }
            }
        }
    }

    /// Wendet erfolgreiche Ergebnisse an; `toolUseResult` gilt nur bei genau einem Ergebnis
    fn results(&mut self, value: &Value) {
        let Some(parts) = value.pointer("/message/content").and_then(Value::as_array) else {
            return;
        };
        let results: Vec<&Value> = parts
            .iter()
            .filter(|part| text(part, "type") == Some("tool_result"))
            .collect();
        let details = if results.len() == 1 {
            value.get("toolUseResult").unwrap_or(&Value::Null)
        } else {
            &Value::Null
        };
        for part in results {
            let Some(id) = text(part, "tool_use_id") else {
                continue;
            };
            let Some((name, input)) = self.pending.remove(id) else {
                continue;
            };
            if !flag(part, "is_error") {
                self.apply(&name, &input, details);
            }
        }
    }

    /// Wendet eine erfolgreiche Änderung an
    fn apply(&mut self, name: &str, input: &Value, details: &Value) {
        let key = if name == "NotebookEdit" {
            "notebook_path"
        } else {
            "file_path"
        };
        let Some(path) = text(input, key).and_then(|path| self.inside(path)) else {
            return;
        };
        let original = details.get("originalFile").and_then(Value::as_str);
        let created = text(details, "type") == Some("create");
        let state = self.files.entry(path).or_insert_with(|| FileState {
            base: match (created, original) {
                (true, _) => Base::Missing,
                (false, Some(text)) => Base::Text(text.to_string()),
                (false, None) => Base::Unknown,
            },
            current: if created {
                Some(String::new())
            } else {
                original.map(str::to_string)
            },
            fragments: Vec::new(),
        });
        match name {
            "Write" => {
                state.current = Some(
                    input
                        .get("content")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                );
                state.fragments.clear();
            }
            "Edit" => {
                if let Some(original) = original {
                    state.current = Some(original.to_string());
                }
                replace(state, input);
            }
            "MultiEdit" => {
                if let Some(original) = original {
                    state.current = Some(original.to_string());
                }
                for edit in input
                    .get("edits")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    replace(state, edit);
                }
            }
            _ => {
                let source = text(input, "new_source").unwrap_or("").to_string();
                state.fragments.push((String::new(), source));
            }
        }
    }

    /// Pfad relativ zum Arbeitsordner, wenn die Datei darin liegt
    fn inside(&self, path: &str) -> Option<String> {
        let normalized = path.replace('\\', "/");
        let rel = relative(&normalized, Some(&self.cwd));
        let absolute = normalized.starts_with('/') || normalized.contains(':');
        if absolute && rel == normalized {
            return None;
        }
        (!rel.split('/').any(|part| part == "..")).then_some(rel)
    }
}

/// Wendet eine Ersetzung `old_string → new_string` auf den aktuellen Stand an; ohne
/// bekannten Stand (oder wenn der alte Text fehlt) bleibt sie als Ausschnitt
fn replace(state: &mut FileState, edit: &Value) {
    let old = edit.get("old_string").and_then(Value::as_str).unwrap_or("");
    let new = edit.get("new_string").and_then(Value::as_str).unwrap_or("");
    let all = flag(edit, "replace_all");
    match state.current.as_mut() {
        Some(current) if old.is_empty() && current.is_empty() => *current = new.to_string(),
        Some(current) if !old.is_empty() && current.contains(old) => {
            *current = if all {
                current.replace(old, new)
            } else {
                current.replacen(old, new, 1)
            };
        }
        _ => state.fragments.push((old.to_string(), new.to_string())),
    }
}

/// Die Datei mit Zahlen, wenn sie sich geändert hat
fn changed(path: &str, state: &FileState) -> Option<ChangedFile> {
    let (mut added, mut removed) = (0, 0);
    let mut status = FileStatus::Modified;
    match (&state.base, &state.current) {
        (Base::Text(base), Some(current)) => {
            if base != current || !state.fragments.is_empty() {
                let counts = line_diff::counts(&line_diff::diff(
                    &line_diff::split(base),
                    &line_diff::split(current),
                ));
                (added, removed) = counts;
            }
        }
        (Base::Missing, Some(current)) => {
            status = FileStatus::Added;
            added = current.lines().count() as i64;
        }
        (Base::Unknown, Some(current)) => added = current.lines().count() as i64,
        _ => {}
    }
    for (old, new) in &state.fragments {
        let counts = line_diff::counts(&line_diff::diff(
            &line_diff::split(old),
            &line_diff::split(new),
        ));
        added += counts.0;
        removed += counts.1;
    }
    let reverted = matches!((&state.base, &state.current), (Base::Text(base), Some(current)) if base == current)
        && state.fragments.is_empty();
    if reverted {
        return None;
    }
    Some(ChangedFile {
        path: path.to_string(),
        status,
        added,
        removed,
    })
}

/// Ein Ausschnitt alt → neu als Abschnitt ohne Zeilennummern
fn fragment(old: &str, new: &str) -> DiffHunk {
    let ops: Vec<Op<'_>> = line_diff::diff(&line_diff::split(old), &line_diff::split(new));
    DiffHunk {
        header: FRAGMENT_HEADER.to_string(),
        lines: ops.into_iter().map(line_diff::line).collect(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::fixtures::{self, file, tool_result, tool_use};
    use super::super::types::DiffLineKind;
    use super::*;

    /// Sammelt die Änderungen eines Transcripts
    fn collect(content: &str) -> TranscriptChanges {
        let mut changes = TranscriptChanges::new(fixtures::CWD);
        for line in content.lines() {
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                changes.feed(&value);
            }
        }
        changes
    }

    /// Transcript aus Aufruf-/Ergebnis-Paaren
    fn calls(pairs: Vec<(Value, Value)>) -> String {
        pairs
            .into_iter()
            .flat_map(|(call, result)| [fixtures::line(call), fixtures::line(result)])
            .collect()
    }

    fn signs(diff: &FileDiff) -> String {
        diff.hunks
            .iter()
            .flat_map(|hunk| hunk.lines.iter())
            .map(|line| match line.kind {
                DiffLineKind::Add => '+',
                DiffLineKind::Remove => '-',
                DiffLineKind::Context => ' ',
            })
            .collect()
    }

    #[test]
    fn verdichtet_edit_und_neue_datei_des_beispiels() {
        let changes = collect(&fixtures::transcript());
        assert_eq!(
            changes.files(),
            vec![
                ChangedFile {
                    path: "src/main.rs".into(),
                    status: FileStatus::Modified,
                    added: 3,
                    removed: 1
                },
                ChangedFile {
                    path: "src/neu.rs".into(),
                    status: FileStatus::Added,
                    added: 1,
                    removed: 0
                },
            ]
        );
        let diff = changes.diff("src/main.rs").unwrap();
        assert_eq!(diff.hunks[0].header, "@@ -1,1 +1,3 @@");
        assert_eq!(signs(&diff), "-+++");
        assert_eq!(
            changes.diff("src/neu.rs").unwrap().hunks[0].header,
            "@@ -0,0 +1,1 @@"
        );
        assert!(changes.diff("src/fremd.rs").is_none());
    }

    #[test]
    fn mehrere_edits_und_multiedit_auf_bekanntem_stand() {
        let original = "a\nb\nc\nd\n";
        let content = calls(vec![
            (
                tool_use(
                    "c1",
                    "m1",
                    "t1",
                    "Edit",
                    json!({ "file_path": file("x.txt"), "old_string": "b", "new_string": "B" }),
                    [0; 4],
                ),
                tool_result(
                    "r1",
                    "t1",
                    json!("ok"),
                    false,
                    json!({ "originalFile": original }),
                ),
            ),
            (
                tool_use(
                    "c2",
                    "m2",
                    "t2",
                    "MultiEdit",
                    json!({ "file_path": file("x.txt"), "edits": [
                    { "old_string": "c", "new_string": "C" },
                    { "old_string": "d", "new_string": "d\ne" },
                ] }),
                    [0; 4],
                ),
                tool_result(
                    "r2",
                    "t2",
                    json!("ok"),
                    false,
                    json!({ "originalFile": "a\nB\nc\nd\n" }),
                ),
            ),
        ]);
        let changes = collect(&content);
        let files = changes.files();
        assert_eq!((files[0].added, files[0].removed), (3, 2));
        assert_eq!(signs(&changes.diff("x.txt").unwrap()), " --++ +");
    }

    #[test]
    fn write_auf_bestehende_datei_mit_und_ohne_ausgangsstand() {
        let content = calls(vec![
            (
                tool_use(
                    "c1",
                    "m1",
                    "t1",
                    "Write",
                    json!({ "file_path": file("bekannt.txt"), "content": "eins\nzwei\n" }),
                    [0; 4],
                ),
                tool_result(
                    "r1",
                    "t1",
                    json!("ok"),
                    false,
                    json!({ "type": "update", "originalFile": "eins\n" }),
                ),
            ),
            (
                tool_use(
                    "c2",
                    "m2",
                    "t2",
                    "Write",
                    json!({ "file_path": file("fremd.txt"), "content": "neu\n" }),
                    [0; 4],
                ),
                tool_result("r2", "t2", json!("ok"), false, json!({ "type": "update" })),
            ),
        ]);
        let changes = collect(&content);
        let bekannt = changes.diff("bekannt.txt").unwrap();
        assert_eq!(signs(&bekannt), " +");
        let fremd = changes.diff("fremd.txt").unwrap();
        assert_eq!(
            fremd.hunks[0].header,
            "@@ -0,0 +1,1 @@ Ausgangsstand unbekannt"
        );
        assert_eq!(signs(&fremd), "+");
        assert_eq!(changes.files()[1].status, FileStatus::Modified);
    }

    #[test]
    fn edit_ohne_ausgangsstand_bleibt_ausschnitt() {
        let content = calls(vec![(
            tool_use(
                "c1",
                "m1",
                "t1",
                "Edit",
                json!({ "file_path": file("a.rs"), "old_string": "x\ny", "new_string": "x\nz" }),
                [0; 4],
            ),
            tool_result("r1", "t1", json!("ok"), false, json!({})),
        )]);
        let changes = collect(&content);
        let diff = changes.diff("a.rs").unwrap();
        assert_eq!(diff.hunks[0].header, FRAGMENT_HEADER);
        assert_eq!(signs(&diff), " -+");
        assert_eq!(
            (changes.files()[0].added, changes.files()[0].removed),
            (1, 1)
        );
    }

    #[test]
    fn ignoriert_fehler_fremde_pfade_und_zurueckgenommene_aenderungen() {
        let content = calls(vec![
            (
                tool_use(
                    "c1",
                    "m1",
                    "t1",
                    "Edit",
                    json!({ "file_path": file("f.rs"), "old_string": "a", "new_string": "b" }),
                    [0; 4],
                ),
                tool_result("r1", "t1", json!("String not found"), true, json!("Error")),
            ),
            (
                tool_use(
                    "c2",
                    "m2",
                    "t2",
                    "Write",
                    json!({ "file_path": "C:\\anderswo\\x.rs", "content": "x" }),
                    [0; 4],
                ),
                tool_result("r2", "t2", json!("ok"), false, json!({ "type": "create" })),
            ),
            (
                tool_use(
                    "c3",
                    "m3",
                    "t3",
                    "Edit",
                    json!({ "file_path": file("z.rs"), "old_string": "a", "new_string": "b" }),
                    [0; 4],
                ),
                tool_result(
                    "r3",
                    "t3",
                    json!("ok"),
                    false,
                    json!({ "originalFile": "a\n" }),
                ),
            ),
            (
                tool_use(
                    "c4",
                    "m4",
                    "t4",
                    "Edit",
                    json!({ "file_path": file("z.rs"), "old_string": "b", "new_string": "a" }),
                    [0; 4],
                ),
                tool_result(
                    "r4",
                    "t4",
                    json!("ok"),
                    false,
                    json!({ "originalFile": "b\n" }),
                ),
            ),
        ]);
        assert!(collect(&content).files().is_empty());
    }
}
