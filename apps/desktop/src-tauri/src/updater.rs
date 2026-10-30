// Selbstaktualisierung: 30 s nach dem Start und dann täglich `latest.json` des neuesten Releases
// prüfen, nach Rückfrage das signierte Update installieren und neu starten.
//
// Alles läuft in Rust, darum braucht die Oberfläche keine Rechte für den Updater: die Einträge
// `updater:default` und `process:allow-restart` in den Capabilities gelten nur für Aufrufe aus dem
// Fenster. Den Neustart macht `AppHandle::restart` selbst, dafür ist kein weiteres Plugin nötig.
//
// Die reine Logik (soll geprüft werden, schon abgelehnt, Text der Rückfrage) steht in Funktionen
// ohne Tauri-Typen, damit sie testbar bleibt.
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

use crate::lifecycle;

// So lange bleibt der Start in Ruhe, bevor die erste Prüfung läuft
pub const START_VERZUG: Duration = Duration::from_secs(30);
// Abstand der weiteren Prüfungen
pub const INTERVALL: Duration = Duration::from_secs(24 * 60 * 60);
// Der Dispatch-Status soll den Dialog nicht aufhalten
const STATUS_TIMEOUT: Duration = Duration::from_secs(2);

// Die Version, die der Benutzer abgelehnt hat. Steht nur im Speicher: nach einem Neustart der App
// fragt sie wieder nach.
#[derive(Default)]
pub struct Abgelehnt {
    version: Mutex<Option<String>>,
}

// Soll die App überhaupt nach Updates sehen? Der Debug-Build nie, damit die Entwicklung nicht
// plötzlich ein Release installiert. Unter Linux nur das AppImage – deb und rpm gehören der
// Paketverwaltung, die sie selbst aktualisiert.
pub fn soll_pruefen(debug_build: bool, linux: bool, appimage: bool) -> bool {
    !debug_build && (!linux || appimage)
}

// Hat der Benutzer genau diese Version schon abgelehnt?
pub fn schon_abgelehnt(gemerkt: Option<&str>, version: &str) -> bool {
    gemerkt == Some(version)
}

// Text der Rückfrage. Laufen Dispatch-Läufe, sagt der Text, was der Neustart mit ihnen macht:
// die Läufe selbst laufen in eigenen Prozessen weiter, nur das Auswerten hängt am Server.
pub fn frage(neu: &str, alt: &str, laeufe: u32) -> String {
    let anfang = format!("Nexus {neu} ist verfügbar (installiert: {alt}).");
    let hinweis = match laeufe {
        0 => String::new(),
        1 => " Es läuft gerade 1 Dispatch-Lauf; er läuft in einem eigenen Prozess weiter, sein \
              Ergebnis wird aber erst nach dem Neustart ausgewertet."
            .to_string(),
        _ => format!(
            " Es laufen gerade {laeufe} Dispatch-Läufe; sie laufen in eigenen Prozessen weiter, \
             ihre Ergebnisse werden aber erst nach dem Neustart ausgewertet."
        ),
    };
    format!("{anfang}{hinweis} Jetzt installieren und neu starten?")
}

// Richtet die Prüfung ein. Wo nicht geprüft werden soll, passiert nichts weiter.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    app.manage(Abgelehnt::default());
    if !soll_pruefen(
        cfg!(debug_assertions),
        cfg!(target_os = "linux"),
        std::env::var_os("APPIMAGE").is_some(),
    ) {
        log::info!(
            "Keine Update-Prüfung: Debug-Build oder eine Installation, die die Paketverwaltung aktualisiert."
        );
        return;
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(START_VERZUG).await;
        // Der erste Takt kommt sofort, die weiteren im Abstand von INTERVALL
        let mut takt = tokio::time::interval(INTERVALL);
        loop {
            takt.tick().await;
            pruefen(&handle).await;
        }
    });
}

// Eine Prüfung: Update suchen, nachfragen, installieren, neu starten.
// Fehler (offline, 404 vor dem ersten Release) gehen nur ins Protokoll.
async fn pruefen<R: Runtime>(app: &AppHandle<R>) {
    let update = match app.updater() {
        Ok(updater) => match updater.check().await {
            Ok(Some(update)) => update,
            Ok(None) => {
                log::info!("Nexus ist aktuell.");
                return;
            }
            Err(error) => {
                log::info!("Nach Updates konnte nicht gesehen werden: {error}");
                return;
            }
        },
        Err(error) => {
            log::warn!("Der Updater ließ sich nicht einrichten: {error}");
            return;
        }
    };
    if abgelehnt(app, &update.version) {
        log::info!("Update auf {} bleibt abgelehnt.", update.version);
        return;
    }
    let text = frage(
        &update.version,
        &update.current_version,
        laufende_laeufe().await,
    );
    if !fragen(app, &text).await {
        merken(app, &update.version);
        log::info!("Update auf {} abgelehnt.", update.version);
        return;
    }
    if let Err(error) = update.download_and_install(|_, _| {}, || {}).await {
        log::warn!("Das Update ließ sich nicht installieren: {error}");
        return;
    }
    log::info!(
        "Update auf {} installiert, Nexus startet neu.",
        update.version
    );
    app.restart();
}

// Stellt die Rückfrage und wartet auf die Antwort; der Dialog meldet sie über einen Kanal zurück
async fn fragen<R: Runtime>(app: &AppHandle<R>, text: &str) -> bool {
    let (sender, empfaenger) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(text)
        .title("Update für Nexus")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Installieren".to_string(),
            "Später".to_string(),
        ))
        .show(move |bestaetigt| {
            let _ = sender.send(bestaetigt);
        });
    empfaenger.await.unwrap_or(false)
}

