//! Liest einen Abschnitt des Transcripts ab einem Byte-Offset in Einträge des Verlaufs.
//!
//! - Nur vollständige Zeilen zählen; eine halbe letzte Zeile (Claude Code schreibt gerade)
//!   bleibt für den nächsten Abschnitt liegen.
//! - Kaputte Zeilen werden gezählt (`skipped`), unbekannte Zeilentypen übersprungen,
//!   Zeilen der Subagenten (`isSidechain`) ausgeblendet.
//! - Ergebnisse von Tools hängen am Tool-Eintrag. Fehlt das Ergebnis im Abschnitt, wird im
//!   Rest der Datei gesucht. Fehlt es dort auch (das Tool läuft noch), bleibt der Cursor vor
//!   dem Tool-Aufruf stehen: der nächste Abschnitt liefert die Einträge ab dort erneut, mit
//!   gleicher `id`, und den Tool-Eintrag samt Ergebnis, sobald es da ist. Ausnahme: ein vom
//!   Limit abgeschnittener Abschnitt, dessen erste Zeile der offene Aufruf ist – dort ginge
//!   es sonst nie weiter.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use super::jsonl::{self, flag, number, text, Line};
use super::tool_summary::summarize;
use super::types::{SystemLevel, ToolResult, TranscriptEntry};
use super::MAX_RESULT_CHARS;

/// Ergebnis des Lesens: Einträge, Offset für den nächsten Abschnitt, kaputte Zeilen
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptChunk {
    pub entries: Vec<TranscriptEntry>,
    pub cursor: u64,
    pub skipped: i64,
}

/// Liest den Abschnitt ab Offset `after` mit höchstens etwa `limit` Einträgen (eine Zeile
/// wird nie geteilt)
pub fn read_page(path: &Path, after: u64, limit: usize) -> std::io::Result<TranscriptChunk> {
    let bytes = jsonl::read_from(path, after)?;
    Ok(TranscriptChunk::parse(&bytes, after, limit))
}

impl TranscriptChunk {
    /// Liest `bytes`, die ab Offset `base` der Datei stehen
    pub fn parse(bytes: &[u8], base: u64, limit: usize) -> TranscriptChunk {
        let lines = jsonl::lines(bytes, base);
        let mut page = Page::default();
        let mut cut = false;
        let mut cursor = base;
        let mut next = lines.len();
        for (index, line) in lines.iter().enumerate() {
            if page.entries.len() >= limit.max(1) {
                cut = true;
                next = index;
                break;
            }
            page.feed(line);
            cursor = line.end;
        }
        if !cut {
            cursor = jsonl::complete_end(bytes, base);
        }
        // Ergebnisse noch offener Tools im Rest der Datei
        for line in &lines[next..] {
            if page.pending.is_empty() {
                break;
            }
            if !line.text.contains("tool_use_id") {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<Value>(line.text) {
                page.results(&value);
            }
        }
        if let Some(start) = page.pending.values().map(|(_, start)| *start).min() {
            if !cut || start > base {
                cursor = cursor.min(start);
            }
        }
        TranscriptChunk {
            entries: page.entries,
            cursor,
            skipped: page.skipped,
        }
    }
}

/// Zustand beim Lesen eines Abschnitts
#[derive(Default)]
struct Page {
    entries: Vec<TranscriptEntry>,
    skipped: i64,
    /// Offene Tool-Aufrufe: Kennung → (Index des Eintrags, Beginn der Zeile)
    pending: HashMap<String, (usize, u64)>,
}

impl Page {
    /// Verarbeitet eine Zeile
    fn feed(&mut self, line: &Line<'_>) {
        let value = match serde_json::from_str::<Value>(line.text) {
            Ok(value) if value.is_object() => value,
            _ => {
                self.skipped += 1;
                return;
            }
        };
        if flag(&value, "isSidechain") {
            return;
        }
        let at = text(&value, "timestamp").unwrap_or("").to_string();
        let id = text(&value, "uuid")
            .map(str::to_string)
            .unwrap_or_else(|| format!("line-{}", line.start));
        match text(&value, "type") {
            Some("user") => self.user(&value, id, at),
            Some("assistant") => self.assistant(&value, line.start, id, at),
            Some("system") => self.system(&value, id, at),
            _ => {}
        }
    }

