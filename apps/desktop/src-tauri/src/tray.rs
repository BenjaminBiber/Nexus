// Tray-Symbol mit Status: Menü zum Öffnen und Beenden, eine Statuszeile je Quelle und drei Symbole.
// Den Status liest die App aus den vorhandenen Routen /api/dispatch/status und /api/db/inbox; sie ändert dort nichts.
use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;
use tauri::image::Image;
use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Runtime};

use crate::lifecycle;
use crate::server;

// Kennung des Tray-Symbols; es gibt nur eines
const TRAY_ID: &str = "main";
// Kennungen der Menüeinträge
const OEFFNEN: &str = "oeffnen";
pub const BEENDEN: &str = "beenden";
// Takt der Statusabfrage; ein offenes Fenster hat seinen eigenen Ereignisstrom, das Tray darf träger sein
const INTERVALL: Duration = Duration::from_secs(10);
// So lange darf eine einzelne Abfrage dauern
const TIMEOUT: Duration = Duration::from_secs(2);
// Art eines Inbox-Eintrags, die als offene Frage zählt (InboxKind in packages/core)
const ART_FRAGE: &str = "question";

// Zustand des Tray-Symbols, angelehnt an den Toast-Zustand des Servers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    Bereit,
    Wartet,
    Frage,
}

impl TrayState {
    // Symbol des Zustands; auf macOS in doppelter Auflösung, weil die Menüleiste auf 18 pt verkleinert
    fn icon(self) -> tauri::Result<Image<'static>> {
        #[cfg(target_os = "macos")]
        let bytes: &[u8] = match self {
            Self::Bereit => include_bytes!("../icons/tray/bereit@2x.png"),
            Self::Wartet => include_bytes!("../icons/tray/wartet@2x.png"),
            Self::Frage => include_bytes!("../icons/tray/frage@2x.png"),
        };
        #[cfg(not(target_os = "macos"))]
        let bytes: &[u8] = match self {
            Self::Bereit => include_bytes!("../icons/tray/bereit.png"),
            Self::Wartet => include_bytes!("../icons/tray/wartet.png"),
            Self::Frage => include_bytes!("../icons/tray/frage.png"),
        };
        Image::from_bytes(bytes)
    }

    // Wort für den Tooltip
    fn wort(self) -> &'static str {
        match self {
            Self::Bereit => "bereit",
            Self::Wartet => "wartet",
            Self::Frage => "Frage offen",
        }
    }
}

// Was von GET /api/dispatch/status gebraucht wird; alles andere ignoriert serde
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DispatchSummary {
    pub state: String,
    pub message: String,
    pub active: u32,
    pub planning: u32,
    pub research: ResearchSummary,
}

// Laufende Recherchen; sie zählen nicht unter `active`
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ResearchSummary {
    pub active: u32,
}

// Was von GET /api/db/inbox gebraucht wird: die Zähler je Art
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct InboxSummary {
    pub counts: HashMap<String, u32>,
    pub total: u32,
}

impl InboxSummary {
    // Offene Fragen
    fn fragen(&self) -> u32 {
        self.counts.get(ART_FRAGE).copied().unwrap_or(0)
    }

    // Alles andere, was wartet: Abnahme, Merge, Blockiert, Notizen …
    fn weitere(&self) -> u32 {
        self.total.saturating_sub(self.fragen())
    }
}

// Zustand des Symbols: Eine offene Frage hat Vorrang, dann alles andere Wartende, sonst bereit.
// Der Dispatcher ändert das Symbol nicht – ein laufender Lauf wartet nicht auf den Benutzer; er steht
// in der Statuszeile des Menüs.
pub fn tray_state(_dispatch: Option<&DispatchSummary>, inbox: Option<&InboxSummary>) -> TrayState {
    match inbox {
        Some(inbox) if inbox.fragen() > 0 => TrayState::Frage,
        Some(inbox) if inbox.weitere() > 0 => TrayState::Wartet,
        _ => TrayState::Bereit,
    }
}

// Die drei Texte des Menüs und des Tooltips; sie wechseln gemeinsam
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texte {
    pub tooltip: String,
    pub dispatch: String,
    pub inbox: String,
}

// Statuszeilen aus den Antworten; fehlt eine Quelle, steht das auch so im Menü
pub fn texte(
    zustand: TrayState,
    dispatch: Option<&DispatchSummary>,
    inbox: Option<&InboxSummary>,
) -> Texte {
    Texte {
        tooltip: format!("Nexus – {}", zustand.wort()),
        dispatch: match dispatch {
            Some(status) if !status.message.is_empty() => {
                format!("Dispatcher: {}", status.message)
            }
            Some(_) => "Dispatcher: aus".to_string(),
            None => "Server nicht erreichbar".to_string(),
        },
        inbox: match inbox {
            Some(inbox) => format!(
                "Offen: {} Fragen, {} weitere",
                inbox.fragen(),
                inbox.weitere()
            ),
            None => "Offen: unbekannt".to_string(),
        },
    }
}

