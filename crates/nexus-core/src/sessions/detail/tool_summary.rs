//! Kurzinfo eines Tool-Aufrufs für die einzeilige Darstellung im Verlauf, z. B.
//! `Edit src/x.ts (+12 −3)`, `Bash: npm test` oder `navigate (claude-in-chrome)`.

use serde_json::Value;

use super::jsonl::text;
use super::line_diff;

/// Höchstlänge eines Befehls bzw. einer Beschreibung in der Kurzinfo
const MAX_PART: usize = 120;

/// Kurzinfo zu Tool `name` mit Eingabe `input`; Pfade relativ zu `cwd`, wenn sie darin liegen
pub fn summarize(name: &str, input: &Value, cwd: Option<&str>) -> String {
    let field = |key: &str| text(input, key).map(one_line);
    let path = |key: &str| text(input, key).map(|path| relative(path, cwd));
    match name {
        "Edit" => match path("file_path") {
            Some(file) => {
                let (added, removed) = edit_counts(input);
                format!("Edit {file} (+{added} −{removed})")
            }
            None => name.to_string(),
        },
        "MultiEdit" => match path("file_path") {
            Some(file) => {
                let (added, removed) = input
                    .get("edits")
                    .and_then(Value::as_array)
                    .map(|edits| {
                        edits
                            .iter()
                            .map(edit_counts)
                            .fold((0, 0), |sum, next| (sum.0 + next.0, sum.1 + next.1))
                    })
                    .unwrap_or((0, 0));
                format!("MultiEdit {file} (+{added} −{removed})")
            }
            None => name.to_string(),
        },
        "Write" => match path("file_path") {
            Some(file) => {
                let lines = input
                    .get("content")
                    .and_then(Value::as_str)
                    .map_or(0, |content| content.lines().count());
                format!("Write {file} (+{lines})")
            }
            None => name.to_string(),
        },
        "Read" | "NotebookEdit" | "NotebookRead" => {
            match path("file_path").or_else(|| path("notebook_path")) {
                Some(file) => format!("{name} {file}"),
                None => name.to_string(),
            }
        }
        "Bash" | "PowerShell" => with(name, ": ", field("command")),
        "Grep" => with(name, " ", field("pattern").map(|pattern| quoted(&pattern))),
        "Glob" => with(name, " ", field("pattern")),
        "Task" | "Agent" => {
            let description = field("description").or_else(|| field("prompt"));
            match (field("subagent_type"), description) {
                (Some(kind), Some(description)) => format!("{name} {kind}: {description}"),
                (Some(kind), None) => format!("{name} {kind}"),
                (None, description) => with(name, ": ", description),
            }
        }
        "WebFetch" => with(name, " ", field("url")),
        "WebSearch" | "ToolSearch" => with(name, " ", field("query").map(|query| quoted(&query))),
        "TodoWrite" => {
            let count = input
                .get("todos")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            format!("TodoWrite ({count} Aufgaben)")
        }
        "TaskCreate" => with(name, ": ", field("subject")),
        "TaskUpdate" => match (task_id(input), field("status")) {
            (Some(id), Some(status)) => format!("TaskUpdate #{id} → {status}"),
            (Some(id), None) => format!("TaskUpdate #{id}"),
            _ => name.to_string(),
        },
        "Skill" => with(name, " ", field("skill").or_else(|| field("command"))),
        _ => mcp_name(name).unwrap_or_else(|| name.to_string()),
    }
}

/// `name` mit Trenner und Teil, ohne Teil nur `name`
fn with(name: &str, separator: &str, part: Option<String>) -> String {
    match part {
        Some(part) => format!("{name}{separator}{part}"),
        None => name.to_string(),
    }
}

/// Erste Zeile, auf [`MAX_PART`] Zeichen gekürzt
fn one_line(value: &str) -> String {
    let first = value.trim().lines().next().unwrap_or("").trim();
    let more = value.trim().lines().nth(1).is_some();
    let mut cut: String = first.chars().take(MAX_PART).collect();
    if more || first.chars().count() > MAX_PART {
        cut.push('…');
    }
    cut
}

/// In Anführungszeichen
fn quoted(value: &str) -> String {
    format!("\"{value}\"")
}

/// Nummer einer Aufgabe (Text oder Zahl)
fn task_id(input: &Value) -> Option<String> {
    match input.get("taskId")? {
        Value::String(id) if !id.is_empty() => Some(id.clone()),
        Value::Number(id) => Some(id.to_string()),
        _ => None,
    }
}

/// Hinzugefügte und entfernte Zeilen einer Ersetzung `old_string → new_string`
fn edit_counts(edit: &Value) -> (i64, i64) {
    let old = edit.get("old_string").and_then(Value::as_str).unwrap_or("");
    let new = edit.get("new_string").and_then(Value::as_str).unwrap_or("");
    line_diff::counts(&line_diff::diff(
        &line_diff::split(old),
        &line_diff::split(new),
    ))
}

