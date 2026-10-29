//! Macht aus der Ausgabe von `claude -p --output-format stream-json` Protokollzeilen;
//! portiert aus `packages/core/src/dispatch/StreamEventFormatter.ts`.
//!
//! Jede Zeile hat eine Art, eine lesbare Kurzfassung und den genauen Aufruf; im Protokoll
//! steht sie als „HH:MM:SS  [art#ref] text<TAB>"detail"“, die Oberfläche zeigt daraus
//! wahlweise die kompakte oder die genaue Ansicht. Das Format legt Claude Code fest:
//! Unbekanntes fällt still heraus, Zeilen ohne JSON bleiben, wie sie sind.
//!
//! Eine Instanz hält den Rest einer unvollständigen Zeile und die Art der offenen Aufrufe,
//! daher je Ausgabestrom eine eigene. Der Rest bleibt als Bytes liegen, damit ein mitten im
//! UTF-8-Zeichen geteilter Block heil bleibt.

use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

use chrono::{NaiveDateTime, Timelike};
use regex::{Captures, Regex};
use serde_json::Value;

use super::shell_summary::{ShellCommandSummary, ShellKind};
use super::text::{clip, one_line};

/// Höchstens so viele Zeichen im genauen Aufruf (`detail`)
const DETAIL_MAX: usize = 4000;

/// Farbcodes der Testausgabe – die Steuerzeichen im Muster sind gewollt
static ANSI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*m").expect("festes Muster"));
/// Zählzeile von vitest/jest: „Tests  3 failed | 409 passed“
static TEST_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*Tests:?\s+(.+)$").expect("festes Muster"));
static FAILED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+) failed").expect("festes Muster"));
static PASSED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+) passed").expect("festes Muster"));
/// Laufwerksbuchstabe eines Windows-Pfads in Schrägstrich-Schreibweise
static DRIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([A-Za-z]):/(.*)$").expect("festes Muster"));

/// Art einer Protokollzeile: Start, Text von Claude, Werkzeugaufrufe (lesen, ändern, suchen,
/// testen, ausführen, Aufgabenliste, sonstiges), Ergebnis eines Aufrufs (ok/fail, über `ref`
/// zugeordnet), Ende des Laufs (done/abort); `Raw` = Zeile ohne JSON
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunLogLineKind {
    Start,
    Say,
    Read,
    Edit,
    Search,
    Test,
    Run,
    Todo,
    Tool,
    Ok,
    Fail,
    Done,
    Abort,
    Raw,
}

impl RunLogLineKind {
    /// Kennung in eckigen Klammern im Protokoll; die Oberfläche wertet sie aus
    pub fn as_str(&self) -> &'static str {
        match self {
            RunLogLineKind::Start => "start",
            RunLogLineKind::Say => "say",
            RunLogLineKind::Read => "read",
            RunLogLineKind::Edit => "edit",
            RunLogLineKind::Search => "search",
            RunLogLineKind::Test => "test",
            RunLogLineKind::Run => "run",
            RunLogLineKind::Todo => "todo",
            RunLogLineKind::Tool => "tool",
            RunLogLineKind::Ok => "ok",
            RunLogLineKind::Fail => "fail",
            RunLogLineKind::Done => "done",
            RunLogLineKind::Abort => "abort",
            RunLogLineKind::Raw => "raw",
        }
    }
}

impl fmt::Display for RunLogLineKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<ShellKind> for RunLogLineKind {
    fn from(kind: ShellKind) -> Self {
        match kind {
            ShellKind::Read => RunLogLineKind::Read,
            ShellKind::Edit => RunLogLineKind::Edit,
            ShellKind::Search => RunLogLineKind::Search,
            ShellKind::Test => RunLogLineKind::Test,
            ShellKind::Run => RunLogLineKind::Run,
        }
    }
}

/// Eine Zeile des Lauf-Protokolls: Kurzfassung `text`, dazu der genaue Aufruf bzw. die ganze
/// Meldung in `detail`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLogLine {
    pub kind: RunLogLineKind,
    pub text: String,
    /// ID des Werkzeugaufrufs (bei Aufrufen und ihren Ergebnissen)
    pub reference: Option<String>,
    pub detail: Option<String>,
}

impl RunLogLine {
    /// Eine Zeile nur mit Art und Kurzfassung
    fn plain_line(kind: RunLogLineKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            reference: None,
            detail: None,
        }
    }
}