// Legt das Tray-Symbol an und startet die Statusabfrage. Ein Fehler ist kein Grund, die App nicht zu starten:
// unter Linux ohne AppIndicator gibt es kein Tray, dort bleibt das Fenster der einzige Weg.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    match anlegen(app) {
        Ok(zeilen) => {
            lifecycle::tray_gemerkt(app, true);
            let handle = app.clone();
            tauri::async_runtime::spawn(beobachten(handle, zeilen));
        }
        Err(error) => {
            log::warn!("Das Tray-Symbol ließ sich nicht anlegen: {error}");
            lifecycle::tray_gemerkt(app, false);
        }
    }
}

// Die beiden Statuszeilen des Menüs; nur ihr Text ändert sich
struct Zeilen<R: Runtime> {
    dispatch: MenuItem<R>,
    inbox: MenuItem<R>,
}

// Baut Menü und Symbol
fn anlegen<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Zeilen<R>> {
    let anfang = texte(TrayState::Bereit, None, None);
    // Die Statuszeilen sind nur Anzeige, darum nicht anklickbar
    let dispatch = MenuItemBuilder::with_id("status-dispatch", &anfang.dispatch)
        .enabled(false)
        .build(app)?;
    let inbox = MenuItemBuilder::with_id("status-inbox", &anfang.inbox)
        .enabled(false)
        .build(app)?;
    let menu = MenuBuilder::new(app)
        .text(OEFFNEN, "Nexus öffnen")
        .separator()
        .item(&dispatch)
        .item(&inbox)
        .separator()
        .text(BEENDEN, "Beenden")
        .build()?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(TrayState::Bereit.icon()?)
        .tooltip(&anfang.tooltip)
        .menu(&menu)
        // Auf macOS öffnet der Linksklick das Menü; dort wäre ein eigener Griff im Weg
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id().as_ref() {
            OEFFNEN => lifecycle::fenster_zeigen(app),
            BEENDEN => lifecycle::beenden_anfragen(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Linksklick auf das Symbol zeigt das Fenster (Windows und Linux)
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if !cfg!(target_os = "macos") {
                    lifecycle::fenster_zeigen(tray.app_handle());
                }
            }
        })
        .build(app)?;
    Ok(Zeilen { dispatch, inbox })
}

// Fragt den Status im Takt ab und setzt Symbol, Tooltip und Menütexte nur, wenn sich etwas geändert hat
async fn beobachten<R: Runtime>(app: AppHandle<R>, zeilen: Zeilen<R>) {
    let client = match reqwest::Client::builder().timeout(TIMEOUT).build() {
        Ok(client) => client,
        Err(error) => {
            log::warn!("Die Statusabfrage ließ sich nicht vorbereiten: {error}");
            return;
        }
    };
    let basis = basis_url();
    let mut gezeigt: Option<(TrayState, Texte)> = None;
    loop {
        let dispatch = lifecycle::dispatch_status(&client, &basis).await;
        let inbox = holen::<InboxSummary>(&client, &format!("{basis}/api/db/inbox")).await;
        let zustand = tray_state(dispatch.as_ref(), inbox.as_ref());
        let neu = texte(zustand, dispatch.as_ref(), inbox.as_ref());
        if gezeigt.as_ref() != Some(&(zustand, neu.clone())) {
            anzeigen(&app, &zeilen, zustand, &neu);
            gezeigt = Some((zustand, neu));
        }
        tokio::time::sleep(INTERVALL).await;
    }
}

// Schreibt Zustand und Texte ans Symbol
fn anzeigen<R: Runtime>(app: &AppHandle<R>, zeilen: &Zeilen<R>, zustand: TrayState, neu: &Texte) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    log::info!(
        "Tray: {} · {} · {}",
        zustand.wort(),
        neu.dispatch,
        neu.inbox
    );
    let gesetzt = zustand
        .icon()
        .and_then(|icon| tray.set_icon(Some(icon)))
        .and_then(|()| tray.set_tooltip(Some(&neu.tooltip)))
        .and_then(|()| zeilen.dispatch.set_text(&neu.dispatch))
        .and_then(|()| zeilen.inbox.set_text(&neu.inbox));
    if let Err(error) = gesetzt {
        log::warn!("Das Tray-Symbol ließ sich nicht setzen: {error}");
    }
}

// Adresse des eigenen Servers ohne Schrägstrich am Ende; ein ungültiger HARNESS_PORT fällt auf den Standard zurück
pub fn basis_url() -> String {
    let port =
        server::port_from_env(&std::env::vars().collect()).unwrap_or(server::DEFAULT_SERVER_PORT);
    format!("http://127.0.0.1:{port}")
}

