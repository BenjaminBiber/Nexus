//! Liest die Testläufe des Agenten aus der Ausgabe von `claude -p --output-format
//! stream-json`; portiert aus `packages/core/src/dispatch/TestRunExtractor.ts`.

use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;
use serde_json::Value;

use super::text::last_chars;
use crate::dispatch::TestRun;

/// Höchstens so viele Zeichen der Ausgabe je Testlauf (das Ende bleibt)
const MAX_OUTPUT: usize = 4000;

/// Werkzeuge, deren Aufrufe Befehle ausführen
const SHELL_TOOLS: [&str; 2] = ["Bash", "PowerShell"];

/// Anfang eines Befehls: Zeilenanfang oder nach ; & | (, optional npx, python -m oder ./ davor
const START: &str = r"(?:^|[;&|(]\s*)(?:npx\s+|python3?\s+-m\s+|\./)?";

/// Befehle, die als Testlauf gelten (Tests, Build, Lint, Typprüfung)
static TEST_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(vitest|jest|pytest)\b",
        r"npm\s+(test|t)\b",
        r"npm\s+run\s+(test|build|lint|typecheck)\b",
        r"tsc\b",
        r"dotnet\s+test\b",
        r"go\s+test\b",
        r"cargo\s+test\b",
        r"(mvn|mvnw|gradle|gradlew)\b.*\btest\b",
    ]
    .into_iter()
    .map(|pattern| Regex::new(&format!("{START}{pattern}")).expect("festes Muster"))
    .collect()
});

/// Liest die Testläufe des Agenten aus der Ausgabe eines Laufs.
#[derive(Debug, Default)]
pub struct TestRunExtractor;

impl TestRunExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Testläufe in der Reihenfolge der Aufrufe; Zeilen ohne JSON fallen weg
    pub fn extract(&self, output: &str) -> Vec<TestRun> {
        let mut runs: IndexMap<String, TestRun> = IndexMap::new();
        for line in output.split('\n') {
            for id in denied_ids(line) {
                runs.shift_remove(&id);
            }
            for block in blocks(line) {
                let kind = block.get("type").and_then(Value::as_str);
                let name = block.get("name").and_then(Value::as_str).unwrap_or("");
                if kind == Some("tool_use") && SHELL_TOOLS.contains(&name) {
                    let Some(id) = block.get("id").and_then(Value::as_str) else {
                        continue;
                    };
                    let command = command(block.get("input"));
                    if !command.is_empty() && self.is_test(&command) {
                        runs.insert(
                            id.to_string(),
                            TestRun {
                                command,
                                ok: None,
                                output: String::new(),
                            },
                        );
                    }
                } else if kind == Some("tool_result") {
                    let Some(id) = block.get("tool_use_id").and_then(Value::as_str) else {
                        continue;
                    };
                    let Some(run) = runs.get_mut(id) else {
                        continue;
                    };
                    let failed = block.get("is_error") == Some(&Value::Bool(true));
                    let content = text(block.get("content"));
                    // Von der Rechteprüfung abgelehnt: der Befehl lief nie, also kein Testlauf
                    if failed && is_denial(&content) {
                        runs.shift_remove(id);
                        continue;
                    }
                    run.ok = Some(!failed);
                    run.output = last_chars(&content, MAX_OUTPUT);
                }
            }
        }
        runs.into_values().collect()
    }

    /// Ist der Befehl ein Testlauf?
    pub fn is_test(&self, command: &str) -> bool {
        TEST_PATTERNS
            .iter()
            .any(|pattern| pattern.is_match(command))
    }
}

