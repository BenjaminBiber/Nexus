// Erzeugt zur Bauzeit den Tauri-Kontext aus tauri.conf.json.
// Die eigenen Befehle stehen im App-Manifest; damit darf die Oberfläche nur die, die
// capabilities/ui.json ausdrücklich freigibt.
fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["meldung_zeigen"])),
    )
    .expect("Der Tauri-Kontext ließ sich nicht erzeugen");
}