// Hat der Benutzer diese Version schon abgelehnt?
fn abgelehnt<R: Runtime>(app: &AppHandle<R>, version: &str) -> bool {
    let Some(state) = app.try_state::<Abgelehnt>() else {
        return false;
    };
    let gemerkt = state.version.lock().expect("Abgelehnt-Sperre vergiftet");
    schon_abgelehnt(gemerkt.as_deref(), version)
}

// Merkt die abgelehnte Version, damit die App nicht bei jeder Prüfung erneut fragt
fn merken<R: Runtime>(app: &AppHandle<R>, version: &str) {
    if let Some(state) = app.try_state::<Abgelehnt>() {
        let mut gemerkt = state.version.lock().expect("Abgelehnt-Sperre vergiftet");
        *gemerkt = Some(version.to_string());
    }
}

// Laufende Dispatch-Läufe für den Hinweis im Dialog; 0, wenn der Server nicht erreichbar ist
async fn laufende_laeufe() -> u32 {
    let Ok(client) = crate::http::client(STATUS_TIMEOUT) else {
        return 0;
    };
    match lifecycle::dispatch_status(&client, &crate::tray::basis_url()).await {
        Some(status) => lifecycle::laufende_laeufe(&status),
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Der Debug-Build prüft nie; unter Linux nur das AppImage, deb und rpm nicht
    #[test]
    fn prueft_nur_im_release_und_unter_linux_nur_als_appimage() {
        assert!(soll_pruefen(false, false, false));
        assert!(!soll_pruefen(true, false, false));
        assert!(soll_pruefen(false, true, true));
        assert!(!soll_pruefen(false, true, false));
        assert!(!soll_pruefen(true, true, true));
    }

    // Abgelehnt gilt nur für die abgelehnte Version; eine neuere wird wieder angeboten
    #[test]
    fn fragt_nach_abgelehnter_version_nicht_erneut() {
        assert!(schon_abgelehnt(Some("1.2.3"), "1.2.3"));
        assert!(!schon_abgelehnt(Some("1.2.3"), "1.2.4"));
        assert!(!schon_abgelehnt(None, "1.2.3"));
    }

    // Der erste Takt kommt 30 s nach dem Start, die weiteren täglich
    #[test]
    fn prueft_nach_dreissig_sekunden_und_dann_taeglich() {
        assert_eq!(START_VERZUG, Duration::from_secs(30));
        assert_eq!(INTERVALL, Duration::from_secs(60 * 60 * 24));
    }

    // Ohne laufende Läufe nennt die Rückfrage nur die Versionen
    #[test]
    fn frage_nennt_beide_versionen() {
        let text = frage("1.3.0", "1.2.0", 0);
        assert_eq!(
            text,
            "Nexus 1.3.0 ist verfügbar (installiert: 1.2.0). Jetzt installieren und neu starten?"
        );
    }

    // Geprüft wird gegen latest.json des neuesten Releases; unter Windows läuft der Installer ohne
    // Rückfragen durch. Der öffentliche Schlüssel steht daneben in `pubkey` (aus
    // .github/release/tauri-updater.pub, siehe M4); er ist kein Geheimnis.
    #[test]
    fn konfig_nennt_endpunkt_und_installmodus() {
        let datei: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let updater = &datei["plugins"]["updater"];
        assert_eq!(
            updater["endpoints"].as_array().unwrap(),
            &[serde_json::json!(
                "https://github.com/BenjaminBiber/Nexus/releases/latest/download/latest.json"
            )]
        );
        assert_eq!(updater["windows"]["installMode"], "passive");
        let schluessel = include_str!("../../../../.github/release/tauri-updater.pub");
        assert_eq!(
            updater["pubkey"].as_str().unwrap(),
            schluessel.trim(),
            "pubkey weicht von .github/release/tauri-updater.pub ab"
        );
    }

    // Die Updater-Artefakte stehen nur in der Release-Konfig: in tauri.conf.json würde jeder lokale
    // Build ohne TAURI_SIGNING_PRIVATE_KEY abbrechen
    #[test]
    fn updater_artefakte_nur_in_der_release_konfig() {
        let konfig: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert!(konfig["bundle"]["createUpdaterArtifacts"].is_null());

        let release: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.release.conf.json")).unwrap();
        assert_eq!(release["bundle"]["createUpdaterArtifacts"], true);
    }

    // Laufen Dispatch-Läufe, sagt die Rückfrage, was der Neustart mit ihnen macht
    #[test]
    fn frage_nennt_laufende_dispatch_laeufe() {
        let einer = frage("1.3.0", "1.2.0", 1);
        assert!(
            einer.contains("Es läuft gerade 1 Dispatch-Lauf;"),
            "{einer}"
        );
        assert!(einer.contains("eigenen Prozess weiter"), "{einer}");
        assert!(
            einer.ends_with("Jetzt installieren und neu starten?"),
            "{einer}"
        );

        let mehrere = frage("1.3.0", "1.2.0", 3);
        assert!(
            mehrere.contains("Es laufen gerade 3 Dispatch-Läufe;"),
            "{mehrere}"
        );
        assert!(mehrere.contains("eigenen Prozessen weiter"), "{mehrere}");
    }
}