/// Inhaltsblöcke einer Zeile (assistant/user); leer, wenn die Zeile kein solches Ereignis ist
fn blocks(line: &str) -> Vec<Value> {
    let text = line.trim();
    if !text.starts_with('{') {
        return Vec::new();
    }
    let Ok(event) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    event
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(Value::as_array)
        .map(|content| {
            content
                .iter()
                .filter(|block| block.is_object())
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// Antworten von Claude Code, wenn die Rechteprüfung einen Aufruf ablehnt
const DENIAL_MARKERS: [&str; 3] = [
    "requires approval",
    "requested permissions to use",
    "haven't granted it yet",
];

/// Ist das Ergebnis die Ablehnung durch die Rechteprüfung statt einer Ausgabe des Befehls?
fn is_denial(content: &str) -> bool {
    DENIAL_MARKERS.iter().any(|marker| content.contains(marker))
}

/// Kennungen der Aufrufe, die das Ergebnis eines Laufs unter `permission_denials` nennt
fn denied_ids(line: &str) -> Vec<String> {
    let text = line.trim();
    if !text.starts_with('{') || !text.contains("permission_denials") {
        return Vec::new();
    }
    let Ok(event) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    event
        .get("permission_denials")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|denial| denial.get("tool_use_id").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

/// Befehl eines Werkzeugaufrufs
fn command(input: Option<&Value>) -> String {
    input
        .and_then(|input| input.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Text eines Ergebnisses: String oder Liste von Textblöcken
fn text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .filter(|text| !text.is_empty())
            .collect::<Vec<&str>>()
            .join("\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Zeile von stream-json: Werkzeugaufruf des Assistenten
    fn call(id: &str, command: &str, name: &str) -> String {
        json!({ "type": "assistant", "message": { "content": [
            { "type": "tool_use", "id": id, "name": name, "input": { "command": command } },
        ] } })
        .to_string()
    }

    /// Zeile von stream-json: Ergebnis eines Werkzeugaufrufs
    fn result(id: &str, content: Value, is_error: bool) -> String {
        json!({ "type": "user", "message": { "content": [
            { "type": "tool_result", "tool_use_id": id, "content": content, "is_error": is_error },
        ] } })
        .to_string()
    }

    fn run(command: &str, ok: Option<bool>, output: &str) -> TestRun {
        TestRun {
            command: command.to_string(),
            ok,
            output: output.to_string(),
        }
    }

    #[test]
    fn paart_testaufrufe_mit_ihrem_ergebnis_grun_rot_kein_test() {
        let output = [
            call("a", "npx vitest run packages/core", "Bash"),
            result("a", json!("Tests 12 passed"), false),
            call("b", "git status --short", "Bash"),
            result("b", json!(" M x.ts"), false),
            call("c", "npm test", "PowerShell"),
            result("c", json!("FAIL x.test.ts"), true),
            "keine JSON-Zeile".to_string(),
        ]
        .join("\n");
        assert_eq!(
            TestRunExtractor::new().extract(&output),
            vec![
                run(
                    "npx vitest run packages/core",
                    Some(true),
                    "Tests 12 passed"
                ),
                run("npm test", Some(false), "FAIL x.test.ts"),
            ]
        );
    }

    #[test]
    fn erkennt_gangige_testbefehle_und_lasst_anderes_weg() {
        let extractor = TestRunExtractor::new();
        for command in [
            "npm run test:unit",
            "npm run build",
            "npm run lint",
            "npm run typecheck",
            "npx tsc --noEmit -p packages/web",
            "tsc --noEmit",
            "npx jest",
            "dotnet test Foo.sln",
            "pytest -q",
            "python -m pytest",
            "go test ./...",
            "cargo test",
            "mvn test",
            "./gradlew test",
            "cd web && npm test",
        ] {
            assert!(extractor.is_test(command), "{command}");
        }
        for command in [
            "git status",
            "npm ci",
            "npm install",
            "ls tests",
            "cat vitest.config.ts",
            "harness board show AIH-0001",
        ] {
            assert!(!extractor.is_test(command), "{command}");
        }
    }

    #[test]
    fn abgelehnte_aufrufe_zahlen_nicht_als_testlauf() {
        let output = [
            call("a", "cd x && python -m pytest --version 2>&1 | head -1", "Bash"),
            result(
                "a",
                json!("This Bash command contains multiple operations. The following part requires approval: cd x && python -m pytest --version 2>&1"),
                true,
            ),
            call("b", "npm test", "Bash"),
            call("c", "cargo test", "Bash"),
            result("c", json!("ok"), false),
            json!({ "type": "result", "subtype": "success", "is_error": false,
                "permission_denials": [{ "tool_name": "Bash", "tool_use_id": "b" }] })
            .to_string(),
        ]
        .join("\n");
        assert_eq!(
            TestRunExtractor::new().extract(&output),
            vec![run("cargo test", Some(true), "ok")]
        );
    }

    #[test]
    fn ein_aufruf_ohne_ergebnis_ist_ohne_ergebnis() {
        assert_eq!(
            TestRunExtractor::new().extract(&call("a", "npm test", "Bash")),
            vec![run("npm test", None, "")]
        );
    }

    #[test]
    fn kurzt_die_ausgabe_auf_die_letzten_4000_zeichen() {
        let long = format!("{}ENDE", "x".repeat(5000));
        let output = [
            call("a", "npm test", "Bash"),
            result("a", json!(long), false),
        ]
        .join("\n");
        let runs = TestRunExtractor::new().extract(&output);
        assert_eq!(runs[0].output.chars().count(), 4000);
        assert!(runs[0].output.ends_with("ENDE"));
    }

    #[test]
    fn liest_ergebnisse_als_liste_von_textblocken() {
        let output = [
            call("a", "npm test", "Bash"),
            result(
                "a",
                json!([{ "type": "text", "text": "Teil 1" }, { "type": "text", "text": "Teil 2" }]),
                false,
            ),
        ]
        .join("\n");
        assert_eq!(
            TestRunExtractor::new().extract(&output)[0].output,
            "Teil 1\nTeil 2"
        );
    }
}
