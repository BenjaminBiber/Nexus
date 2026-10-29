//! Benachrichtigungen des Servers; portiert aus
//! `packages/server/tests/unit/Notifications.test.ts`.
//!
//! Kein Test zeigt eine echte Systemmeldung und keiner ruft einen echten ntfy-Server auf:
//! die Meldungen gehen an [`FakeNotifier`], der Versand an eine Attrappe des Abrufs.

mod common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use common::notify::{model, service, view, ServiceParts};
use nexus_core::sessions::session::DisplayState;
use nexus_core::settings::{NotificationMode, UserSettings};
use nexus_server::notifications::attention::{AttentionKind, AttentionTracker, PendingSession};
use nexus_server::notifications::model::ToastKind;
use nexus_server::notifications::ntfy::{NtfyResponse, NtfySender, NtfyTarget};
use nexus_server::notifications::texts::{SingleInput, ToastTexts};
use nexus_server::notifications::{SessionChange, TicketRef};

// ---------------------------------------------------------------- AttentionTracker

#[test]
fn meldet_bereit_nur_nach_laeuft_und_wartet_nur_einmal() {
    let mut tracker = AttentionTracker::new();
    assert_eq!(
        tracker.observe(None, &view("a", DisplayState::Idle), "A"),
        None
    );
    assert_eq!(
        tracker.observe(
            Some(DisplayState::Running),
            &view("a", DisplayState::Idle),
            "A"
        ),
        Some(AttentionKind::Bereit)
    );
    assert_eq!(
        tracker.observe(
            Some(DisplayState::Running),
            &view("a", DisplayState::Waiting),
            "A"
        ),
        Some(AttentionKind::Wartet)
    );
    assert_eq!(
        tracker.observe(
            Some(DisplayState::Waiting),
            &view("a", DisplayState::Waiting),
            "A"
        ),
        None
    );
}

#[test]
fn entfernt_sessions_die_wieder_laufen_oder_enden() {
    let mut tracker = AttentionTracker::new();
    tracker.observe(
        Some(DisplayState::Running),
        &view("a", DisplayState::Idle),
        "A",
    );
    tracker.observe(
        Some(DisplayState::Running),
        &view("b", DisplayState::Waiting),
        "B",
    );
    tracker.observe(
        Some(DisplayState::Idle),
        &view("a", DisplayState::Running),
        "A",
    );
    assert_eq!(
        tracker.list(),
        vec![PendingSession {
            kind: AttentionKind::Wartet,
            label: "B".into(),
        }]
    );
}

// ---------------------------------------------------------------- ToastTexts

#[test]
fn formuliert_eine_einzelmeldung() {
    let ticket = TicketRef {
        id: "T-0042".into(),
        title: "Favoriten".into(),
    };
    let model = ToastTexts.single(SingleInput {
        kind: AttentionKind::Wartet,
        project_name: "webshop",
        ticket: Some(&ticket),
        session_title: Some("Favoriten als Seitenfenster"),
        session_model: Some("Sonnet 5.5"),
        open_url: "u",
    });
    assert_eq!(model.title, "Session braucht deine Antwort");
    assert_eq!(model.line1, "webshop · T-0042 Favoriten");
    assert_eq!(model.line2, "Favoriten als Seitenfenster · Sonnet 5.5");
}

#[test]
fn zeigt_ohne_ticket_nur_das_projekt() {
    let model = ToastTexts.single(SingleInput {
        kind: AttentionKind::Bereit,
        project_name: "webshop",
        ticket: None,
        session_title: Some("Favoriten als Seitenfenster"),
        session_model: Some("Sonnet 5.5"),
        open_url: "u",
    });
    assert_eq!(model.title, "Session ist fertig");
    assert_eq!(model.line1, "webshop");
}

#[test]
fn ohne_titel_und_modell_steht_zurueck_zur_session_in_der_zweiten_zeile() {
    let model = ToastTexts.single(SingleInput {
        kind: AttentionKind::Bereit,
        project_name: "webshop",
        ticket: None,
        session_title: None,
        session_model: None,
        open_url: "u",
    });
    assert_eq!(model.line2, "Zurück zur Session");
}

#[test]
fn fasst_mehrere_sessions_zusammen_und_kuerzt_die_liste() {
    let pending: Vec<PendingSession> = [
        (AttentionKind::Wartet, "T-1"),
        (AttentionKind::Bereit, "T-2"),
        (AttentionKind::Bereit, "T-3"),
        (AttentionKind::Bereit, "T-4"),
        (AttentionKind::Bereit, "T-5"),
    ]
    .into_iter()
    .map(|(kind, label)| PendingSession {
        kind,
        label: label.into(),
    })
    .collect();
    let model = ToastTexts.collected(&pending, "board");
    assert_eq!(model.kind, ToastKind::Gesammelt);
    assert_eq!(model.title, "5 Sessions warten auf dich");
    assert_eq!(model.line1, "1 braucht eine Antwort, 4 sind fertig");
    assert_eq!(model.line2, "T-1 · T-2 · T-3 + 2 weitere");
}

