//! Vergleicht das Toast-XML aus Rust mit den Zeichenketten, die der TypeScript-Erbauer
//! `ToastXmlBuilder.ts` liefert – für alle fünf Arten, mit und ohne Hero-Bild.
//!
//! Die Golden-Datei entsteht aus dem TypeScript-Erbauer und liegt unter
//! `tests/fixtures/toast/toast-xml.golden.json`. Die Bildpfade werden vor dem Vergleich auf
//! Schrägstriche gebracht, weil `PathBuf::join` unter Windows einen Rückstrich einsetzt und
//! `path.join` in Node einen Schrägstrich.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nexus_server::notifications::model::{ToastKind, ToastModel, ToastOptions};
use nexus_server::notifications::toast_xml::ToastXmlBuilder;

/// Bildordner der Golden-Datei
const ASSETS_DIR: &str = "/a/toast";

/// Die Modelle der Golden-Datei, Art für Art
fn models() -> Vec<(&'static str, ToastModel)> {
    vec![
        (
            "bereit",
            ToastModel {
                kind: ToastKind::Bereit,
                title: "Session ist fertig".into(),
                line1: "webshop · T-0042 Favoriten".into(),
                line2: "Favoriten als Seitenfenster · Sonnet 5.5".into(),
                open_url: "http://127.0.0.1:7777/p/p1/board/t/T-0042".into(),
            },
        ),
        (
            "wartet",
            ToastModel {
                kind: ToastKind::Wartet,
                title: "Session braucht deine Antwort".into(),
                line1: "webshop · T-0042 A & B".into(),
                line2: "\"Favoriten\" <Seitenfenster>".into(),
                open_url: "http://127.0.0.1:7777/p/p1/board/t/T-0042".into(),
            },
        ),
        (
            "gesammelt",
            ToastModel {
                kind: ToastKind::Gesammelt,
                title: "3 Sessions warten auf dich".into(),
                line1: "1 braucht eine Antwort, 2 sind fertig".into(),
                line2: "T-1 · T-2 · T-3".into(),
                open_url: "http://127.0.0.1:7777/p/p1/sessions".into(),
            },
        ),
        (
            "frage-neu",
            ToastModel {
                kind: ToastKind::FrageNeu,
                title: "Neue Frage von einer Session".into(),
                line1: "ARC · Arches · ARC-F0001 Welcher Speicher?".into(),
                line2: "Anlass: ARC-0042 · noch keine Recherche".into(),
                open_url: "http://127.0.0.1:7777/p/p1/questions/ARC-F0001".into(),
            },
        ),
        (
            "recherche",
            ToastModel {
                kind: ToastKind::Recherche,
                title: "Recherche ist da".into(),
                line1: "ARC · Arches · ARC-F0001 Welcher Speicher?".into(),
                line2: "3 Optionen (O3 bis O5), O4 empfohlen".into(),
                open_url: "http://127.0.0.1:7777/p/p1/questions/ARC-F0001".into(),
            },
        ),
    ]
}

/// Die Golden-Zeichenketten aus TypeScript
fn golden() -> BTreeMap<String, String> {
    let file =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/toast/toast-xml.golden.json");
    let text = std::fs::read_to_string(&file).expect("Golden-Datei");
    serde_json::from_str(&text).expect("Golden-Datei als JSON")
}

#[test]
fn das_toast_xml_ist_wortgleich_zu_dem_aus_typescript() {
    let golden = golden();
    let builder = ToastXmlBuilder;
    let mut geprueft = 0;
    for (name, model) in models() {
        for hero_images in [false, true] {
            let key = match hero_images {
                true => format!("{name}-hero"),
                false => name.to_string(),
            };
            let xml = builder.build(
                &model,
                &ToastOptions {
                    assets_dir: PathBuf::from(ASSETS_DIR),
                    hero_images,
                },
            );
            let erwartet = golden.get(&key).unwrap_or_else(|| panic!("{key} fehlt"));
            assert_eq!(&xml.replace('\\', "/"), erwartet, "{key}");
            geprueft += 1;
        }
    }
    // Alle fünf Arten, je mit und ohne Hero
    assert_eq!(geprueft, 10);
    assert_eq!(golden.len(), 10);
}