// Holt eine Antwort; None, wenn der Server nicht erreichbar ist oder einen Fehler meldet
// (kein Board eingerichtet, keine Board-Datenbank)
pub async fn holen<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
) -> Option<T> {
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json::<T>().await.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Antwort von GET /api/dispatch/status, gekürzt auf eine echte Form (DispatchStatusDto)
    const DISPATCH_JSON: &str = r#"{
        "enabled": true, "state": "running", "message": "läuft · 1/2", "pausedUntil": null,
        "wip": 2, "active": 1, "planning": 0,
        "runs": [{"id":"r1","kind":"work","ticketId":"NXS-0027","title":"Tray","profile":"schwer",
                  "model":"opus","effort":"medium","startedAt":"2026-10-28T12:39:48.958Z","log":"r1.log"}],
        "research": {"auto": true, "wip": 1, "active": 2, "maxPerDay": 10, "today": 1,
                     "since": null, "retryAt": null, "error": null},
        "planningAuto": {"auto": false, "wip": 1, "active": 0, "since": null, "retryAt": null, "error": null},
        "merges": [], "boardId": 3, "boardInScope": true, "tickets": [], "ticketsError": null,
        "lastTick": "2026-10-28T12:40:00.000Z", "lastErrors": [], "configFile": "/tmp/dispatch.yaml",
        "configError": null, "agentPrefix": "junior-"
    }"#;

    // Antwort von GET /api/db/inbox, gekürzt (InboxListDto)
    fn inbox(fragen: u32, weitere: u32) -> InboxSummary {
        let json = format!(
            r#"{{"items":[],"counts":{{"merge":0,"setup":0,"question":{fragen},"accept":{weitere},
               "blocked":0,"planning":0,"note":0}},"total":{}}}"#,
            fragen + weitere
        );
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn zustand_frage_hat_vorrang() {
        let status = dispatch();
        assert_eq!(
            tray_state(Some(&status), Some(&inbox(1, 3))),
            TrayState::Frage
        );
    }

    #[test]
    fn zustand_wartet_bei_abnahme() {
        assert_eq!(tray_state(None, Some(&inbox(0, 2))), TrayState::Wartet);
    }

    #[test]
    fn zustand_bereit_ohne_eintraege() {
        assert_eq!(tray_state(None, Some(&inbox(0, 0))), TrayState::Bereit);
    }

    #[test]
    fn zustand_ohne_daten_ist_bereit() {
        assert_eq!(tray_state(None, None), TrayState::Bereit);
    }

    // Zusätzliche Felder der Antwort dürfen nichts brechen, fehlende auch nicht
    #[test]
    fn liest_dispatch_status_mit_zusatzfeldern() {
        let status = dispatch();
        assert_eq!(status.state, "running");
        assert_eq!(status.message, "läuft · 1/2");
        assert_eq!(status.active, 1);
        assert_eq!(status.planning, 0);
        assert_eq!(status.research.active, 2);

        let knapp: DispatchSummary = serde_json::from_str(r#"{"state":"idle"}"#).unwrap();
        assert_eq!(knapp.active, 0);
        assert_eq!(knapp.research.active, 0);
        assert_eq!(knapp.message, "");
    }

    // Zähler unbekannter Arten zählen zu „weitere“, weil `total` sie mitzählt
    #[test]
    fn liest_inbox_mit_unbekannter_art() {
        let gelesen: InboxSummary =
            serde_json::from_str(r#"{"counts":{"question":2,"todo":1},"total":3}"#).unwrap();
        assert_eq!(gelesen.fragen(), 2);
        assert_eq!(gelesen.weitere(), 1);
    }

    #[test]
    fn statuszeile_texte() {
        let status = dispatch();
        let voll = texte(TrayState::Frage, Some(&status), Some(&inbox(1, 2)));
        assert_eq!(voll.tooltip, "Nexus – Frage offen");
        assert_eq!(voll.dispatch, "Dispatcher: läuft · 1/2");
        assert_eq!(voll.inbox, "Offen: 1 Fragen, 2 weitere");

        let ohne = texte(TrayState::Bereit, None, None);
        assert_eq!(ohne.tooltip, "Nexus – bereit");
        assert_eq!(ohne.dispatch, "Server nicht erreichbar");
        assert_eq!(ohne.inbox, "Offen: unbekannt");

        let leer: DispatchSummary = serde_json::from_str(r#"{"state":"off"}"#).unwrap();
        assert_eq!(
            texte(TrayState::Bereit, Some(&leer), None).dispatch,
            "Dispatcher: aus"
        );
    }

    #[test]
    fn basis_url_zeigt_auf_den_eigenen_port() {
        let url = basis_url();
        assert!(url.starts_with("http://127.0.0.1:"), "{url}");
        assert!(!url.ends_with('/'), "{url}");
    }

    fn dispatch() -> DispatchSummary {
        serde_json::from_str(DISPATCH_JSON).unwrap()
    }
}