// ---------------------------------------------------------------- ToastXmlBuilder

/// Das XML der Meldung dieser Art; der Aufbau wird in `toast_xml.rs` gegen TypeScript geprüft
fn xml(kind: ToastKind, open_url: &str, hero_images: bool) -> String {
    use nexus_server::notifications::model::ToastOptions;
    use nexus_server::notifications::toast_xml::ToastXmlBuilder;
    let mut model = model(kind);
    model.title = "A & B".into();
    model.open_url = open_url.into();
    ToastXmlBuilder.build(
        &model,
        &ToastOptions {
            assets_dir: std::path::PathBuf::from("/a/toast"),
            hero_images,
        },
    )
}

#[test]
fn maskiert_sonderzeichen_und_bietet_kein_spaeter_erinnern_an() {
    let out = xml(
        ToastKind::Wartet,
        "http://127.0.0.1:1/p/p1/tickets/T-1",
        false,
    );
    assert!(out.contains("<text>A &amp; B</text>"), "{out}");
    for verboten in [
        "scenario",
        "snooze",
        "Später erinnern",
        "placement=\"hero\"",
    ] {
        assert!(!out.contains(verboten), "{verboten} in {out}");
    }
}

#[test]
fn zeigt_das_hero_nur_bei_wartet_und_eingeschalteter_einstellung() {
    let url = "http://127.0.0.1:1/p/p1/tickets/T-1";
    assert!(xml(ToastKind::Wartet, url, true).contains("placement=\"hero\""));
    assert!(!xml(ToastKind::Bereit, url, true).contains("placement=\"hero\""));
}

#[test]
fn hat_bei_bereit_nur_board_oeffnen_und_kein_ignorieren() {
    let out = xml(
        ToastKind::Bereit,
        "http://127.0.0.1:1/p/p1/tickets/T-1",
        false,
    );
    assert!(out.contains("Board öffnen"), "{out}");
    for verboten in ["scenario", "snooze", "gnorieren"] {
        assert!(!out.contains(verboten), "{verboten} in {out}");
    }
}

#[test]
fn beschriftet_die_schaltflaeche_mit_sessions_oeffnen_wenn_sie_die_sessions_oeffnet() {
    let out = xml(ToastKind::Wartet, "http://127.0.0.1:1/p/p1/sessions", false);
    assert!(out.contains("content=\"Sessions öffnen\""), "{out}");
    assert!(!out.contains("Board öffnen"), "{out}");
}

// ---------------------------------------------------------------- NotificationService

/// Ein Zustandswechsel einer Session des Projekts `p1`
fn change(id: &str, now: DisplayState, before: DisplayState) -> SessionChange {
    SessionChange {
        project_id: "p1".into(),
        project_name: "webshop".into(),
        before: Some(before),
        view: view(id, now),
        ticket: None,
    }
}

#[tokio::test]
async fn zeigt_im_modus_windows_einen_toast_mit_adresse_des_tickets() {
    let parts = windows_service();
    parts
        .service
        .on_session_change(&SessionChange {
            ticket: Some(TicketRef {
                id: "T-1".into(),
                title: "Favoriten".into(),
            }),
            ..change("a", DisplayState::Waiting, DisplayState::Running)
        })
        .await;
    let shown = parts.shown();
    assert_eq!(shown.len(), 1);
    assert!(
        shown[0].contains("http://127.0.0.1:7777/p/p1/board/t/T-1"),
        "{}",
        shown[0]
    );
}

#[tokio::test]
async fn zeigt_in_anderen_modi_nichts() {
    let parts = service(UserSettings {
        notifications: NotificationMode::Browser,
        ..UserSettings::default()
    });
    parts
        .service
        .on_session_change(&change("a", DisplayState::Waiting, DisplayState::Running))
        .await;
    assert!(parts.shown().is_empty());
}

#[tokio::test]
async fn fasst_zwei_wartende_sessions_zu_einer_meldung_zusammen() {
    let parts = windows_service();
    parts
        .service
        .on_session_change(&change("a", DisplayState::Waiting, DisplayState::Running))
        .await;
    parts
        .service
        .on_session_change(&change("b", DisplayState::Idle, DisplayState::Running))
        .await;
    let shown = parts.shown();
    assert!(
        shown[1].contains("2 Sessions warten auf dich"),
        "{}",
        shown[1]
    );
}

