// Eine Instanz und nexus://-Links: Ein zweiter Start holt das vorhandene Fenster nach vorn,
// ein Link der Form nexus://ticket/ARC-0042 führt dort auf die Weiterleitungsseite der Oberfläche.
use tauri::{AppHandle, Runtime};
use tauri_plugin_deep_link::DeepLinkExt;

use crate::lifecycle;
use crate::server;

// Schema, unter dem die App bei den Betriebssystemen angemeldet ist
const SCHEMA: &str = "nexus://";
// Längste ID, die noch plausibel ist; schützt vor aufgeblähten Links
const ID_MAX: usize = 40;

// Wandelt einen nexus://-Link in eine Seitenadresse der Oberfläche um; None bei unbekannten Links.
// Die ID landet in einer Adresse, darum nur Großbuchstaben, Ziffern und Bindestriche durchlassen.
pub fn deep_link_path(url: &str) -> Option<String> {
    let rest = url.strip_prefix(SCHEMA)?.trim_end_matches('/');
    let (kind, id) = rest.split_once('/')?;
    if !matches!(kind, "ticket" | "question") {
        return None;
    }
    if id.is_empty() || id.len() > ID_MAX {
        return None;
    }
    if !id
        .chars()
        .all(|zeichen| zeichen.is_ascii_uppercase() || zeichen.is_ascii_digit() || zeichen == '-')
    {
        return None;
    }
    Some(format!("/open/{kind}/{id}"))
}

// Erster nexus://-Link in den Argumenten eines Starts; None, wenn keiner dabei ist
pub fn deep_link_in_args<S: AsRef<str>>(argv: &[S]) -> Option<String> {
    argv.iter().find_map(|arg| deep_link_path(arg.as_ref()))
}

// Zeigt das Hauptfenster, holt es nach vorn und folgt dabei einem Link, wenn einer dabei ist
pub fn focus_main<R: Runtime>(app: &AppHandle<R>, path: Option<String>) {
    lifecycle::fenster_zeigen(app);
    if let Some(path) = path {
        log::info!("Deep-Link öffnet {path}");
        server::open_path(app, &path);
    }
}

// Nimmt nexus://-Links an: einen, mit dem die App gerade gestartet wurde, und jeden weiteren im Lauf.
// Unter Windows und Linux meldet sich die App im Debug-Build selbst beim System an, damit Links
// auch ohne Installer ankommen.
pub fn setup<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(all(debug_assertions, any(windows, target_os = "linux")))]
    if let Err(error) = app.deep_link().register_all() {
        log::warn!("Das Schema nexus:// ließ sich nicht anmelden: {error}");
    }
    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        let urls: Vec<String> = event.urls().iter().map(|url| url.to_string()).collect();
        focus_main(&handle, deep_link_in_args(&urls));
    });
    // Kaltstart über einen Link: der Link steht schon beim Start bereit
    match app.deep_link().get_current() {
        Ok(Some(urls)) => {
            let urls: Vec<String> = urls.iter().map(|url| url.to_string()).collect();
            if let Some(path) = deep_link_in_args(&urls) {
                focus_main(app, Some(path));
            }
        }
        Ok(None) => {}
        Err(error) => log::warn!("Der Start-Link ist nicht lesbar: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_link() {
        assert_eq!(
            deep_link_path("nexus://ticket/ARC-0042").as_deref(),
            Some("/open/ticket/ARC-0042")
        );
    }

    #[test]
    fn frage_link() {
        assert_eq!(
            deep_link_path("nexus://question/ARC-F0001").as_deref(),
            Some("/open/question/ARC-F0001")
        );
    }

    #[test]
    fn unbekannte_art() {
        assert_eq!(deep_link_path("nexus://doc/ARC-0042"), None);
        assert_eq!(deep_link_path("nexus://ticket"), None);
        assert_eq!(deep_link_path("nexus://ticket/"), None);
    }

    #[test]
    fn fremdes_schema() {
        assert_eq!(deep_link_path("http://ticket/ARC-0042"), None);
        assert_eq!(deep_link_path("harness://ticket/ARC-0042"), None);
        assert_eq!(deep_link_path("/open/ticket/ARC-0042"), None);
    }

    #[test]
    fn id_mit_sonderzeichen_wird_abgelehnt() {
        for link in [
            "nexus://ticket/../etc/passwd",
            "nexus://ticket/ARC-0042?x=1",
            "nexus://ticket/arc-0042",
            "nexus://ticket/ARC 0042",
            "nexus://ticket/ARC-0042/mehr",
        ] {
            assert_eq!(deep_link_path(link), None, "{link} sollte abgelehnt werden");
        }
    }

    #[test]
    fn id_bleibt_kurz() {
        let lang = "A".repeat(ID_MAX + 1);
        assert_eq!(deep_link_path(&format!("nexus://ticket/{lang}")), None);
    }

    #[test]
    fn link_mit_schraegstrich_am_ende() {
        assert_eq!(
            deep_link_path("nexus://ticket/ARC-0042/").as_deref(),
            Some("/open/ticket/ARC-0042")
        );
    }

    // Ohne das Schema in der Konfiguration meldet der Installer nexus:// nicht beim Betriebssystem an
    #[test]
    fn konfiguration_meldet_das_schema_an() {
        let datei: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let schemes = datei["plugins"]["deep-link"]["desktop"]["schemes"]
            .as_array()
            .unwrap();
        assert_eq!(schemes, &[serde_json::json!("nexus")]);
    }

    #[test]
    fn link_aus_den_argumenten() {
        let argv = ["nexus-desktop", "--flag", "nexus://question/ARC-F0001"];
        assert_eq!(
            deep_link_in_args(&argv).as_deref(),
            Some("/open/question/ARC-F0001")
        );
        assert_eq!(deep_link_in_args(&["nexus-desktop"]), None);
    }
}
