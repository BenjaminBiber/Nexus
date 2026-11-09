//! Mitgeliefertes Theme `themes/minecraft`: Manifest, Domain-Freigabe nur für Mojang, Paket installierbar.

use std::path::{Path, PathBuf};

use nexus_core::themes::{install_theme, pack_theme, parse_manifest};
use nexus_core::NexusHome;
use tempfile::TempDir;

/// Quelle des Themes im Repo
fn theme_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/minecraft")
}

#[test]
fn manifest_erlaubt_nur_mojang_domains() {
    let json = std::fs::read_to_string(theme_dir().join("manifest.json")).unwrap();
    let manifest = parse_manifest(&json).expect("Manifest gültig");
    assert_eq!(manifest.id, "minecraft");
    assert_eq!(manifest.slots, vec!["Shell".to_string(), "NavIcon".to_string()]);
    assert!(!manifest.permissions.network.is_empty());
    for domain in &manifest.permissions.network {
        assert!(
            domain.ends_with(".mojang.com") || domain == "resources.download.minecraft.net",
            "fremde Domain: {domain}"
        );
    }
    // Sounds liegen im Asset-Index, nicht im Client-JAR
    assert!(manifest.permissions.network.iter().any(|d| d == "resources.download.minecraft.net"));
}

#[test]
fn manifest_deklariert_den_schalter_sounds() {
    let json = std::fs::read_to_string(theme_dir().join("manifest.json")).unwrap();
    let manifest = parse_manifest(&json).expect("Manifest gültig");
    let schema = manifest.settings_schema.expect("settingsSchema");
    assert!(schema.contains_key("sounds"));
}

#[test]
fn gepacktes_paket_wird_installiert() {
    // Wie dist/ nach `npm run build`: Manifest und CSS aus der Quelle, main.js als Platzhalter
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    std::fs::create_dir_all(&dist).unwrap();
    for name in ["manifest.json", "theme.css"] {
        std::fs::copy(theme_dir().join(name), dist.join(name)).unwrap();
    }
    std::fs::write(dist.join("main.js"), "export default { id: 'minecraft', slots: {} };").unwrap();

    let packed = pack_theme(&dist).expect("packen");
    let home = NexusHome::at(tmp.path().join("home"));
    let manifest = install_theme(&home, &packed.bytes).expect("installieren");
    assert_eq!(manifest.id, "minecraft");
    assert!(home.themes_dir().join("minecraft/theme.css").is_file());
    assert!(home.themes_dir().join("minecraft/main.js").is_file());
}