/// Macht aus der Ausgabe von `claude -p --output-format stream-json` Protokollzeilen.
pub struct StreamEventFormatter {
    /// Rest eines Blocks bis zum nächsten Zeilenende (Bytes, damit geteilte Zeichen heil bleiben)
    pending: Vec<u8>,
    /// Art der offenen Werkzeugaufrufe, damit das Ergebnis sie zuordnen kann
    tools: HashMap<String, RunLogLineKind>,
    shell: ShellCommandSummary,
    /// Muster des Arbeitsordners; Pfade darin erscheinen in der Kurzfassung relativ
    root: Option<Regex>,
}

impl fmt::Debug for StreamEventFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamEventFormatter")
            .field("pending", &self.pending.len())
            .field("tools", &self.tools.len())
            .finish()
    }
}

impl Default for StreamEventFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamEventFormatter {
    /// Ohne Arbeitsordner: Pfade bleiben, wie Claude sie nennt
    pub fn new() -> Self {
        Self::with_root(None)
    }

    /// `root`: Arbeitsordner des Laufs; Pfade darin erscheinen in der Kurzfassung relativ
    pub fn with_root(root: Option<&str>) -> Self {
        let pattern = root.and_then(path_pattern);
        let strip = pattern.clone();
        Self {
            pending: Vec::new(),
            tools: HashMap::new(),
            shell: ShellCommandSummary::with_strip(Box::new(move |text| {
                relative(strip.as_ref(), text)
            })),
            root: pattern,
        }
    }

    /// Nimmt einen Block der Ausgabe und liefert die Einträge aller darin vollständigen Zeilen
    pub fn push(&mut self, chunk: &[u8]) -> Vec<RunLogLine> {
        self.pending.extend_from_slice(chunk);
        let Some(end) = self.pending.iter().rposition(|byte| *byte == b'\n') else {
            return Vec::new();
        };
        let complete = String::from_utf8_lossy(&self.pending[..end]).into_owned();
        self.pending.drain(..=end);
        complete
            .split('\n')
            .flat_map(|line| self.readable(line))
            .collect()
    }

    /// Liefert den Rest ohne Zeilenende (am Ende des Laufs)
    pub fn flush(&mut self) -> Vec<RunLogLine> {
        let rest = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        self.readable(&rest)
    }

    /// Ein ganzer Text als lesbare Zeilen (für Auswertungen und Kommentare nach dem Lauf)
    pub fn format(&mut self, text: &str) -> Vec<String> {
        text.split('\n')
            .flat_map(|line| self.readable(line))
            .flat_map(|entry| plain(&entry))
            .collect()
    }

    /// Eine Zeile als lesbare Zeilen; `None`, wenn sie kein JSON-Objekt ist (dann bleibt sie,
    /// wie sie ist)
    pub fn describe(&mut self, line: &str) -> Option<Vec<String>> {
        Some(
            self.entries(line)?
                .iter()
                .flat_map(plain)
                .collect::<Vec<String>>(),
        )
    }

    /// Eine Zeile als Einträge; `None`, wenn sie kein JSON-Objekt ist
    pub fn entries(&mut self, line: &str) -> Option<Vec<RunLogLine>> {
        let event = parse(line)?;
        let content = event
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
        let subtype = event.get("subtype").and_then(Value::as_str);
        match kind {
            "system" => Some(if subtype == Some("init") {
                let model = event
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or("Modell unbekannt");
                vec![RunLogLine::plain_line(
                    RunLogLineKind::Start,
                    format!("Claude startet ({model})"),
                )]
            } else {
                Vec::new()
            }),
            "assistant" => Some(
                content
                    .iter()
                    .flat_map(|block| self.assistant_block(block))
                    .collect(),
            ),
            "user" => Some(
                content
                    .iter()
                    .filter(|block| {
                        block.get("type").and_then(Value::as_str) == Some("tool_result")
                    })
                    .flat_map(|block| self.result(block))
                    .collect(),
            ),
            "result" => {
                let turns = match event.get("num_turns").and_then(Value::as_f64) {
                    Some(turns) => format!(", {} Schritte", js_number(turns)),
                    None => String::new(),
                };
                let minutes = match event.get("duration_ms").and_then(Value::as_f64) {
                    Some(duration) => format!(", {} min", js_round(duration / 60_000.0)),
                    None => String::new(),
                };
                let ok = subtype == Some("success");
                let head = if ok {
                    "fertig".to_string()
                } else {
                    format!("beendet ({})", subtype.unwrap_or("?"))
                };
                Some(vec![RunLogLine::plain_line(
                    if ok {
                        RunLogLineKind::Done
                    } else {
                        RunLogLineKind::Abort
                    },
                    format!("{head}{turns}{minutes}"),
                )])
            }
            _ => Some(Vec::new()),
        }
    }

