//! Golden-Test der Stream-Auswertung: die aufgezeichnete `stream-json`-Ausgabe aus
//! `tests/fixtures/dispatch/stream/roh.jsonl` wird durch `StreamEventFormatter` geschickt und
//! Byte für Byte mit `protokoll.txt` verglichen.
//!
//! Beide Dateien stammen aus dem TypeScript-Original (`fixtures/dispatch/stream/make.mts`,
//! einmal mit tsx erzeugt) und werden nie automatisch neu geschrieben: weicht etwas ab, ist der
//! Rust-Port falsch, nicht die Fixture. Das Protokollformat ist Vertrag – die Oberfläche liest
//! die Art in eckigen Klammern und den Tabulator vor dem Detail.

use std::fs::read_to_string;
use std::path::PathBuf;

use chrono::NaiveDate;
use nexus_core::dispatch::StreamEventFormatter;

/// Arbeitsordner und Uhrzeit wie in `make.mts`
const ROOT: &str = "/work/Nexus-worktrees/ARC-0001";

fn fixture(name: &str) -> String {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "fixtures",
        "dispatch",
        "stream",
        name,
    ]
    .iter()
    .collect();
    read_to_string(&path).unwrap_or_else(|err| panic!("{} nicht lesbar: {err}", path.display()))
}

/// Das Protokoll der rohen Ausgabe, in Blöcken von `chunk` Bytes eingespeist
fn protokoll(raw: &[u8], chunk: usize) -> String {
    let at = NaiveDate::from_ymd_opt(2026, 10, 29)
        .unwrap()
        .and_hms_opt(13, 45, 7)
        .unwrap();
    let mut formatter = StreamEventFormatter::with_root(Some(ROOT));
    let mut lines = Vec::new();
    for block in raw.chunks(chunk.max(1)) {
        lines.extend(formatter.push(block));
    }
    lines.extend(formatter.flush());
    lines
        .iter()
        .map(|line| StreamEventFormatter::stamped(line, at))
        .collect()
}

#[test]
fn formatiert_die_aufgezeichnete_ausgabe_byte_genau_wie_typescript() {
    let raw = fixture("roh.jsonl");
    let expected = fixture("protokoll.txt");
    assert_eq!(protokoll(raw.as_bytes(), raw.len()), expected);
}

#[test]
fn geteilte_eingabe_liefert_dasselbe_protokoll_wie_die_ungeteilte() {
    let raw = fixture("roh.jsonl");
    let expected = fixture("protokoll.txt");
    // 1 Byte teilt mitten in UTF-8-Zeichen, die anderen Größen mitten in Zeilen
    for chunk in [1, 2, 3, 7, 64, 333, 1901] {
        assert_eq!(
            protokoll(raw.as_bytes(), chunk),
            expected,
            "Blockgröße {chunk} liefert ein anderes Protokoll"
        );
    }
}
