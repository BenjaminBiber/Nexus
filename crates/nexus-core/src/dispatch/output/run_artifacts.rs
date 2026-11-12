//! Liest aus der stream-json-Ausgabe von Claude Code, was neben dem Protokoll aufbewahrt wird:
//! die Session-ID (sie führt zum Transcript) und die Bilder aus Tool-Ergebnissen (Screenshots).
//! Das Protokoll selbst schreibt weiterhin der `StreamEventFormatter`.

use serde_json::Value;

/// Ein Fund in der Ausgabe eines Laufs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunArtifact {
    /// Die Session von Claude Code; je Lauf höchstens einmal gemeldet
    Session { session_id: String },
    /// Ein Bild aus einem Tool-Ergebnis; `data` ist Base64, wie geliefert
    Image {
        tool_use_id: String,
        media_type: String,
        data: String,
    },
}

/// Nimmt die Ausgabe stückweise entgegen und meldet Funde, sobald eine Zeile vollständig ist
#[derive(Debug, Default)]
pub struct RunArtifactScanner {
    pending: Vec<u8>,
    session_seen: bool,
}

impl RunArtifactScanner {
    /// Ein leerer Leser
    pub fn new() -> Self {
        Self::default()
    }

    /// Nimmt ein Stück der Ausgabe; liefert die Funde aller Zeilen, die damit vollständig sind
    pub fn push(&mut self, chunk: &[u8]) -> Vec<RunArtifact> {
        self.pending.extend_from_slice(chunk);
        let Some(end) = self.pending.iter().rposition(|byte| *byte == b'\n') else {
            return Vec::new();
        };
        let rest = self.pending.split_off(end + 1);
        let complete = std::mem::replace(&mut self.pending, rest);
        self.scan(&complete)
    }

    /// Wertet den Rest ohne Zeilenende aus (Ende der Ausgabe)
    pub fn flush(&mut self) -> Vec<RunArtifact> {
        let rest = std::mem::take(&mut self.pending);
        self.scan(&rest)
    }

    /// Geht die vollständigen Zeilen durch
    fn scan(&mut self, bytes: &[u8]) -> Vec<RunArtifact> {
        let text = String::from_utf8_lossy(bytes);
        let mut found = Vec::new();
        for line in text.lines() {
            self.scan_line(line.trim(), &mut found);
        }
        found
    }

    /// Liest eine Zeile nur dann als JSON, wenn sie eine Session-ID oder ein Bild enthalten kann:
    /// Zeilen mit Bildern sind groß, alle anderen bleiben ungelesen.
    fn scan_line(&mut self, line: &str, found: &mut Vec<RunArtifact>) {
        let wants_session = !self.session_seen && line.contains("\"session_id\"");
        let wants_image = line.contains("\"image\"");
        if !wants_session && !wants_image {
            return;
        }
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return;
        };
        if wants_session {
            let id = event.get("session_id").and_then(Value::as_str);
            if let Some(id) = id.filter(|id| !id.is_empty()) {
                self.session_seen = true;
                found.push(RunArtifact::Session {
                    session_id: id.to_string(),
                });
            }
        }
        if wants_image && event.get("type").and_then(Value::as_str) == Some("user") {
            images(&event, found);
        }
    }
}

