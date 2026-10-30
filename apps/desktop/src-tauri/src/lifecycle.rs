// Leben der App ohne Fenster: Schließen versteckt das Fenster, Tray und Dock holen es zurück,
// und „Beenden“ fragt nach, solange Dispatch-Läufe laufen.
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "macos")]
use tauri::menu::{AboutMetadata, Menu, MenuItemBuilder, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Runtime, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::server;
use crate::tray::{self, DispatchSummary};

// Das Fenster, das die Oberfläche trägt
const HAUPTFENSTER: &str = "main";

// Gibt es ein Tray-Symbol? Ohne Tray (Linux ohne AppIndicator) darf das Fenster nicht verschwinden,
// sonst käme der Benutzer nicht mehr an die App.
#[derive(Default)]
pub struct Tray {
    vorhanden: AtomicBool,
}

// Legt den Platz für die Tray-Angabe an; vor `tray::init` aufzurufen
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    app.manage(Tray::default());
}

// Merkt, ob das Tray-Symbol angelegt werden konnte
pub fn tray_gemerkt<R: Runtime>(app: &AppHandle<R>, vorhanden: bool) {
    if let Some(tray) = app.try_state::<Tray>() {
        tray.vorhanden.store(vorhanden, Ordering::Relaxed);
    }
}

// Gibt es ein Tray-Symbol, über das die App erreichbar bleibt?
fn tray_vorhanden<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<Tray>()
        .is_some_and(|tray| tray.vorhanden.load(Ordering::Relaxed))
}

// Zeigt das Hauptfenster und holt es nach vorne
pub fn fenster_zeigen<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(HAUPTFENSTER) else {
        return;
    };
    let gezeigt = window.show().and_then(|()| window.unminimize());
    if let Err(error) = gezeigt.and_then(|()| window.set_focus()) {
        log::warn!("Das Fenster ließ sich nicht zeigen: {error}");
    }
}

// Das Fenster schließen heißt verstecken: Dispatch-Läufe und Meldungen hängen nicht am offenen Fenster.
// Ohne Tray bleibt es beim echten Schließen, sonst wäre die App nicht mehr erreichbar.
pub fn fenster_ereignis<R: Runtime>(window: &tauri::Window<R>, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    if window.label() != HAUPTFENSTER || !tray_vorhanden(window.app_handle()) {
        return;
    }
    api.prevent_close();
    if let Err(error) = window.hide() {
        log::warn!("Das Fenster ließ sich nicht verstecken: {error}");
    }
}

// Ereignisse der laufenden App: Klick aufs Dock-Symbol und der Wunsch zu beenden
pub fn lauf_ereignis<R: Runtime>(app: &AppHandle<R>, event: &tauri::RunEvent) {
    // macOS: Klick auf das Dock-Symbol, während kein Fenster sichtbar ist. Das Ereignis gibt es nur dort.
    #[cfg(target_os = "macos")]
    if let tauri::RunEvent::Reopen {
        has_visible_windows: false,
        ..
    } = event
    {
        fenster_zeigen(app);
    }
    // `code` ist None, wenn der Benutzer beenden will (Cmd+Q, letztes Fenster zu); dann erst fragen.
    // Programmatisch (code Some) kommt der Wunsch schon aus `beenden_anfragen` oder aus einem Signal.
    if let tauri::RunEvent::ExitRequested {
        code: None, api, ..
    } = event
    {
        api.prevent_exit();
        beenden_anfragen(app);
    }
}

// Beenden: erst den frischen Dispatch-Status holen, bei laufenden Läufen nachfragen, dann beenden
pub fn beenden_anfragen<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let laeufe = match status_holen().await {
            Some(status) => laufende_laeufe(&status),
            None => 0,
        };
        if laeufe == 0 {
            handle.exit(0);
            return;
        }
        let frage = rueckfrage(laeufe, server::eigener_server(&handle));
        let beenden = handle.clone();
        handle
            .dialog()
            .message(frage)
            .title("Nexus beenden?")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Beenden".to_string(),
                "Abbrechen".to_string(),
            ))
            .show(move |bestaetigt| {
                if bestaetigt {
                    beenden.exit(0);
                } else {
                    log::info!("Beenden abgebrochen, {laeufe} Dispatch-Läufe laufen weiter.");
                }
            });
    });
}

// Alles, was gerade im Auftrag des Dispatchers läuft: Ticket-Läufe, Planungen und Recherchen
pub fn laufende_laeufe(status: &DispatchSummary) -> u32 {
    status.active + status.planning + status.research.active
}

// Text der Rückfrage. Die Worker laufen losgelöst weiter (ProcessWorkerLauncher startet sie detached),
// nur das Auswerten hängt am Server – darum verspricht der Text nichts anderes.
pub fn rueckfrage(laeufe: u32, eigener_server: bool) -> String {
    let anfang = if laeufe == 1 {
        "Es läuft gerade 1 Dispatch-Lauf.".to_string()
    } else {
        format!("Es laufen gerade {laeufe} Dispatch-Läufe.")
    };
    let folge = if eigener_server {
        "Die Läufe selbst laufen in eigenen Prozessen weiter, aber der Server, den Nexus gestartet hat, hört mit auf: \
         Ihre Ergebnisse werden erst beim nächsten Start ausgewertet."
    } else {
        "Die Läufe und der Server laufen weiter; Nexus hat den Server nicht gestartet und beendet ihn nicht."
    };
    format!("{anfang} {folge} Trotzdem beenden?")
}

