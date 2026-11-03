//! Ersatz für Claude Code in Tests: liest die Anweisung von stdin und antwortet mit einem
//! JSON-Ergebnis. Portiert aus `packages/server/tests/helpers/FakeClaude.ts`; der Modus
//! steht in `FAKE_CLAUDE_MODE` (`ok`, `sleep`, `fail`, `stream`, `limit`).
//!
//! Dazu kommen Modi, für die die TypeScript-Tests ein eigenes Node-Skript geschrieben haben
//! und die hier im Ersatz stecken: `riesig` (überlange erste Zeile), `geteilt` (ein UTF-8-
//! Zeichen in zwei Blöcken), `entkommt` (eigene Prozessgruppe, der Abbruch des Baums greift
//! nicht) und `cwd` (schreibt den Arbeitsordner).
//!
//! Der Modus `antwort` liefert, was in `FAKE_CLAUDE_REPLY` steht: so geben Tests dem Ersatz
//! eine fertige Antwort mit, ohne die Umgebung des Testprozesses zu ändern. Zeigt
//! `FAKE_CLAUDE_REPLY_FILE` auf eine Datei, gilt deren Inhalt – damit ein Test die Antwort
//! zwischen zwei Läufen wechseln kann.
//!
//! Das Binary wird nicht ausgeliefert; Tests finden es über
//! `env!("CARGO_BIN_EXE_nexus-fake-claude")`.

use std::io::{Read, Write};

fn main() {
    let mut eingabe = String::new();
    let _ = std::io::stdin().read_to_string(&mut eingabe);
    let modus = std::env::var("FAKE_CLAUDE_MODE").unwrap_or_else(|_| "ok".to_string());
    let ticket = std::env::var("HARNESS_TICKET").unwrap_or_else(|_| "undefined".to_string());

    match modus.as_str() {
        // Läuft bis zum Abbruch weiter: für die Tests des Zeitlimits
        "sleep" => loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        },
        "fail" => {
            let _ = std::io::stderr().write_all(b"kaputt");
            std::process::exit(3);
        }
        "stream" => strom(&ticket),
        // Eine überlange erste Zeile, danach das Ergebnis: der Leser darf nur ganze Zeilen behalten
        "riesig" => {
            let mut stdout = std::io::stdout();
            let lang = format!(
                "{{\"type\":\"user\",\"x\":\"{}rate_limit{}\"}}\n",
                "a".repeat(150_000),
                "b".repeat(100_000)
            );
            let _ = stdout.write_all(lang.as_bytes());
            let _ = stdout.write_all(
                b"{\"type\":\"result\",\"subtype\":\"error_max_turns\",\"is_error\":true,\"result\":\"\"}\n",
            );
            let _ = stdout.flush();
            std::process::exit(1);
        }
        // Ein „ä“ in zwei Blöcken: der Leser muss die Bytes zusammensetzen
        "geteilt" => {
            let mut stdout = std::io::stdout();
            let _ = stdout.write_all(&[0xc3]);
            let _ = stdout.flush();
            std::thread::sleep(std::time::Duration::from_millis(200));
            let _ = stdout.write_all(&[0xa4]);
            let _ = stdout.flush();
        }
        // Eigene Prozessgruppe: killpg auf die Gruppe des Starters erreicht diesen Prozess
        // nicht mehr, nur das harte Beenden der Prozessnummer selbst hilft
        "entkommt" => {
            #[cfg(unix)]
            {
                let _ = nix::unistd::setsid();
            }
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        "cwd" => {
            let cwd = std::env::current_dir().unwrap_or_default();
            let _ = std::io::stdout().write_all(format!("cwd={}\n", cwd.display()).as_bytes());
        }
        _ => {
            let text = if modus == "antwort" {
                // Steht die Antwort in einer Datei, gilt ihr Inhalt: so wechselt ein Test sie
                // zwischen zwei Läufen, ohne den Dienst neu zu bauen
                match std::env::var("FAKE_CLAUDE_REPLY_FILE") {
                    Ok(pfad) => std::fs::read_to_string(pfad).unwrap_or_default(),
                    Err(_) => std::env::var("FAKE_CLAUDE_REPLY").unwrap_or_default(),
                }
            } else if modus == "limit" {
                "Claude usage limit reached".to_string()
            } else {
                let key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_else(|_| "-".to_string());
                let args: Vec<String> = std::env::args().skip(1).collect();
                format!(
                    "ticket={ticket} key={key} args={} prompt={eingabe}",
                    args.join(" ")
                )
            };
            let ergebnis = serde_json::json!({
                "type": "result",
                "subtype": "success",
                "is_error": false,
                "result": text,
            });
            let _ = std::io::stdout().write_all(ergebnis.to_string().as_bytes());
        }
    }
}

/// Wie `--output-format stream-json`: je Ereignis eine Zeile, hier absichtlich mitten in
/// der Zeile geteilt, damit die Leser beide Hälften zusammensetzen müssen.
fn strom(ticket: &str) {
    let ereignisse = [
        serde_json::json!({ "type": "system", "subtype": "init", "model": "fake-opus" }),
        serde_json::json!({
            "type": "assistant",
            "message": { "content": [
                { "type": "text", "text": "Ich lese das Ticket." },
                { "type": "tool_use", "name": "Bash", "input": { "command": format!("harness board show {ticket}") } },
            ] }
        }),
        serde_json::json!({
            "type": "user",
            "message": { "content": [{ "type": "tool_result", "is_error": true, "content": "abgelehnt" }] }
        }),
        serde_json::json!({
            "type": "result", "subtype": "success", "is_error": false, "num_turns": 3,
            "duration_ms": 60000, "result": "erledigt",
            "permission_denials": [{ "tool_name": "Bash", "tool_input": { "command": "git push" } }]
        }),
    ];
    let text: String = ereignisse
        .iter()
        .map(|ereignis| format!("{ereignis}\n"))
        .collect();
    let haelfte = text
        .char_indices()
        .nth(text.chars().count() / 2)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(&text.as_bytes()[..haelfte]);
    let _ = stdout.flush();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let _ = stdout.write_all(&text.as_bytes()[haelfte..]);
    let _ = stdout.flush();
}