#[tokio::test]
async fn verlinkt_die_zusammengefasste_meldung_auf_die_sessions_des_projekts() {
    let parts = windows_service();
    parts
        .service
        .on_session_change(&change("a", DisplayState::Waiting, DisplayState::Running))
        .await;
    parts
        .service
        .on_session_change(&change("b", DisplayState::Idle, DisplayState::Running))
        .await;
    let shown = parts.shown();
    assert!(
        shown[1].contains("http://127.0.0.1:7777/p/p1/sessions"),
        "{}",
        shown[1]
    );
    assert!(!shown[1].contains("/board"), "{}", shown[1]);
}

#[tokio::test]
async fn verlinkt_eine_meldung_ohne_ticket_auf_die_sessions_des_projekts() {
    let parts = windows_service();
    parts
        .service
        .on_session_change(&change("a", DisplayState::Idle, DisplayState::Running))
        .await;
    let shown = parts.shown();
    assert!(
        shown[0].contains("http://127.0.0.1:7777/p/p1/sessions"),
        "{}",
        shown[0]
    );
    assert!(!shown[0].contains("/board"), "{}", shown[0]);
    assert!(
        shown[0].contains("content=\"Sessions öffnen\""),
        "{}",
        shown[0]
    );
}

#[tokio::test]
async fn zeigt_als_testmeldung_ein_beispiel_mit_einer_ticket_id_des_db_boards() {
    let parts = windows_service();
    assert_eq!(parts.service.send_test().await, None);
    let shown = parts.shown();
    assert!(shown[0].contains("ARC-0042 Favoriten"), "{}", shown[0]);
    assert!(!shown[0].contains("T-0042"), "{}", shown[0]);
}

/// Dienst im Modus „windows“ ohne ntfy
fn windows_service() -> ServiceParts {
    service(UserSettings {
        notifications: NotificationMode::Windows,
        ..UserSettings::default()
    })
}

// ---------------------------------------------------- NotificationService mit offenem Tab

#[tokio::test]
async fn schickt_die_meldung_an_den_tab_statt_an_das_system_mit_pfad_der_oberflaeche() {
    let parts = windows_service();
    let (sender, mut empfangen) = tokio::sync::mpsc::unbounded_channel();
    let _tab = parts.tabs.attach(sender);
    parts
        .service
        .on_session_change(&SessionChange {
            ticket: Some(TicketRef {
                id: "T-1".into(),
                title: "Favoriten".into(),
            }),
            ..change("a", DisplayState::Waiting, DisplayState::Running)
        })
        .await;
    assert!(parts.shown().is_empty());
    let event = empfangen.try_recv().expect("eine Meldung");
    let json = serde_json::to_value(&event).expect("JSON");
    assert_eq!(json["type"], "notice");
    assert_eq!(json["kind"], "wartet");
    assert_eq!(json["tag"], "attention");
    assert_eq!(json["title"], "Session braucht deine Antwort");
    assert_eq!(json["url"], "/p/p1/board/t/T-1");
    assert!(empfangen.try_recv().is_err());
}

#[tokio::test]
async fn zeigt_die_systemmeldung_sobald_kein_tab_mehr_offen_ist() {
    let parts = windows_service();
    let (sender, _empfangen) = tokio::sync::mpsc::unbounded_channel();
    drop(parts.tabs.attach(sender));
    parts
        .service
        .on_session_change(&change("a", DisplayState::Waiting, DisplayState::Running))
        .await;
    assert_eq!(parts.shown().len(), 1);
}

// ---------------------------------------------------------------- NtfySender

/// Die Meldung, die die ntfy-Tests senden
fn ntfy_model() -> nexus_server::notifications::model::ToastModel {
    nexus_server::notifications::model::ToastModel {
        kind: ToastKind::Wartet,
        title: "Session braucht deine Antwort".into(),
        line1: "webshop · T-1".into(),
        line2: "Favoriten".into(),
        open_url: "http://127.0.0.1:7777/p/p1".into(),
    }
}

/// Das Ziel, das die ntfy-Tests ansprechen (mit „/“ am Ende, wie in TypeScript)
fn ntfy_target() -> NtfyTarget {
    NtfyTarget {
        server: "https://ntfy.example/".into(),
        topic: "harness-x".into(),
        token: String::new(),
    }
}

/// Eine Anfrage, die die Attrappe des Abrufs aufgezeichnet hat
#[derive(Debug, Clone)]
struct NtfyCall {
    url: String,
    headers: Vec<(String, String)>,
    body: serde_json::Value,
}

/// Abruf für die Tests: zeichnet auf und antwortet mit festem Status
struct CapturingFetch {
    status: u16,
    calls: Arc<Mutex<Vec<NtfyCall>>>,
}

