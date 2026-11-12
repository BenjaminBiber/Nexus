//! Der Referenz-Index der mitgelieferten Themes (erzeugt von scripts/release/theme-index.mjs) besteht
//! die Prüfung des Stores vollständig: nichts ausgelassen, alles zur aktuellen SDK-Version passend.

use nexus_core::theme::{parse_index, store_index::theme_sdk_version};

#[test]
fn referenz_index_der_mitgelieferten_themes_ist_gueltig_und_kompatibel() {
    let pfad = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/release/tests/fixtures/nexus-themes.reference.json"
    );
    let bytes = std::fs::read(pfad).expect("Referenz-Index lesbar");
    let index = parse_index(&bytes, &theme_sdk_version()).expect("Index gültig");
    assert!(index.skipped.is_empty(), "{:?}", index.skipped);
    let ids: Vec<_> = index.entries.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "alucard",
            "catppuccin",
            "dracula",
            "gta",
            "minecraft",
            "pastel",
            "signal",
            "synthwave",
            "terminal",
            "win95"
        ]
    );
    for eintrag in &index.entries {
        assert!(eintrag.compatible, "{} passt nicht zum SDK", eintrag.id);
    }
}