    /// Prompt, Ergebnisse von Tools, Ausgaben lokaler Befehle
    fn user(&mut self, value: &Value, id: String, at: String) {
        if flag(value, "isMeta") {
            return;
        }
        let mut texts: Vec<String> = Vec::new();
        match value.pointer("/message/content") {
            Some(Value::String(content)) => texts.push(content.clone()),
            Some(Value::Array(parts)) => {
                for part in parts {
                    match text(part, "type") {
                        Some("tool_result") => self.result(part),
                        Some("text") => texts.push(text(part, "text").unwrap_or("").to_string()),
                        Some("image") => texts.push("[Bild]".to_string()),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        let joined = texts.join("\n");
        if joined.trim().is_empty() {
            return;
        }
        let origin = value.pointer("/origin/kind").and_then(Value::as_str);
        if let Some(entry) = prompt(id, at, &joined, origin) {
            self.entries.push(entry);
        }
    }

    /// Antworttext, Nachdenken und Tool-Aufrufe
    fn assistant(&mut self, value: &Value, start: u64, id: String, at: String) {
        let Some(blocks) = value.pointer("/message/content").and_then(Value::as_array) else {
            return;
        };
        let cwd = text(value, "cwd");
        for (index, block) in blocks.iter().enumerate() {
            let block_id = format!("{id}:{index}");
            match text(block, "type") {
                Some("text") => {
                    if let Some(content) = text(block, "text") {
                        self.entries.push(TranscriptEntry::Text {
                            id: block_id,
                            at: at.clone(),
                            text: content.to_string(),
                        });
                    }
                }
                Some("thinking") | Some("redacted_thinking") => {
                    self.entries.push(TranscriptEntry::Thinking {
                        id: block_id,
                        at: at.clone(),
                        text: block
                            .get("thinking")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        duration_ms: number(value, "thinkingDurationMs"),
                    });
                }
                Some("tool_use") | Some("server_tool_use") => {
                    let name = text(block, "name").unwrap_or("Tool").to_string();
                    let input = block.get("input").cloned().unwrap_or(Value::Null);
                    let tool_id = text(block, "id").map_or(block_id, str::to_string);
                    self.pending
                        .insert(tool_id.clone(), (self.entries.len(), start));
                    self.entries.push(TranscriptEntry::Tool {
                        id: tool_id,
                        at: at.clone(),
                        summary: summarize(&name, &input, cwd),
                        name,
                        input,
                        result: None,
                    });
                }
                _ => {}
            }
        }
    }

    /// Hinweise des Systems; die Dauer eines Zuges entfällt
    fn system(&mut self, value: &Value, id: String, at: String) {
        let content = text(value, "content");
        let (message, level) = match text(value, "subtype") {
            Some("turn_duration") => return,
            Some("stop_hook_summary") => {
                let errors: Vec<String> = value
                    .get("hookErrors")
                    .and_then(Value::as_array)
                    .map(|list| {
                        list.iter()
                            .map(|error| match error {
                                Value::String(text) => text.clone(),
                                other => other.to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if errors.is_empty() {
                    return;
                }
                (errors.join("\n"), SystemLevel::Warning)
            }
            Some("api_error") => (
                value
                    .pointer("/error/formatted")
                    .and_then(Value::as_str)
                    .or(content)
                    .unwrap_or("API-Fehler")
                    .to_string(),
                SystemLevel::Error,
            ),
            Some("compact_boundary") => (
                content.unwrap_or("Konversation kompaktiert").to_string(),
                SystemLevel::Info,
            ),
            _ => {
                let Some(content) = content else {
                    return;
                };
                (clean(content), level(text(value, "level")))
            }
        };
        if message.trim().is_empty() {
            return;
        }
        self.entries.push(TranscriptEntry::System {
            id,
            at,
            text: message,
            level,
        });
    }

    /// Ergebnisse von Tools in einer Zeile des Nutzers (für die Suche im Rest der Datei)
    fn results(&mut self, value: &Value) {
        if flag(value, "isSidechain") {
            return;
        }
        if let Some(parts) = value.pointer("/message/content").and_then(Value::as_array) {
            for part in parts {
                if text(part, "type") == Some("tool_result") {
                    self.result(part);
                }
            }
        }
    }

    /// Hängt ein Ergebnis an seinen Tool-Eintrag
    fn result(&mut self, part: &Value) {
        let Some(tool_id) = text(part, "tool_use_id") else {
            return;
        };
        let Some((index, _)) = self.pending.remove(tool_id) else {
            return;
        };
        if let Some(TranscriptEntry::Tool { result, .. }) = self.entries.get_mut(index) {
            *result = Some(tool_result(part));
        }
    }
}

/// Ergebnis eines Tools aus einem `tool_result`, gekürzt
pub fn tool_result(part: &Value) -> ToolResult {
    let full = match part.get("content") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match text(item, "type") {
                Some("text") => item.get("text").and_then(Value::as_str).map(str::to_string),
                Some("image") => Some("[Bild]".to_string()),
                Some("tool_reference") => text(item, "tool_name")
                    .or_else(|| text(item, "name"))
                    .map(str::to_string),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    };
    let truncated = full.chars().count() > MAX_RESULT_CHARS;
    ToolResult {
        text: if truncated {
            full.chars().take(MAX_RESULT_CHARS).collect()
        } else {
            full
        },
        is_error: flag(part, "is_error"),
        truncated,
    }
}

/// Prompt oder Systemeintrag aus dem Text einer Nutzerzeile; nichts bei leerem Text
fn prompt(id: String, at: String, raw: &str, origin: Option<&str>) -> Option<TranscriptEntry> {
    let content = reminders().replace_all(raw, "");
    let content = content.trim();
    if content.is_empty() || content.contains("<local-command-caveat>") {
        return None;
    }
    if let Some(output) =
        inner(content, "local-command-stdout").or_else(|| inner(content, "local-command-stderr"))
    {
        let output = clean(&output);
        return (!output.is_empty()).then_some(TranscriptEntry::System {
            id,
            at,
            text: output,
            level: SystemLevel::Info,
        });
    }
    if let Some(name) = inner(content, "command-name") {
        let name = name.trim();
        let slash = if name.starts_with('/') { "" } else { "/" };
        let args = inner(content, "command-args").unwrap_or_default();
        let text = format!("{slash}{name} {}", args.trim()).trim().to_string();
        return Some(TranscriptEntry::Prompt { id, at, text });
    }
    if origin.is_some_and(|kind| kind != "human") {
        let text = inner(content, "summary").unwrap_or_else(|| clean(content));
        return (!text.trim().is_empty()).then(|| TranscriptEntry::System {
            id,
            at,
            text: text.trim().to_string(),
            level: SystemLevel::Info,
        });
    }
    Some(TranscriptEntry::Prompt {
        id,
        at,
        text: content.to_string(),
    })
}

/// Gewicht laut Feld `level`
fn level(name: Option<&str>) -> SystemLevel {
    match name {
        Some("error") => SystemLevel::Error,
        Some("warning") | Some("warn") => SystemLevel::Warning,
        _ => SystemLevel::Info,
    }
}

/// Inhalt des ersten Elements `<tag>…</tag>`
fn inner(content: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = content.find(&open)? + open.len();
    let end = content[start..]
        .find(&close)
        .map_or(content.len(), |end| start + end);
    Some(content[start..end].to_string())
}

/// Ohne Tags und Farbcodes, getrimmt
fn clean(content: &str) -> String {
    let without_ansi = ansi().replace_all(content, "");
    tags().replace_all(&without_ansi, "").trim().to_string()
}

/// `<system-reminder>…</system-reminder>` samt Inhalt
fn reminders() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?s)<system-reminder>.*?</system-reminder>").unwrap())
}

/// Einfache Tags wie `<task-id>` oder `</status>`
fn tags() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"</?[A-Za-z][A-Za-z0-9_-]*>").unwrap())
}

/// Farbcodes des Terminals
fn ansi() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").unwrap())
}

#[cfg(test)]
mod tests {
    use super::super::fixtures;
    use super::*;

    fn kinds(entries: &[TranscriptEntry]) -> Vec<String> {
        entries
            .iter()
            .map(|entry| {
                serde_json::to_value(entry).unwrap()["kind"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    fn find<'a>(entries: &'a [TranscriptEntry], id: &str) -> &'a TranscriptEntry {
        entries
            .iter()
            .find(|entry| entry.id() == id)
            .unwrap_or_else(|| panic!("Eintrag {id} fehlt"))
    }

    #[test]
    fn liest_alle_zeilentypen_in_reihenfolge() {
        let content = fixtures::transcript();
        let page = TranscriptChunk::parse(content.as_bytes(), 0, 500);
        assert_eq!(page.skipped, 1, "die kaputte Zeile wird gezählt");
        assert_eq!(page.cursor, content.len() as u64);
        assert_eq!(
            kinds(&page.entries),
            vec![
                "prompt", "prompt", "system", "thinking", "text", "tool", "tool", "tool", "tool",
                "tool", "system", "system", "system", "tool", "tool", "tool", "system", "text"
            ]
        );
    }

    #[test]
    fn bereinigt_prompts_und_befehle() {
        let page = TranscriptChunk::parse(fixtures::transcript().as_bytes(), 0, 500);
        match find(&page.entries, "u-prompt") {
            TranscriptEntry::Prompt { text, at, .. } => {
                assert_eq!(text, "Bitte füge eine Funktion hinzu");
                assert_eq!(at, "2026-11-04T08:00:00.000Z");
            }
            other => panic!("kein Prompt: {other:?}"),
        }
        match find(&page.entries, "u-command") {
            TranscriptEntry::Prompt { text, .. } => assert_eq!(text, "/model opus"),
            other => panic!("kein Prompt: {other:?}"),
        }
        match find(&page.entries, "u-stdout") {
            TranscriptEntry::System { text, level, .. } => {
                assert_eq!(text, "Modell gesetzt");
                assert_eq!(*level, SystemLevel::Info);
            }
            other => panic!("kein Systemeintrag: {other:?}"),
        }
        match find(&page.entries, "u-notification") {
            TranscriptEntry::System { text, .. } => assert_eq!(text, "Hintergrundaufgabe fertig"),
            other => panic!("kein Systemeintrag: {other:?}"),
        }
    }

    #[test]
    fn haengt_ergebnisse_an_die_tools_und_kuerzt_lange() {
        let page = TranscriptChunk::parse(fixtures::transcript().as_bytes(), 0, 500);
        match find(&page.entries, "toolu_read") {
            TranscriptEntry::Tool {
                summary, result, ..
            } => {
                assert_eq!(summary, "Read src/main.rs");
                let result = result.as_ref().unwrap();
                assert_eq!(result.text, "1\tfn main() {}");
                assert!(!result.is_error);
            }
            other => panic!("kein Tool: {other:?}"),
        }
        match find(&page.entries, "toolu_bash") {
            TranscriptEntry::Tool { result, .. } => {
                let result = result.as_ref().unwrap();
                assert!(result.is_error);
                assert_eq!(result.text, "Exit code 1\n[Bild]");
            }
            other => panic!("kein Tool: {other:?}"),
        }
        match find(&page.entries, "toolu_grep") {
            TranscriptEntry::Tool { result, .. } => {
                let result = result.as_ref().unwrap();
                assert!(result.truncated);
                assert_eq!(result.text.chars().count(), MAX_RESULT_CHARS);
            }
            other => panic!("kein Tool: {other:?}"),
        }
    }

    #[test]
    fn nachdenken_systemhinweise_und_subagenten() {
        let page = TranscriptChunk::parse(fixtures::transcript().as_bytes(), 0, 500);
        match find(&page.entries, "a-think:0") {
            TranscriptEntry::Thinking {
                text, duration_ms, ..
            } => {
                assert_eq!(text, "");
                assert_eq!(*duration_ms, Some(1500));
            }
            other => panic!("kein Thinking: {other:?}"),
        }
        let levels: Vec<(String, SystemLevel)> = page
            .entries
            .iter()
            .filter_map(|entry| match entry {
                TranscriptEntry::System { text, level, .. } => Some((text.clone(), *level)),
                _ => None,
            })
            .collect();
        assert!(levels.contains(&("Hook fehlgeschlagen".into(), SystemLevel::Warning)));
        assert!(levels.contains(&("Überlastet".into(), SystemLevel::Error)));
        assert!(levels.contains(&("Conversation compacted".into(), SystemLevel::Info)));
        assert!(
            page.entries.iter().all(|entry| entry.id() != "a-side:0"),
            "Zeilen der Subagenten sind ausgeblendet"
        );
    }

    #[test]
    fn eine_halbe_letzte_zeile_wird_nicht_verbraucht() {
        let content = fixtures::transcript();
        let half = "{\"type\":\"assistant\",\"uuid\":\"a-neu\"";
        let bytes = format!("{content}{half}");
        let page = TranscriptChunk::parse(bytes.as_bytes(), 0, 500);
        assert_eq!(page.cursor, content.len() as u64);
        assert_eq!(page.skipped, 1);
    }

    #[test]
    fn liest_ab_dem_offset_in_abschnitten_weiter() {
        let content = fixtures::transcript();
        let all = TranscriptChunk::parse(content.as_bytes(), 0, 500).entries;
        let first = TranscriptChunk::parse(content.as_bytes(), 0, 4);
        assert_eq!(first.entries.len(), 4);
        let offset = first.cursor as usize;
        let second = TranscriptChunk::parse(&content.as_bytes()[offset..], first.cursor, 500);
        let mut joined = first.entries.clone();
        joined.extend(second.entries);
        assert_eq!(joined, all);
    }

    #[test]
    fn haelt_den_cursor_vor_einem_tool_ohne_ergebnis() {
        let start = fixtures::transcript();
        let open = fixtures::line(serde_json::json!({
            "type": "assistant", "uuid": "a-offen", "timestamp": "t", "cwd": fixtures::CWD,
            "message": { "id": "m-offen", "content": [
                { "type": "tool_use", "id": "toolu_offen", "name": "Bash", "input": { "command": "sleep 5" } }
            ] }
        }));
        let content = format!("{start}{open}");
        let page = TranscriptChunk::parse(content.as_bytes(), 0, 500);
        assert_eq!(page.cursor, start.len() as u64);
        let result = fixtures::line(serde_json::json!({
            "type": "user", "uuid": "u-offen", "timestamp": "t",
            "message": { "content": [
                { "type": "tool_result", "tool_use_id": "toolu_offen", "content": "fertig" }
            ] }
        }));
        let content = format!("{content}{result}");
        let next = TranscriptChunk::parse(
            &content.as_bytes()[page.cursor as usize..],
            page.cursor,
            500,
        );
        match find(&next.entries, "toolu_offen") {
            TranscriptEntry::Tool { result, .. } => {
                assert_eq!(result.as_ref().unwrap().text, "fertig")
            }
            other => panic!("kein Tool: {other:?}"),
        }
        assert_eq!(next.cursor, content.len() as u64);
    }

    #[test]
    fn liefert_ein_offenes_tool_einer_abgeschnittenen_seite_spaeter_mit_ergebnis_erneut() {
        let start = fixtures::transcript();
        let open = fixtures::line(serde_json::json!({
            "type": "assistant", "uuid": "a-offen", "timestamp": "t", "cwd": fixtures::CWD,
            "message": { "id": "m-offen", "content": [
                { "type": "tool_use", "id": "toolu_offen", "name": "Bash", "input": { "command": "sleep 5" } }
            ] }
        }));
        let more = fixtures::line(fixtures::assistant(
            "a-weiter",
            "m-weiter",
            serde_json::json!({ "type": "text", "text": "Warte." }),
            [0; 4],
        ));
        let content = format!("{start}{open}{more}");
        // Alle Einträge des Beispiels und der offene Aufruf; „Warte.“ fällt hinter das Limit
        let first = TranscriptChunk::parse(content.as_bytes(), 0, 19);
        assert_eq!(first.entries.last().unwrap().id(), "toolu_offen");
        assert_eq!(
            first.cursor,
            start.len() as u64,
            "Cursor bleibt vor dem offenen Aufruf"
        );
        let result = fixtures::line(serde_json::json!({
            "type": "user", "uuid": "u-offen", "timestamp": "t",
            "message": { "content": [
                { "type": "tool_result", "tool_use_id": "toolu_offen", "content": "fertig" }
            ] }
        }));
        let content = format!("{content}{result}");
        let next = TranscriptChunk::parse(
            &content.as_bytes()[first.cursor as usize..],
            first.cursor,
            500,
        );
        let ids: Vec<&str> = next.entries.iter().map(TranscriptEntry::id).collect();
        assert_eq!(ids, vec!["toolu_offen", "a-weiter:0"]);
        match &next.entries[0] {
            TranscriptEntry::Tool { result, .. } => {
                assert_eq!(result.as_ref().unwrap().text, "fertig")
            }
            other => panic!("kein Tool: {other:?}"),
        }
        assert_eq!(next.cursor, content.len() as u64);
    }

    #[test]
    fn ein_offener_aufruf_am_anfang_einer_abgeschnittenen_seite_haelt_nicht_auf() {
        let open = fixtures::line(serde_json::json!({
            "type": "assistant", "uuid": "a-offen", "timestamp": "t",
            "message": { "id": "m-offen", "content": [
                { "type": "tool_use", "id": "toolu_offen", "name": "Bash", "input": { "command": "x" } }
            ] }
        }));
        let more = fixtures::line(fixtures::assistant(
            "a-weiter",
            "m-weiter",
            serde_json::json!({ "type": "text", "text": "Warte." }),
            [0; 4],
        ));
        let content = format!("{open}{more}");
        let page = TranscriptChunk::parse(content.as_bytes(), 0, 1);
        assert_eq!(page.cursor, open.len() as u64);
    }

    #[test]
    fn sucht_ergebnisse_hinter_dem_limit() {
        let content = fixtures::transcript();
        // Bis einschließlich des ersten Tool-Aufrufs (Read); sein Ergebnis folgt danach
        let page = TranscriptChunk::parse(content.as_bytes(), 0, 6);
        match page.entries.last().unwrap() {
            TranscriptEntry::Tool { id, result, .. } => {
                assert_eq!(id, "toolu_read");
                assert!(result.is_some());
            }
            other => panic!("kein Tool: {other:?}"),
        }
    }

    #[test]
    fn liest_eine_datei_ab_offset() {
        let dir = crate::test_support::make_temp_dir("transcript-");
        let path = dir.path().join("s.jsonl");
        std::fs::write(&path, fixtures::transcript()).unwrap();
        let page = read_page(&path, 0, 500).unwrap();
        assert_eq!(page.entries.len(), 18);
        let rest = read_page(&path, page.cursor, 500).unwrap();
        assert!(rest.entries.is_empty());
        assert_eq!(rest.cursor, page.cursor);
        let behind = read_page(&path, page.cursor + 100, 500).unwrap();
        assert!(behind.entries.is_empty());
    }
}