    /// Ist die Zeile ein Ereignis von stream-json (JSON-Objekt mit `type`)?
    pub fn is_event(&self, line: &str) -> bool {
        parse(line).is_some_and(|event| event.get("type").and_then(Value::as_str).is_some())
    }

    /// Protokollzeile in Ortszeit: „HH:MM:SS  text“ für Zeilen ohne JSON, sonst
    /// „HH:MM:SS  [art#ref] text<TAB>"detail"“
    pub fn stamped(entry: &RunLogLine, at: NaiveDateTime) -> String {
        let time = format!("{:02}:{:02}:{:02}", at.hour(), at.minute(), at.second());
        if entry.kind == RunLogLineKind::Raw {
            return format!("{time}  {}\n", entry.text);
        }
        let reference = match &entry.reference {
            Some(reference) => format!("#{reference}"),
            None => String::new(),
        };
        let detail = match &entry.detail {
            // `JSON.stringify` eines Strings: Rust schreibt dieselbe Zeichenkette
            Some(detail) => format!("\t{}", Value::String(detail.clone())),
            None => String::new(),
        };
        format!(
            "{time}  [{}{reference}] {}{detail}\n",
            entry.kind, entry.text
        )
    }

    /// Protokollzeile eines Texts ohne JSON (stderr, eigene Meldungen)
    pub fn stamped_text(text: &str, at: NaiveDateTime) -> String {
        Self::stamped(&RunLogLine::plain_line(RunLogLineKind::Raw, text), at)
    }