// Holt den Dispatch-Status für die Rückfrage; None, wenn der Server nicht erreichbar ist
// oder kein Board eingerichtet ist (dann läuft auch kein Dispatch)
async fn status_holen() -> Option<DispatchSummary> {
    let client = crate::http::client(std::time::Duration::from_secs(2)).ok()?;
    dispatch_status(&client, &tray::basis_url()).await
}

// Eine Abfrage von GET /api/dispatch/status
pub async fn dispatch_status(client: &reqwest::Client, basis: &str) -> Option<DispatchSummary> {
    tray::holen(client, &format!("{basis}/api/dispatch/status")).await
}

// Menü der App auf macOS: wie das Standardmenü von Tauri, nur beendet „Nexus beenden“ über die Rückfrage.
// Das vorgefertigte „Quit“ ginge direkt an das Betriebssystem und ließe sich nicht abfangen.
#[cfg(target_os = "macos")]
pub fn app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let info = app.package_info();
    let about = AboutMetadata {
        name: Some(info.name.clone()),
        version: Some(info.version.to_string()),
        copyright: app.config().bundle.copyright.clone(),
        authors: app.config().bundle.publisher.clone().map(|p| vec![p]),
        ..Default::default()
    };
    let beenden = MenuItemBuilder::with_id(tray::BEENDEN, format!("{} beenden", info.name))
        .accelerator("CmdOrCtrl+Q")
        .build(app)?;
    Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                info.name.clone(),
                true,
                &[
                    &PredefinedMenuItem::about(app, None, Some(about))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::services(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, None)?,
                    &PredefinedMenuItem::hide_others(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &beenden,
                ],
            )?,
            &Submenu::with_items(
                app,
                "Datei",
                true,
                &[&PredefinedMenuItem::close_window(app, None)?],
            )?,
            &Submenu::with_items(
                app,
                "Bearbeiten",
                true,
                &[
                    &PredefinedMenuItem::undo(app, None)?,
                    &PredefinedMenuItem::redo(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, None)?,
                    &PredefinedMenuItem::copy(app, None)?,
                    &PredefinedMenuItem::paste(app, None)?,
                    &PredefinedMenuItem::select_all(app, None)?,
                ],
            )?,
            &Submenu::with_items(
                app,
                "Ansicht",
                true,
                &[&PredefinedMenuItem::fullscreen(app, None)?],
            )?,
            &Submenu::with_items(
                app,
                "Fenster",
                true,
                &[
                    &PredefinedMenuItem::minimize(app, None)?,
                    &PredefinedMenuItem::maximize(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::close_window(app, None)?,
                ],
            )?,
        ],
    )
}

// Menüereignisse der App; auf macOS kommt „Nexus beenden“ hier an
pub fn menu_ereignis<R: Runtime>(app: &AppHandle<R>, event: tauri::menu::MenuEvent) {
    if event.id().as_ref() == tray::BEENDEN {
        beenden_anfragen(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(json: &str) -> DispatchSummary {
        serde_json::from_str(json).unwrap()
    }

    // Ticket-Läufe, Planungen und Recherchen zählen zusammen; fehlende Felder zählen als null
    #[test]
    fn zaehlt_laufende_laeufe() {
        assert_eq!(
            laufende_laeufe(&status(
                r#"{"active":2,"planning":1,"research":{"active":3}}"#
            )),
            6
        );
        assert_eq!(laufende_laeufe(&status(r#"{"state":"idle"}"#)), 0);
        assert_eq!(
            laufende_laeufe(&status(
                r#"{"active":0,"planning":0,"research":{"active":1}}"#
            )),
            1
        );
    }

    // Die Rückfrage darf nur versprechen, was der Code tut: Worker laufen losgelöst weiter
    #[test]
    fn rueckfrage_nennt_folgen_des_beendens() {
        let eigener = rueckfrage(2, true);
        assert!(
            eigener.starts_with("Es laufen gerade 2 Dispatch-Läufe."),
            "{eigener}"
        );
        assert!(
            eigener.contains("laufen in eigenen Prozessen weiter"),
            "{eigener}"
        );
        assert!(
            eigener.contains("erst beim nächsten Start ausgewertet"),
            "{eigener}"
        );
        assert!(eigener.ends_with("Trotzdem beenden?"), "{eigener}");

        let fremder = rueckfrage(1, false);
        assert!(
            fremder.starts_with("Es läuft gerade 1 Dispatch-Lauf."),
            "{fremder}"
        );
        assert!(fremder.contains("beendet ihn nicht"), "{fremder}");
    }
}