#[async_trait]
impl nexus_server::notifications::ntfy::NtfyFetch for CapturingFetch {
    async fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<NtfyResponse, String> {
        self.calls.lock().expect("Sperre").push(NtfyCall {
            url: url.to_string(),
            headers: headers.to_vec(),
            body: serde_json::from_str(body).expect("Körper als JSON"),
        });
        Ok(NtfyResponse {
            ok: self.status < 300,
            status: self.status,
            text: "unauthorized".into(),
        })
    }
}

/// Sender samt Aufzeichnung der Anfragen
fn capture(status: u16) -> (Arc<Mutex<Vec<NtfyCall>>>, NtfySender) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let sender = NtfySender::new(Box::new(CapturingFetch {
        status,
        calls: calls.clone(),
    }));
    (calls, sender)
}

#[tokio::test]
async fn veroeffentlicht_als_json_mit_thema_text_tag_und_hoher_prioritaet_bei_wartet() {
    let (calls, sender) = capture(200);
    assert_eq!(sender.send(&ntfy_target(), &ntfy_model()).await, None);
    let calls = calls.lock().expect("Sperre").clone();
    assert_eq!(calls[0].url, "https://ntfy.example");
    assert!(
        !calls[0]
            .headers
            .iter()
            .any(|(name, _)| name == "Authorization"),
        "{:?}",
        calls[0].headers
    );
    assert_eq!(
        calls[0].body,
        serde_json::json!({
            "topic": "harness-x",
            "title": "Session braucht deine Antwort",
            "message": "webshop · T-1\nFavoriten",
            "tags": ["raising_hand"],
            "priority": 4,
        })
    );
}

#[tokio::test]
async fn schickt_das_token_und_verlinkt_nur_erreichbare_adressen() {
    let (calls, sender) = capture(200);
    sender
        .send(
            &NtfyTarget {
                token: "tk_1".into(),
                ..ntfy_target()
            },
            &nexus_server::notifications::model::ToastModel {
                kind: ToastKind::Bereit,
                open_url: "http://harness.lan:7777/p/p1".into(),
                ..ntfy_model()
            },
        )
        .await;
    let calls = calls.lock().expect("Sperre").clone();
    assert!(calls[0]
        .headers
        .contains(&("Authorization".to_string(), "Bearer tk_1".to_string())));
    assert_eq!(calls[0].body["priority"], 3);
    assert_eq!(calls[0].body["click"], "http://harness.lan:7777/p/p1");
}

#[tokio::test]
async fn meldet_fehler_des_servers_als_text() {
    let (_calls, sender) = capture(401);
    assert_eq!(
        sender.send(&ntfy_target(), &ntfy_model()).await.as_deref(),
        Some("ntfy antwortete mit 401: unauthorized.")
    );
}

// ---------------------------------------------------- NotificationService mit ntfy

/// Dienst mit eingeschaltetem ntfy; der Versand läuft in die Attrappe
fn with_ntfy(notifications: NotificationMode, topic: &str, on: bool) -> ServiceParts {
    service(UserSettings {
        notifications,
        ntfy: on,
        ntfy_server: "https://ntfy.sh".into(),
        ntfy_topic: topic.into(),
        ntfy_token: String::new(),
        ..UserSettings::default()
    })
}

#[tokio::test]
async fn pusht_auch_im_modus_aus_ohne_systemmeldung() {
    let parts = with_ntfy(NotificationMode::Off, "harness-x", true);
    parts
        .service
        .on_session_change(&change("a", DisplayState::Waiting, DisplayState::Running))
        .await;
    assert!(parts.shown().is_empty());
    let pushed = parts.ntfy.pushed();
    assert_eq!(pushed.len(), 1);
    assert_eq!(pushed[0]["title"], "Session braucht deine Antwort");
}

#[tokio::test]
async fn pusht_im_modus_windows_zusaetzlich_zur_systemmeldung() {
    let parts = with_ntfy(NotificationMode::Windows, "harness-x", true);
    parts
        .service
        .on_session_change(&change("a", DisplayState::Waiting, DisplayState::Running))
        .await;
    assert_eq!(parts.shown().len(), 1);
    assert_eq!(parts.ntfy.pushed().len(), 1);
}

#[tokio::test]
async fn pusht_nicht_ohne_thema_oder_ausgeschaltet() {
    for (topic, on) in [("", true), ("harness-x", false)] {
        let parts = with_ntfy(NotificationMode::Off, topic, on);
        parts
            .service
            .on_session_change(&change("a", DisplayState::Waiting, DisplayState::Running))
            .await;
        assert!(parts.ntfy.pushed().is_empty(), "{topic}/{on}");
    }
}

#[tokio::test]
async fn meldet_bei_der_testmeldung_ein_fehlendes_thema() {
    let parts = with_ntfy(NotificationMode::Off, "", true);
    assert_eq!(
        parts.service.send_ntfy_test().await.as_deref(),
        Some("Für ntfy ist kein Thema eingetragen.")
    );
}