    /// Ein Block eines Assistenten-Ereignisses: Text oder Werkzeugaufruf
    fn assistant_block(&mut self, block: &Value) -> Vec<RunLogLine> {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                let text = block.get("text").and_then(Value::as_str).unwrap_or("");
                if text.trim().is_empty() {
                    return Vec::new();
                }
                let short = one_line(text, 400);
                let detail = (short.chars().count() < text.trim().chars().count())
                    .then(|| clip(text.trim(), DETAIL_MAX, false));
                vec![RunLogLine {
                    kind: RunLogLineKind::Say,
                    text: short,
                    reference: None,
                    detail,
                }]
            }
            Some("tool_use") => {
                let name = block.get("name").and_then(Value::as_str).unwrap_or("?");
                let empty = Value::Object(serde_json::Map::new());
                let input = match block.get("input") {
                    Some(input) if input.is_object() => input,
                    _ => &empty,
                };
                let mut entry = self.tool(name, input);
                if let Some(reference) = block.get("id").and_then(Value::as_str) {
                    self.tools.insert(reference.to_string(), entry.kind);
                    entry.reference = Some(reference.to_string());
                }
                vec![entry]
            }
            _ => Vec::new(),
        }
    }

    /// Was ein Werkzeugaufruf tut: Kurzfassung und genauer Aufruf
    fn tool(&self, name: &str, input: &Value) -> RunLogLine {
        let path = input
            .get("file_path")
            .and_then(Value::as_str)
            .or_else(|| input.get("path").and_then(Value::as_str))
            .unwrap_or("");
        let file = self.relative(path);
        let with_detail = |kind: RunLogLineKind, text: String, detail: String| RunLogLine {
            kind,
            text,
            reference: None,
            detail: Some(detail),
        };
        match name {
            "Read" => with_detail(
                RunLogLineKind::Read,
                format!("liest {file}"),
                format!("Read {path}{}", range(input)),
            ),
            "Edit" | "MultiEdit" | "NotebookEdit" => with_detail(
                RunLogLineKind::Edit,
                format!("ändert {file}"),
                format!("{name} {path}"),
            ),
            "Write" => with_detail(
                RunLogLineKind::Edit,
                format!("schreibt {file}"),
                format!("Write {path}"),
            ),
            "Bash" | "PowerShell" => {
                let command = input
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let description = input.get("description").and_then(Value::as_str);
                let summary = self.shell.summarize(&command, description);
                with_detail(
                    summary.kind.into(),
                    summary.text,
                    format!(
                        "{} {}",
                        if name == "Bash" { "$" } else { "PS>" },
                        clip(&command, DETAIL_MAX, false)
                    ),
                )
            }
            "Grep" => {
                let pattern = input.get("pattern").and_then(Value::as_str).unwrap_or("");
                let glob = match input.get("glob").and_then(Value::as_str) {
                    Some(glob) => format!(" --glob {glob}"),
                    None => String::new(),
                };
                let where_ = if file.is_empty() {
                    String::new()
                } else {
                    format!(" in {file}")
                };
                let place = if path.is_empty() {
                    String::new()
                } else {
                    format!(" {path}")
                };
                with_detail(
                    RunLogLineKind::Search,
                    format!("sucht „{}“{where_}", one_line(pattern, 80)),
                    format!("Grep {}{place}{glob}", Value::String(pattern.to_string())),
                )
            }
            "Glob" => {
                let pattern = input.get("pattern").and_then(Value::as_str).unwrap_or("");
                let place = if path.is_empty() {
                    String::new()
                } else {
                    format!(" {path}")
                };
                with_detail(
                    RunLogLineKind::Search,
                    format!("sucht Dateien {}", one_line(pattern, 80)),
                    format!("Glob {pattern}{place}"),
                )
            }
            "TodoWrite" => {
                let todos = input
                    .get("todos")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                fn status(todo: &Value) -> Option<&str> {
                    todo.get("status").and_then(Value::as_str)
                }
                let done = todos
                    .iter()
                    .filter(|todo| status(todo) == Some("completed"))
                    .count();
                let active = todos
                    .iter()
                    .find(|todo| status(todo) == Some("in_progress"))
                    .and_then(|todo| todo.get("content").and_then(Value::as_str))
                    .filter(|content| !content.is_empty());
                let now = match active {
                    Some(content) => format!(" – jetzt: {}", one_line(content, 120)),
                    None => String::new(),
                };
                let detail = todos
                    .iter()
                    .map(|todo| {
                        let mark = match status(todo) {
                            Some("completed") => "[x]",
                            Some("in_progress") => "[>]",
                            _ => "[ ]",
                        };
                        format!(
                            "{mark} {}",
                            todo.get("content").and_then(Value::as_str).unwrap_or("")
                        )
                    })
                    .collect::<Vec<String>>()
                    .join("\n");
                with_detail(
                    RunLogLineKind::Todo,
                    format!("Aufgabenliste: {done}/{} erledigt{now}", todos.len()),
                    detail,
                )
            }
            _ => with_detail(
                RunLogLineKind::Tool,
                if file.is_empty() {
                    name.to_string()
                } else {
                    format!("{name} {file}")
                },
                format!("{name} {}", clip(&input.to_string(), DETAIL_MAX, false)),
            ),
        }
    }

    /// Ergebnis eines Aufrufs: Fehler mit Meldung, Tests mit Zählung, sonst ein leeres „ok“
    /// (färbt den Aufruf in der Oberfläche)
    fn result(&self, block: &Value) -> Vec<RunLogLine> {
        let reference = block
            .get("tool_use_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        let output = result_text(block.get("content"));
        let is_test = reference
            .as_ref()
            .and_then(|reference| self.tools.get(reference))
            .is_some_and(|kind| *kind == RunLogLineKind::Test);
        let tests = if is_test {
            test_count(&output)
        } else {
            String::new()
        };
        if block.get("is_error") == Some(&Value::Bool(true)) {
            return vec![RunLogLine {
                kind: RunLogLineKind::Fail,
                text: if tests.is_empty() {
                    one_line(&output, 300)
                } else {
                    tests
                },
                reference,
                detail: Some(clip(output.trim(), DETAIL_MAX, true)),
            }];
        }
        vec![RunLogLine {
            kind: RunLogLineKind::Ok,
            text: tests,
            reference,
            detail: None,
        }]
    }

    /// Einträge einer Zeile: leere fallen weg, Nicht-JSON bleibt als `Raw`
    fn readable(&mut self, raw: &str) -> Vec<RunLogLine> {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.trim().is_empty() {
            return Vec::new();
        }
        self.entries(line)
            .unwrap_or_else(|| vec![RunLogLine::plain_line(RunLogLineKind::Raw, line)])
    }

    /// Pfade im Arbeitsordner relativ; der Ordner selbst wird zu „.“
    fn relative(&self, text: &str) -> String {
        relative(self.root.as_ref(), text)
    }
}

