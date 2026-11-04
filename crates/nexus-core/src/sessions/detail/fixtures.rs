//! Gekürztes, anonymisiertes Transcript für die Tests der Detailansicht. Aufgebaut wie die
//! JSONL-Dateien von Claude Code (Zeilentypen, Felder, `toolUseResult`), Inhalte erfunden.

use serde_json::{json, Value};

/// Arbeitsordner der Beispiel-Session (so, wie Claude Code ihn unter Windows schreibt)
pub const CWD: &str = "D:\\Projekte\\demo";

/// Eine Zeile JSONL samt Zeilenumbruch
pub fn line(value: Value) -> String {
    format!("{value}\n")
}

/// Felder jeder Nachrichtenzeile
fn base(kind: &str, uuid: &str, at: &str, entrypoint: &str, branch: &str) -> Value {
    json!({
        "type": kind,
        "uuid": uuid,
        "timestamp": at,
        "sessionId": "s-demo",
        "entrypoint": entrypoint,
        "cwd": CWD,
        "gitBranch": branch,
        "version": "2.1.0",
        "isSidechain": false,
    })
}

/// Setzt Felder in einem Objekt
fn with(mut value: Value, fields: Value) -> Value {
    if let (Some(target), Some(source)) = (value.as_object_mut(), fields.as_object()) {
        for (key, field) in source {
            target.insert(key.clone(), field.clone());
        }
    }
    value
}

/// Nutzerzeile mit Inhalt
pub fn user(uuid: &str, content: Value, extra: Value) -> Value {
    with(
        with(
            base(
                "user",
                uuid,
                "2026-11-04T08:00:00.000Z",
                "claude-desktop",
                "main",
            ),
            json!({ "message": { "role": "user", "content": content } }),
        ),
        extra,
    )
}

/// Antwortzeile mit einem Block und den Tokens der Antwort `message_id`
pub fn assistant(uuid: &str, message_id: &str, block: Value, usage: [i64; 4]) -> Value {
    with(
        base(
            "assistant",
            uuid,
            "2026-11-04T08:01:00.000Z",
            "cli",
            "feature/demo",
        ),
        json!({
            "message": {
                "id": message_id,
                "model": "claude-demo",
                "role": "assistant",
                "content": [block],
                "usage": {
                    "input_tokens": usage[0],
                    "output_tokens": usage[1],
                    "cache_read_input_tokens": usage[2],
                    "cache_creation_input_tokens": usage[3],
                },
            },
        }),
    )
}

/// Aufruf eines Tools
pub fn tool_use(
    uuid: &str,
    message_id: &str,
    id: &str,
    name: &str,
    input: Value,
    usage: [i64; 4],
) -> Value {
    assistant(
        uuid,
        message_id,
        json!({ "type": "tool_use", "id": id, "name": name, "input": input }),
        usage,
    )
}

/// Ergebnis eines Tools samt `toolUseResult`
pub fn tool_result(uuid: &str, id: &str, content: Value, is_error: bool, details: Value) -> Value {
    user(
        uuid,
        json!([{ "type": "tool_result", "tool_use_id": id, "content": content, "is_error": is_error }]),
        json!({ "toolUseResult": details }),
    )
}

/// Pfad im Arbeitsordner
pub fn file(relative: &str) -> String {
    format!("{CWD}\\{}", relative.replace('/', "\\"))
}

/// Systemzeile
fn system(uuid: &str, fields: Value) -> Value {
    with(
        json!({ "type": "system", "uuid": uuid, "timestamp": "2026-11-04T08:02:00.000Z", "isSidechain": false }),
        fields,
    )
}

