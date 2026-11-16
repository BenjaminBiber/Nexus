// Systemmeldungen der Oberfläche (Befehl `meldung_zeigen`).
//
// Unter Windows zeigt die App dieselbe Meldung wie der Server – Logo je Anlass, großes Bild bei
// „wartet“, Schaltfläche „Board öffnen“ –, nur führt ein Klick über einen nexus://-Link zurück in
// dieses Fenster statt in den Browser. Das Notification-Plugin taugt dafür unter Windows nicht:
// seine Meldungen haben kein Ziel, ein Klick darauf bleibt folgenlos.
// Auf macOS und Linux zeigt weiter das Plugin Titel und Text.
use serde::Deserialize;
use tauri::{AppHandle, Runtime};

use nexus_server::notifications::{ToastKind, ToastModel};

// Eine Meldung, wie der Server sie an offene Fenster schickt; `url` ist ein Pfad der Oberfläche
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Meldung {
    pub kind: ToastKind,
    // Gleiche Kennung ersetzt eine frühere Meldung
    pub tag: String,
    pub title: String,
    pub body: String,
    pub url: String,
}

// Link, über den ein Klick den Pfad in diesem Fenster öffnet (siehe instance::deep_link_path)
pub fn klick_link(pfad: &str) -> String {
    match pfad.starts_with('/') {
        true => format!("nexus://open{pfad}"),
        false => format!("nexus://open/{pfad}"),
    }
}

// Modell der Meldung wie beim Server: erste Zeile des Texts oben, der Rest darunter
pub fn modell(meldung: &Meldung) -> ToastModel {
    let (line1, line2) = meldung
        .body
        .split_once('\n')
        .unwrap_or((meldung.body.as_str(), ""));
    ToastModel {
        kind: meldung.kind,
        title: meldung.title.clone(),
        line1: line1.to_string(),
        line2: line2.to_string(),
        open_url: klick_link(&meldung.url),
    }
}

// Zeigt die Meldung als Systemmeldung dieses Rechners, auch bei verstecktem Fenster
#[tauri::command]
pub async fn meldung_zeigen<R: Runtime>(app: AppHandle<R>, meldung: Meldung) -> Result<(), String> {
    zeigen(&app, &meldung).await
}

// Windows: Toast über WinRT, unter derselben App-Kennung wie die Meldungen des Servers
#[cfg(windows)]
async fn zeigen<R: Runtime>(app: &AppHandle<R>, meldung: &Meldung) -> Result<(), String> {
    use nexus_platform::ToastRequest;
    use nexus_server::notifications::service::APP_ICON;
    use nexus_server::notifications::{ToastOptions, ToastXmlBuilder};

    let bilder = crate::server::toast_ordner(app);
    let model = modell(meldung);
    let xml = ToastXmlBuilder.build(
        &model,
        &ToastOptions {
            assets_dir: bilder.clone(),
            hero_images: grosses_bild(),
        },
    );
    let request = ToastRequest {
        title: model.title.clone(),
        body: meldung.body.clone(),
        url: Some(model.open_url.clone()),
        tag: Some(meldung.tag.clone()),
        xml: Some(xml),
        icon: Some(bilder.join(APP_ICON)),
    };
    match nexus_platform::notifier::local().show(&request).await {
        None => Ok(()),
        Some(error) => {
            log::warn!("Die Systemmeldung ließ sich nicht zeigen: {error}");
            Err(error)
        }
    }
}

// macOS und Linux: das Plugin zeigt Titel und Text
#[cfg(not(windows))]
async fn zeigen<R: Runtime>(app: &AppHandle<R>, meldung: &Meldung) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title(&meldung.title)
        .body(&meldung.body)
        .show()
        .map_err(|error| error.to_string())
}

// Einstellung „Großes Bild bei braucht deine Antwort“ aus dem Nexus-Ordner, den auch der Server liest
#[cfg(windows)]
fn grosses_bild() -> bool {
    use nexus_core::registry::HarnessHome;
    use nexus_core::settings::UserSettingsStore;

    let home = crate::server::harness_home(&std::env::vars().collect());
    UserSettingsStore::new(HarnessHome::at(&home))
        .read()
        .hero_images
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::deep_link_path;
    use nexus_server::notifications::{ToastOptions, ToastXmlBuilder};

    fn meldung(body: &str, url: &str) -> Meldung {
        Meldung {
            kind: ToastKind::Wartet,
            tag: "attention".into(),
            title: "Session braucht deine Antwort".into(),
            body: body.into(),
            url: url.into(),
        }
    }

    #[test]
    fn die_meldung_kommt_als_json_der_oberflaeche_an() {
        let gelesen: Meldung = serde_json::from_str(
            r#"{"kind":"frage-neu","tag":"question-p1-ARC-F0001","title":"T","body":"B","url":"/p/p1/questions/ARC-F0001"}"#,
        )
        .unwrap();
        assert_eq!(gelesen.kind, ToastKind::FrageNeu);
        assert_eq!(gelesen.url, "/p/p1/questions/ARC-F0001");
    }

    // Was der Klick öffnet, muss der Deep-Link wieder auf genau diesen Pfad zurückführen
    #[test]
    fn der_klick_fuehrt_auf_den_pfad_der_meldung() {
        for pfad in [
            "/p/webshop/board/t/ARC-0042",
            "/p/webshop/sessions",
            "/p/webshop/questions/ARC-F0001",
            "/",
        ] {
            let link = klick_link(pfad);
            assert!(link.starts_with("nexus://open/"), "{link}");
            assert_eq!(deep_link_path(&link).as_deref(), Some(pfad), "{link}");
        }
    }

    #[test]
    fn der_text_verteilt_sich_auf_zwei_zeilen() {
        let zwei = modell(&meldung(
            "webshop · ARC-0042\nFavoriten",
            "/p/webshop/board/t/ARC-0042",
        ));
        assert_eq!(zwei.line1, "webshop · ARC-0042");
        assert_eq!(zwei.line2, "Favoriten");
        let eine = modell(&meldung("webshop", "/p/webshop/sessions"));
        assert_eq!(eine.line1, "webshop");
        assert_eq!(eine.line2, "");
    }

    // Windows startet beim Klick den Link (activationType protocol); ohne ihn passiert nichts
    #[test]
    fn klick_und_schaltflaeche_oeffnen_die_app() {
        let model = modell(&meldung("webshop", "/p/webshop/sessions"));
        let xml = ToastXmlBuilder.build(
            &model,
            &ToastOptions {
                assets_dir: std::path::PathBuf::from("toast"),
                hero_images: false,
            },
        );
        assert!(
            xml.contains(
                r#"<toast launch="nexus://open/p/webshop/sessions" activationType="protocol">"#
            ),
            "{xml}"
        );
        assert!(
            xml.contains(r#"<action content="Sessions öffnen" arguments="nexus://open/p/webshop/sessions" activationType="protocol"/>"#),
            "{xml}"
        );
    }
}