/// Lesbare Zeilen als Text (Kommentare, Auswertungen): Ergebnisse ohne Text fallen weg
fn plain(entry: &RunLogLine) -> Vec<String> {
    match entry.kind {
        RunLogLineKind::Say => vec![format!("Claude: {}", entry.text)],
        RunLogLineKind::Fail => vec![format!("  Fehler: {}", entry.text)],
        RunLogLineKind::Ok => {
            if entry.text.is_empty() {
                Vec::new()
            } else {
                vec![format!("  {}", entry.text)]
            }
        }
        _ => vec![entry.text.clone()],
    }
}

/// Pfade im Arbeitsordner relativ; der Ordner selbst wird zu „.“
fn relative(root: Option<&Regex>, text: &str) -> String {
    match root {
        None => text.to_string(),
        Some(pattern) => pattern
            .replace_all(
                text,
                |found: &Captures| {
                    if found.get(1).is_some() {
                        ""
                    } else {
                        "."
                    }
                },
            )
            .into_owned(),
    }
}

/// Muster für den Ordner in allen Schreibweisen (C:\a\b, C:/a/b, /c/a/b), danach optional
/// ein Trenner
fn path_pattern(root: &str) -> Option<Regex> {
    let norm = root.replace('\\', "/");
    let norm = norm.trim_end_matches('/');
    if norm.is_empty() {
        return None;
    }
    // Jeder Trenner im Muster passt auf beide Schreibweisen
    let escape = |text: &str| regex::escape(text).replace('/', r"[\\/]");
    let head = match DRIVE.captures(norm) {
        Some(found) => format!(
            "(?:{}:[\\\\/]|/{}/){}",
            &found[1],
            &found[1],
            escape(&found[2])
        ),
        None => escape(norm),
    };
    Regex::new(&format!(r"(?i){head}([\\/])?")).ok()
}

/// JSON-Objekt der Zeile oder `None`
fn parse(line: &str) -> Option<serde_json::Map<String, Value>> {
    let text = line.trim();
    if !text.starts_with('{') {
        return None;
    }
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(object)) => Some(object),
        _ => None,
    }
}

/// Text eines Werkzeugergebnisses (Text oder Liste von Textblöcken)
fn result_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::Array(blocks)) => blocks
            .iter()
            .map(|block| block.get("text").and_then(Value::as_str).unwrap_or(""))
            .collect::<Vec<&str>>()
            .join("\n"),
        Some(Value::String(text)) => text.clone(),
        // `String(content ?? '')` in JavaScript: fehlt der Inhalt, bleibt der Text leer
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// Zählung aus der Ausgabe von vitest/jest („Tests  3 failed | 409 passed“), über mehrere
/// Pakete summiert; leer ohne Zählung
fn test_count(output: &str) -> String {
    let clean = ANSI.replace_all(output, "");
    let mut failed = 0_u64;
    let mut passed = 0_u64;
    let mut found = false;
    for line in TEST_LINE.captures_iter(&clean) {
        let count = |pattern: &Regex| {
            pattern
                .captures(&line[1])
                .and_then(|m| m[1].parse::<u64>().ok())
        };
        let (f, p) = (count(&FAILED), count(&PASSED));
        if f.is_none() && p.is_none() {
            continue;
        }
        found = true;
        failed += f.unwrap_or(0);
        passed += p.unwrap_or(0);
    }
    if !found {
        return String::new();
    }
    if failed > 0 {
        format!("{failed} fehlgeschlagen, {passed} bestanden")
    } else {
        format!("{passed} bestanden")
    }
}

/// „ (Zeilen a–b)“ für Read mit offset/limit
fn range(input: &Value) -> String {
    let offset = input.get("offset").and_then(Value::as_f64);
    let limit = input.get("limit").and_then(Value::as_f64);
    if offset.is_none() && limit.is_none() {
        return String::new();
    }
    let from = offset.unwrap_or(1.0);
    match limit {
        None => format!(" (ab Zeile {})", js_number(from)),
        Some(limit) => format!(
            " (Zeilen {}–{})",
            js_number(from),
            js_number(from + limit - 1.0)
        ),
    }
}

