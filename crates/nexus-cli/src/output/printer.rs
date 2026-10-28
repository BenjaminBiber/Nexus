//! Ergebnisse, Warnungen und Fehler ausgeben – als Text oder, mit `--json`, als JSON.
//! Portiert aus `packages/cli/src/output/Printer.ts`.

use serde::Serialize;

/// Gibt Ergebnisse als Text oder, mit `--json`, als JSON aus.
#[derive(Debug, Clone, Copy)]
pub struct Printer {
    json: bool,
}

impl Printer {
    /// Ausgabe für den gewählten Modus
    pub fn new(json: bool) -> Self {
        Self { json }
    }

    /// Erkennt `--json` an den rohen Argumenten, damit auch Parserfehler das Format kennen
    pub fn from_args<S: AsRef<str>>(args: &[S]) -> Self {
        Self::new(args.iter().any(|arg| arg.as_ref() == "--json"))
    }

    /// Gibt ein Ergebnis aus: JSON-Objekt im JSON-Modus, sonst die Textzeilen
    pub fn result(&self, data: &impl Serialize, lines: &[String]) {
        for line in self.result_lines(data, lines) {
            println!("{line}");
        }
    }

    /// Gibt eine Warnung auf stderr aus (auch im JSON-Modus, damit stdout sauber bleibt)
    pub fn warn(&self, message: &str) {
        eprintln!("Warnung: {message}");
    }

    /// Gibt einen Fehler aus: im JSON-Modus `{"error":{"code","message"}}` auf stdout, sonst `text` auf stderr
    pub fn failure(&self, code: &str, message: &str, text: &str) {
        match self.failure_json(code, message) {
            Some(json) => println!("{json}"),
            None => eprintln!("{text}"),
        }
    }

    /// Die Zeilen, die `result` schreiben würde (für Tests)
    pub fn result_lines(&self, data: &impl Serialize, lines: &[String]) -> Vec<String> {
        if self.json {
            vec![to_json(data)]
        } else {
            lines.to_vec()
        }
    }

    /// Der JSON-Text, den `failure` auf stdout schreiben würde; im Textmodus nichts (für Tests)
    pub fn failure_json(&self, code: &str, message: &str) -> Option<String> {
        self.json
            .then(|| to_json(&serde_json::json!({ "error": { "code": code, "message": message } })))
    }
}

/// JSON mit zwei Leerzeichen Einrückung wie `JSON.stringify(…, null, 2)`
fn to_json(data: &impl Serialize) -> String {
    serde_json::to_string_pretty(data).unwrap_or_else(|error| format!("{{\"error\":\"{error}\"}}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textmodus_gibt_die_zeilen_aus() {
        let printer = Printer::new(false);
        let zeilen = printer.result_lines(
            &serde_json::json!({ "id": "ARC-0001" }),
            &["ARC-0001 angelegt".to_string()],
        );
        assert_eq!(zeilen, vec!["ARC-0001 angelegt".to_string()]);
    }

    #[test]
    fn json_modus_gibt_das_objekt_mit_zwei_leerzeichen_aus() {
        let printer = Printer::new(true);
        let zeilen = printer.result_lines(&serde_json::json!({ "id": "ARC-0001" }), &[]);
        assert_eq!(zeilen, vec!["{\n  \"id\": \"ARC-0001\"\n}".to_string()]);
    }

    #[test]
    fn fehler_im_json_modus_nennt_code_und_meldung() {
        let printer = Printer::new(true);
        assert_eq!(
            printer.failure_json("conflict", "Bitte neu lesen."),
            Some("{\n  \"error\": {\n    \"code\": \"conflict\",\n    \"message\": \"Bitte neu lesen.\"\n  }\n}".to_string())
        );
    }

    #[test]
    fn fehler_im_textmodus_geht_nach_stderr() {
        assert_eq!(
            Printer::new(false).failure_json("conflict", "Bitte neu lesen."),
            None
        );
    }

    #[test]
    fn json_wird_an_den_rohen_argumenten_erkannt() {
        assert!(Printer::from_args(&["board", "list", "--json"])
            .failure_json("x", "y")
            .is_some());
        assert!(Printer::from_args(&["board", "list"])
            .failure_json("x", "y")
            .is_none());
    }
}