/// Das Beispiel-Transcript: alle relevanten Zeilentypen, eine kaputte Zeile, Tokens, Kosten,
/// Todos und zwei Dateiänderungen (Edit an `src/main.rs`, neue Datei `src/neu.rs`)
pub fn transcript() -> String {
    let lines = vec![
        json!({ "type": "custom-title", "customTitle": "Demo-Titel", "sessionId": "s-demo" }),
        user(
            "u-prompt",
            json!("Bitte <system-reminder>versteckt</system-reminder>füge eine Funktion hinzu"),
            json!({ "origin": { "kind": "human" } }),
        ),
        user(
            "u-meta",
            json!("<local-command-caveat>Hinweis</local-command-caveat>"),
            json!({ "isMeta": true }),
        ),
        user(
            "u-command",
            json!("<command-name>/model</command-name>\n<command-message>model</command-message>\n<command-args>opus</command-args>"),
            json!({}),
        ),
        user(
            "u-stdout",
            json!("<local-command-stdout>\u{1b}[1mModell gesetzt\u{1b}[22m</local-command-stdout>"),
            json!({}),
        ),
        with(
            assistant("a-think", "m1", json!({ "type": "thinking", "thinking": "", "signature": "x" }), [10, 5, 100, 50]),
            json!({ "thinkingDurationMs": 1500 }),
        ),
        assistant("a-text", "m1", json!({ "type": "text", "text": "Ich lese die Datei." }), [10, 5, 100, 50]),
        tool_use("a-read", "m1", "toolu_read", "Read", json!({ "file_path": file("src/main.rs") }), [10, 5, 100, 50]),
        tool_result(
            "u-read",
            "toolu_read",
            json!("1\tfn main() {}"),
            false,
            json!({ "type": "text", "file": { "filePath": file("src/main.rs") } }),
        ),
        tool_use(
            "a-edit",
            "m2",
            "toolu_edit",
            "Edit",
            json!({ "file_path": file("src/main.rs"), "old_string": "fn main() {}", "new_string": "fn main() {\n    hallo();\n}", "replace_all": false }),
            [2, 20, 150, 10],
        ),
        tool_result(
            "u-edit",
            "toolu_edit",
            json!("The file has been updated successfully."),
            false,
            json!({
                "filePath": file("src/main.rs"),
                "oldString": "fn main() {}",
                "newString": "fn main() {\n    hallo();\n}",
                "originalFile": "fn main() {}\n",
                "structuredPatch": [],
                "replaceAll": false,
            }),
        ),
        tool_use(
            "a-write",
            "m3",
            "toolu_write",
            "Write",
            json!({ "file_path": file("src/neu.rs"), "content": "pub fn hallo() {}\n" }),
            [3, 30, 160, 0],
        ),
        tool_result(
            "u-write",
            "toolu_write",
            json!("File created successfully"),
            false,
            json!({ "type": "create", "filePath": file("src/neu.rs"), "content": "pub fn hallo() {}\n", "structuredPatch": [], "originalFile": null }),
        ),
        tool_use("a-bash", "m4", "toolu_bash", "Bash", json!({ "command": "cargo test", "description": "Tests" }), [1, 1, 170, 0]),
        tool_result(
            "u-bash",
            "toolu_bash",
            json!([{ "type": "text", "text": "Exit code 1" }, { "type": "image", "source": {} }]),
            true,
            json!("Error: Exit code 1"),
        ),
        tool_use("a-grep", "m5", "toolu_grep", "Grep", json!({ "pattern": "hallo" }), [1, 1, 180, 0]),
        tool_result("u-grep", "toolu_grep", json!("x".repeat(25_000)), false, json!({})),
        with(
            assistant("a-side", "m-side", json!({ "type": "text", "text": "Subagent" }), [100, 100, 0, 0]),
            json!({ "isSidechain": true }),
        ),
        system("s-hook", json!({ "subtype": "stop_hook_summary", "hookErrors": ["Hook fehlgeschlagen"], "hookCount": 1 })),
        system("s-api", json!({ "subtype": "api_error", "level": "error", "error": { "formatted": "Überlastet" } })),
        system("s-compact", json!({ "subtype": "compact_boundary", "content": "Conversation compacted", "level": "info" })),
        system("s-turn", json!({ "subtype": "turn_duration", "durationMs": 1000 })),
    ];
    let more = vec![
        tool_use(
            "a-todo",
            "m6",
            "toolu_todo",
            "TodoWrite",
            json!({ "todos": [
                { "content": "Plan lesen", "status": "completed", "activeForm": "Lese Plan" },
                { "content": "Umsetzen", "status": "in_progress", "activeForm": "Setze um" },
            ] }),
            [1, 2, 190, 0],
        ),
        tool_result("u-todo", "toolu_todo", json!("Todos have been modified successfully."), false, json!({})),
        tool_use(
            "a-task",
            "m7",
            "toolu_task",
            "TaskCreate",
            json!({ "subject": "Tests schreiben", "description": "…", "activeForm": "Schreibe Tests" }),
            [1, 2, 200, 0],
        ),
        tool_result(
            "u-task",
            "toolu_task",
            json!("Task #7 created successfully: Tests schreiben"),
            false,
            json!({ "task": { "id": "7", "subject": "Tests schreiben" } }),
        ),
        tool_use("a-update", "m8", "toolu_update", "TaskUpdate", json!({ "taskId": "7", "status": "completed" }), [1, 2, 210, 0]),
        tool_result(
            "u-update",
            "toolu_update",
            json!("Updated task #7 status"),
            false,
            json!({ "success": true, "taskId": "7", "updatedFields": ["status"] }),
        ),
        user(
            "u-notification",
            json!("<task-notification>\n<task-id>b1</task-id>\n<summary>Hintergrundaufgabe fertig</summary>\n</task-notification>"),
            json!({ "origin": { "kind": "task-notification" } }),
        ),
        json!({
            "type": "cost-state",
            "totalCostUSD": 0.42,
            "totalDuration": 65_000,
            "totalLinesAdded": 4,
            "totalLinesRemoved": 1,
            "modelUsage": {
                "claude-demo": { "inputTokens": 100, "outputTokens": 200, "cacheReadInputTokens": 2000, "cacheCreationInputTokens": 250, "costUSD": 0.4 },
                "claude-klein": { "inputTokens": 20, "outputTokens": 3, "cacheReadInputTokens": 300, "cacheCreationInputTokens": 10, "costUSD": 0.02 },
            },
        }),
        assistant("a-final", "m9", json!({ "type": "text", "text": "Fertig." }), [5, 40, 1000, 200]),
    ];
    let mut text: String = lines.into_iter().map(line).collect();
    text.push_str("{kaputt\n");
    text.extend(more.into_iter().map(line));
    text
}

/// Das Beispiel ohne die Zeile `cost-state` (Tokens dann aus den Antworten)
pub fn transcript_without_costs() -> String {
    transcript()
        .lines()
        .filter(|line| !line.contains("\"cost-state\""))
        .map(|line| format!("{line}\n"))
        .collect()
}