/// Eine Zahl so, wie JavaScript sie in einen String schreibt (ganze Zahlen ohne Nachkomma)
fn js_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e21 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// `Math.round` aus JavaScript (halbe Werte nach oben, auch bei negativen Zahlen)
fn js_round(value: f64) -> i64 {
    (value + 0.5).floor() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use serde_json::json;

    fn event(value: Value) -> String {
        value.to_string()
    }

    fn assistant(content: Value) -> String {
        event(json!({ "type": "assistant", "message": { "content": content } }))
    }

    fn user(content: Value) -> String {
        event(json!({ "type": "user", "message": { "content": content } }))
    }

    #[test]
    fn beschreibt_start_text_werkzeuge_fehler_und_ergebnis() {
        let mut f = StreamEventFormatter::new();
        assert_eq!(
            f.describe(&event(
                json!({ "type": "system", "subtype": "init", "model": "claude-opus" })
            )),
            Some(vec!["Claude startet (claude-opus)".to_string()])
        );
        assert_eq!(
            f.describe(&event(json!({ "type": "system", "subtype": "hook" }))),
            Some(Vec::new())
        );
        assert_eq!(
            f.describe(&assistant(
                json!([{ "type": "text", "text": "Ich lese\n  zuerst  das Ticket." }])
            )),
            Some(vec!["Claude: Ich lese zuerst das Ticket.".to_string()])
        );
        let long = f
            .describe(&assistant(
                json!([{ "type": "text", "text": "x".repeat(500) }]),
            ))
            .expect("JSON-Zeile");
        assert_eq!(long[0].chars().count(), "Claude: ".len() + 400);
        assert_eq!(
            f.describe(&assistant(json!([
                { "type": "tool_use", "name": "Read", "input": { "file_path": "src/a.ts" } },
                { "type": "tool_use", "name": "Edit", "input": { "file_path": "src/b.ts" } },
                { "type": "tool_use", "name": "Write", "input": { "file_path": "src/c.ts" } },
                { "type": "tool_use", "name": "Bash", "input": { "command": "npm test" } },
                { "type": "tool_use", "name": "Grep", "input": { "pattern": "TODO", "path": "src" } },
                { "type": "tool_use", "name": "Glob", "input": { "pattern": "**/*.ts" } },
                { "type": "tool_use", "name": "WebFetch", "input": { "url": "x" } },
                { "type": "thinking", "thinking": "still" },
            ]))),
            Some(vec![
                "liest src/a.ts".to_string(),
                "ändert src/b.ts".to_string(),
                "schreibt src/c.ts".to_string(),
                "Tests (alle)".to_string(),
                "sucht „TODO“ in src".to_string(),
                "sucht Dateien **/*.ts".to_string(),
                "WebFetch".to_string(),
            ])
        );
        assert_eq!(
            f.describe(&user(json!([
                { "type": "tool_result", "is_error": true, "content": "Datei fehlt" },
                { "type": "tool_result", "is_error": true, "content": [{ "type": "text", "text": "Befehl" }, { "type": "text", "text": "abgelehnt" }] },
                { "type": "tool_result", "content": "ok" },
            ]))),
            Some(vec![
                "  Fehler: Datei fehlt".to_string(),
                "  Fehler: Befehl abgelehnt".to_string(),
            ])
        );
        assert_eq!(
            f.describe(&event(json!({
                "type": "result", "subtype": "success", "num_turns": 12, "duration_ms": 185_000
            }))),
            Some(vec!["fertig, 12 Schritte, 3 min".to_string()])
        );
        assert_eq!(
            f.describe(&event(
                json!({ "type": "result", "subtype": "error_max_turns" })
            )),
            Some(vec!["beendet (error_max_turns)".to_string()])
        );
    }

    #[test]
    fn liefert_eintrage_mit_art_kurzfassung_genauem_aufruf_und_zugeordnetem_ergebnis() {
        let mut g = StreamEventFormatter::with_root(Some("C:/wt/AIH-1"));
        assert_eq!(
            g.entries(&assistant(json!([{
                "type": "tool_use", "id": "t1", "name": "Bash",
                "input": {
                    "command": "cd /c/wt/AIH-1/packages/web && npx vitest run tests/A.test.tsx",
                    "description": "Web-Tests",
                },
            }]))),
            Some(vec![RunLogLine {
                kind: RunLogLineKind::Test,
                text: "Web-Tests".to_string(),
                reference: Some("t1".to_string()),
                detail: Some(
                    "$ cd /c/wt/AIH-1/packages/web && npx vitest run tests/A.test.tsx".to_string()
                ),
            }])
        );
        assert_eq!(
            g.entries(&user(json!([{
                "type": "tool_result", "tool_use_id": "t1", "is_error": true,
                "content": "Test Files  1 failed\n      Tests  2 failed | 40 passed (42)\nExit code 1",
            }]))),
            Some(vec![RunLogLine {
                kind: RunLogLineKind::Fail,
                text: "2 fehlgeschlagen, 40 bestanden".to_string(),
                reference: Some("t1".to_string()),
                detail: Some(
                    "Test Files  1 failed\n      Tests  2 failed | 40 passed (42)\nExit code 1"
                        .to_string()
                ),
            }])
        );
        assert_eq!(
            g.entries(&assistant(json!([{
                "type": "tool_use", "id": "t2", "name": "Read",
                "input": { "file_path": r"C:\wt\AIH-1\src\a.ts", "offset": 10, "limit": 5 },
            }]))),
            Some(vec![RunLogLine {
                kind: RunLogLineKind::Read,
                text: r"liest src\a.ts".to_string(),
                reference: Some("t2".to_string()),
                detail: Some(r"Read C:\wt\AIH-1\src\a.ts (Zeilen 10–14)".to_string()),
            }])
        );
        assert_eq!(
            g.entries(&user(
                json!([{ "type": "tool_result", "tool_use_id": "t2", "content": "inhalt" }])
            )),
            Some(vec![RunLogLine {
                kind: RunLogLineKind::Ok,
                text: String::new(),
                reference: Some("t2".to_string()),
                detail: None,
            }])
        );
    }

    #[test]
    fn lasst_unbekannte_ereignisse_still_weg_und_ubernimmt_nicht_json_unverandert() {
        let mut f = StreamEventFormatter::new();
        assert_eq!(
            f.describe(&event(json!({ "type": "stream_event" }))),
            Some(Vec::new())
        );
        assert_eq!(f.describe("Fehler: kein Netz"), None);
        assert_eq!(f.describe("{kaputt"), None);
        let text = format!(
            "Hinweis\n{}\n{}\n",
            event(json!({ "type": "rate_limit_event" })),
            assistant(json!([{ "type": "text", "text": "Hallo" }]))
        );
        assert_eq!(
            f.format(&text),
            vec!["Hinweis".to_string(), "Claude: Hallo".to_string()]
        );
        assert!(f.is_event(&event(json!({ "type": "assistant" }))));
        assert!(!f.is_event(r#"{"a":1}"#));
        assert!(!f.is_event("Text"));
    }

    #[test]
    fn setzt_teilzeilen_uber_blocke_hinweg_zusammen() {
        let mut lines = StreamEventFormatter::new();
        let text = format!(
            "{}\r\n{}\nRest",
            assistant(json!([{ "type": "text", "text": "Hallo" }])),
            assistant(
                json!([{ "type": "tool_use", "name": "Bash", "input": { "command": "ls" } }])
            )
        );
        let bytes = text.as_bytes();
        let mut out: Vec<String> = Vec::new();
        for chunk in bytes.chunks(7) {
            out.extend(lines.push(chunk).into_iter().map(|entry| entry.text));
        }
        assert_eq!(out, vec!["Hallo".to_string(), "prüft Dateien".to_string()]);
        assert_eq!(
            lines.flush(),
            vec![RunLogLine::plain_line(RunLogLineKind::Raw, "Rest")]
        );
        assert_eq!(lines.flush(), Vec::new());
    }

    /// Eine mitten in einem UTF-8-Zeichen geteilte Eingabe liefert dieselben Zeilen wie die
    /// ungeteilte – der Rest bleibt deshalb als Bytes liegen
    #[test]
    fn geteilte_utf8_zeichen_bleiben_heil() {
        let text = format!(
            "{}\n{}\n",
            assistant(
                json!([{ "type": "text", "text": "Ändert größere Dateien – übrigens „schön“" }])
            ),
            assistant(json!([{ "type": "text", "text": "Zweite Zeile mit ☃ und 🧊" }]))
        );
        let whole = {
            let mut formatter = StreamEventFormatter::new();
            let mut lines = formatter.push(text.as_bytes());
            lines.extend(formatter.flush());
            lines
        };
        for size in 1..=9 {
            let mut formatter = StreamEventFormatter::new();
            let mut lines: Vec<RunLogLine> = Vec::new();
            for chunk in text.as_bytes().chunks(size) {
                lines.extend(formatter.push(chunk));
            }
            lines.extend(formatter.flush());
            assert_eq!(lines, whole, "Blockgröße {size}");
        }
    }

    #[test]
    fn stempelt_protokollzeilen_mit_uhrzeit_art_und_genauem_aufruf() {
        let at = NaiveDate::from_ymd_opt(2026, 10, 16)
            .expect("gültiges Datum")
            .and_hms_opt(9, 5, 7)
            .expect("gültige Uhrzeit");
        assert_eq!(
            StreamEventFormatter::stamped_text("Claude: Hallo", at),
            "09:05:07  Claude: Hallo\n"
        );
        assert_eq!(
            StreamEventFormatter::stamped(
                &RunLogLine::plain_line(RunLogLineKind::Raw, "stderr"),
                at
            ),
            "09:05:07  stderr\n"
        );
        assert_eq!(
            StreamEventFormatter::stamped(
                &RunLogLine {
                    kind: RunLogLineKind::Run,
                    text: "git status".to_string(),
                    reference: Some("t1".to_string()),
                    detail: Some("$ git status\n".to_string()),
                },
                at
            ),
            "09:05:07  [run#t1] git status\t\"$ git status\\n\"\n"
        );
        assert_eq!(
            StreamEventFormatter::stamped(
                &RunLogLine {
                    kind: RunLogLineKind::Ok,
                    text: String::new(),
                    reference: Some("t1".to_string()),
                    detail: None,
                },
                at
            ),
            "09:05:07  [ok#t1] \n"
        );
    }

    #[test]
    fn kurzt_pfade_auf_den_arbeitsordner_und_nennt_ihn_selbst_punkt() {
        let formatter = StreamEventFormatter::with_root(Some("/wt/AIH-1/"));
        assert_eq!(formatter.relative("/wt/AIH-1/src/a.ts"), "src/a.ts");
        assert_eq!(formatter.relative("/wt/AIH-1"), ".");
        assert_eq!(
            StreamEventFormatter::new().relative("/wt/AIH-1"),
            "/wt/AIH-1"
        );
    }

    #[test]
    fn zahlt_testergebnisse_uber_mehrere_pakete_und_ohne_farbcodes() {
        assert_eq!(
            test_count("Tests  1 failed | 2 passed\n\u{1b}[32mTests  5 passed\u{1b}[0m"),
            "1 fehlgeschlagen, 7 bestanden"
        );
        assert_eq!(test_count("Tests  5 passed"), "5 bestanden");
        assert_eq!(test_count("keine Zählung"), "");
        assert_eq!(test_count("Tests  no tests"), "");
    }

    #[test]
    fn nennt_die_aufgabenliste_mit_stand_und_aktueller_aufgabe() {
        let formatter = StreamEventFormatter::new();
        let entry = formatter.tool(
            "TodoWrite",
            &json!({ "todos": [
                { "status": "completed", "content": "lesen" },
                { "status": "in_progress", "content": "schreiben" },
                { "status": "pending", "content": "testen" },
            ] }),
        );
        assert_eq!(entry.kind, RunLogLineKind::Todo);
        assert_eq!(entry.text, "Aufgabenliste: 1/3 erledigt – jetzt: schreiben");
        assert_eq!(
            entry.detail.as_deref(),
            Some("[x] lesen\n[>] schreiben\n[ ] testen")
        );
    }

    #[test]
    fn nennt_bei_read_den_gelesenen_bereich() {
        let formatter = StreamEventFormatter::new();
        assert_eq!(
            formatter
                .tool("Read", &json!({ "file_path": "a.ts", "offset": 10 }))
                .detail
                .as_deref(),
            Some("Read a.ts (ab Zeile 10)")
        );
        assert_eq!(
            formatter
                .tool("Read", &json!({ "file_path": "a.ts", "limit": 5 }))
                .detail
                .as_deref(),
            Some("Read a.ts (Zeilen 1–5)")
        );
    }
}