/// Bilder aus den Tool-Ergebnissen eines `user`-Ereignisses
fn images(event: &Value, found: &mut Vec<RunArtifact>) {
    let Some(blocks) = event.pointer("/message/content").and_then(Value::as_array) else {
        return;
    };
    for block in blocks {
        if block.get("type").and_then(Value::as_str) != Some("tool_result") {
            continue;
        }
        let Some(tool_use_id) = block.get("tool_use_id").and_then(Value::as_str) else {
            continue;
        };
        let Some(parts) = block.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            if part.get("type").and_then(Value::as_str) != Some("image") {
                continue;
            }
            let source = &part["source"];
            if source.get("type").and_then(Value::as_str) != Some("base64") {
                continue;
            }
            let media_type = source.get("media_type").and_then(Value::as_str);
            let data = source.get("data").and_then(Value::as_str);
            if let (Some(media_type), Some(data)) = (media_type, data) {
                found.push(RunArtifact::Image {
                    tool_use_id: tool_use_id.to_string(),
                    media_type: media_type.to_string(),
                    data: data.to_string(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn line(event: serde_json::Value) -> String {
        format!("{event}\n")
    }

    fn image_event(tool_use_id: &str, data: &str) -> serde_json::Value {
        json!({ "type": "user", "session_id": "sitzung-1", "message": { "content": [{
            "type": "tool_result", "tool_use_id": tool_use_id,
            "content": [
                { "type": "text", "text": "Screenshot aufgenommen" },
                { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": data } }
            ]
        }] } })
    }

    fn image(tool_use_id: &str, data: &str) -> RunArtifact {
        RunArtifact::Image {
            tool_use_id: tool_use_id.into(),
            media_type: "image/png".into(),
            data: data.into(),
        }
    }

    #[test]
    fn meldet_die_session_genau_einmal() {
        let mut scanner = RunArtifactScanner::new();
        let first = scanner.push(
            line(json!({ "type": "system", "subtype": "init", "session_id": "sitzung-1" }))
                .as_bytes(),
        );
        let second = scanner.push(line(json!({ "type": "assistant", "session_id": "sitzung-1", "message": { "content": [] } })).as_bytes());
        assert_eq!(
            first,
            vec![RunArtifact::Session {
                session_id: "sitzung-1".into()
            }]
        );
        assert!(second.is_empty());
    }

    #[test]
    fn meldet_ein_bild_mit_der_id_des_aufrufs() {
        let mut scanner = RunArtifactScanner::new();
        scanner.push(
            line(json!({ "type": "system", "subtype": "init", "session_id": "sitzung-1" }))
                .as_bytes(),
        );
        let found = scanner.push(line(image_event("toolu_bild", "QUJD")).as_bytes());
        assert_eq!(found, vec![image("toolu_bild", "QUJD")]);
    }

    #[test]
    fn setzt_geteilte_zeilen_zusammen_und_wertet_den_rest_am_ende_aus() {
        let mut scanner = RunArtifactScanner::new();
        let text = line(image_event("toolu_a", "QUJD"));
        let (links, rechts) = text.split_at(text.len() / 2);
        assert!(scanner.push(links.as_bytes()).is_empty());
        assert_eq!(
            scanner.push(rechts.as_bytes()),
            vec![
                RunArtifact::Session {
                    session_id: "sitzung-1".into()
                },
                image("toolu_a", "QUJD")
            ]
        );
        // Letzte Zeile ohne Zeilenende: erst flush liefert sie
        let ohne_ende = image_event("toolu_b", "REVG").to_string();
        assert!(scanner.push(ohne_ende.as_bytes()).is_empty());
        assert_eq!(scanner.flush(), vec![image("toolu_b", "REVG")]);
        assert!(scanner.flush().is_empty());
    }

    #[test]
    fn uebergeht_was_kein_bild_eines_tool_ergebnisses_ist() {
        let bild = json!({ "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "QUJD" } });
        let mut scanner = RunArtifactScanner::new();
        let mut found = Vec::new();
        // Kein JSON, obwohl beide Stichworte vorkommen
        found.extend(scanner.push(b"kein JSON, aber \"image\" und \"session_id\"\n"));
        // Tool-Ergebnis als reiner Text
        found.extend(scanner.push(line(json!({ "type": "user", "message": { "content": [
            { "type": "tool_result", "tool_use_id": "t1", "content": "nur Text mit dem Wort \"image\"" }
        ] } })).as_bytes()));
        // Bild über eine Adresse statt Base64
        found.extend(scanner.push(line(json!({ "type": "user", "message": { "content": [
            { "type": "tool_result", "tool_use_id": "t2", "content": [{ "type": "image", "source": { "type": "url", "url": "https://example.org/a.png" } }] }
        ] } })).as_bytes()));
        // Bild in einer Antwort des Assistenten, nicht in einem Tool-Ergebnis
        found.extend(
            scanner.push(
                line(json!({ "type": "assistant", "message": { "content": [bild.clone()] } }))
                    .as_bytes(),
            ),
        );
        // Tool-Ergebnis ohne ID des Aufrufs
        found.extend(
            scanner.push(
                line(json!({ "type": "user", "message": { "content": [
            { "type": "tool_result", "content": [bild] }
        ] } }))
                .as_bytes(),
            ),
        );
        // Leere Session-ID
        found.extend(scanner.push(line(json!({ "type": "system", "session_id": "" })).as_bytes()));
        assert!(found.is_empty(), "{found:?}");
    }
}