/// `mcp__server__tool` → `tool (server)`
fn mcp_name(name: &str) -> Option<String> {
    let rest = name.strip_prefix("mcp__")?;
    let (server, tool) = rest.rsplit_once("__")?;
    Some(format!("{tool} ({server})"))
}

/// Pfad relativ zu `cwd` mit `/`, wenn er darin liegt; sonst unverändert (mit `/`)
pub fn relative(path: &str, cwd: Option<&str>) -> String {
    let path = path.replace('\\', "/");
    let Some(cwd) = cwd else {
        return path;
    };
    let base = cwd.replace('\\', "/");
    let base = base.trim_end_matches('/');
    if base.is_empty() || path.len() <= base.len() + 1 {
        return path;
    }
    let (head, tail) = path.split_at(base.len());
    let same = if cfg!(windows) {
        head.eq_ignore_ascii_case(base)
    } else {
        head == base
    };
    match tail.strip_prefix('/') {
        Some(rest) if same => rest.to_string(),
        _ => path,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const CWD: Option<&str> = Some("D:/Projekte/demo");

    #[test]
    fn edit_zeigt_relativen_pfad_und_zeilenzahlen() {
        let input = json!({
            "file_path": "D:\\Projekte\\demo\\src\\x.ts",
            "old_string": "a\nb\nc",
            "new_string": "a\nB\nc\nd",
        });
        assert_eq!(summarize("Edit", &input, CWD), "Edit src/x.ts (+2 −1)");
    }

    #[test]
    fn multiedit_summiert_die_ersetzungen() {
        let input = json!({
            "file_path": "D:/Projekte/demo/a.rs",
            "edits": [
                { "old_string": "x", "new_string": "y" },
                { "old_string": "", "new_string": "neu\nneu" },
            ],
        });
        assert_eq!(
            summarize("MultiEdit", &input, CWD),
            "MultiEdit a.rs (+3 −1)"
        );
    }

    #[test]
    fn write_read_und_pfade_ausserhalb_des_arbeitsordners() {
        assert_eq!(
            summarize(
                "Write",
                &json!({ "file_path": "D:/Projekte/demo/n.md", "content": "a\nb\n" }),
                CWD
            ),
            "Write n.md (+2)"
        );
        assert_eq!(
            summarize("Read", &json!({ "file_path": "C:/anderswo/a.rs" }), CWD),
            "Read C:/anderswo/a.rs"
        );
    }

    #[test]
    fn befehle_zeigen_die_erste_zeile_gekuerzt() {
        assert_eq!(
            summarize("Bash", &json!({ "command": "npm test" }), CWD),
            "Bash: npm test"
        );
        assert_eq!(
            summarize(
                "PowerShell",
                &json!({ "command": "Get-Item a\nGet-Item b" }),
                CWD
            ),
            "PowerShell: Get-Item a…"
        );
        let long = "x".repeat(200);
        let summary = summarize("Bash", &json!({ "command": long }), CWD);
        assert_eq!(summary.chars().count(), "Bash: ".len() + MAX_PART + 1);
    }

    #[test]
    fn suche_agenten_und_aufgaben() {
        assert_eq!(
            summarize("Grep", &json!({ "pattern": "fn main" }), CWD),
            "Grep \"fn main\""
        );
        assert_eq!(
            summarize("Glob", &json!({ "pattern": "**/*.rs" }), CWD),
            "Glob **/*.rs"
        );
        assert_eq!(
            summarize(
                "Task",
                &json!({ "subagent_type": "Explore", "description": "Suche Router", "prompt": "…" }),
                CWD
            ),
            "Task Explore: Suche Router"
        );
        assert_eq!(
            summarize("Agent", &json!({ "description": "Prüfen" }), CWD),
            "Agent: Prüfen"
        );
        assert_eq!(
            summarize("TodoWrite", &json!({ "todos": [{}, {}, {}] }), CWD),
            "TodoWrite (3 Aufgaben)"
        );
        assert_eq!(
            summarize("TaskCreate", &json!({ "subject": "Tests schreiben" }), CWD),
            "TaskCreate: Tests schreiben"
        );
        assert_eq!(
            summarize(
                "TaskUpdate",
                &json!({ "taskId": "2", "status": "completed" }),
                CWD
            ),
            "TaskUpdate #2 → completed"
        );
    }

    #[test]
    fn web_skills_und_mcp_tools_mit_kurznamen() {
        assert_eq!(
            summarize("WebSearch", &json!({ "query": "rust axum" }), CWD),
            "WebSearch \"rust axum\""
        );
        assert_eq!(
            summarize("WebFetch", &json!({ "url": "https://example.org" }), CWD),
            "WebFetch https://example.org"
        );
        assert_eq!(
            summarize("Skill", &json!({ "skill": "board" }), CWD),
            "Skill board"
        );
        assert_eq!(
            summarize("mcp__claude-in-chrome__navigate", &json!({}), CWD),
            "navigate (claude-in-chrome)"
        );
        assert_eq!(summarize("Unbekannt", &json!({}), CWD), "Unbekannt");
    }
}
