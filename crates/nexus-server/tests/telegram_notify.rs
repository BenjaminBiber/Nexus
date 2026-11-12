//! Telegram als dritter Meldeweg: welche Meldung mit welchen Knöpfen in die gekoppelten
//! Chats geht, welche Anlässe abgeschaltet sind und wann die Nachtruhe alles anhält.
//!
//! Kein Test spricht mit Telegram: die Bot-API ist die Attrappe aus
//! [`nexus_server::telegram::fake`].

mod common;

use chrono::{DateTime, Local, Utc};
use common::notify::{service, service_at, write_telegram, ServiceParts};
use nexus_core::actors::AuthorKind;
use nexus_core::board::question_types::{
    BoardQuestionOption, NotifiableBoardQuestion, NotifiableOrigin, NotifiableResearch,
};
use nexus_core::clock::Clock;
use nexus_core::settings::{NotificationMode, UserSettings};
use nexus_core::telegram::{QuietHours, TelegramConfig, TelegramNotify};
use nexus_server::dispatch::notifier::{DispatchNotice, DispatchNoticeKind};
use nexus_server::notifications::question_changes::QuestionChangeKind;
use nexus_server::notifications::QuestionChange;
use nexus_server::telegram::api::TelegramError;
use serde_json::Value;

/// Ein Token, das dem Muster von @BotFather entspricht
const TOKEN: &str = "123456:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw";

/// Der gekoppelte Chat der Tests
const CHAT: i64 = 4711;

/// Zeitpunkt der Tests (derselbe wie in der Uhr des Dienstes)
fn now() -> DateTime<Utc> {
    common::notify::now()
}

/// Der Zeitpunkt, an dem es am Tag von [`now`] örtlich `hour` Uhr ist
fn local_at(hour: u32) -> DateTime<Utc> {
    let tag = now().with_timezone(&Local).date_naive();
    tag.and_hms_opt(hour, 0, 0)
        .expect("Uhrzeit")
        .and_local_timezone(Local)
        .single()
        .expect("eindeutige Ortszeit")
        .with_timezone(&Utc)
}

/// Eine eingeschaltete Einstellung mit gekoppeltem Chat
fn config() -> TelegramConfig {
    TelegramConfig {
        enabled: true,
        bot_token: TOKEN.to_string(),
        chats: vec![CHAT],
        ..TelegramConfig::default()
    }
}

/// Ein Dienst, der nur über Telegram meldet (System aus, ntfy aus)
fn telegram_service(config: &TelegramConfig) -> ServiceParts {
    let parts = service(UserSettings {
        notifications: NotificationMode::Off,
        ..UserSettings::default()
    });
    write_telegram(&parts.home, config);
    parts
}

/// Wie [`telegram_service`], mit einer Uhr auf diesem Zeitpunkt
fn telegram_service_at(config: &TelegramConfig, at: DateTime<Utc>) -> ServiceParts {
    let parts = service_at(
        UserSettings {
            notifications: NotificationMode::Off,
            ..UserSettings::default()
        },
        Clock::fixed(at),
    );
    write_telegram(&parts.home, config);
    parts
}

/// Eine Meldung des Dispatchers dieser Art
fn notice(kind: DispatchNoticeKind, text: &str) -> DispatchNotice {
    DispatchNotice {
        kind,
        ticket_id: Some("NXS-0042".to_string()),
        title: "Login bauen".to_string(),
        text: text.to_string(),
        repo_path: None,
    }
}

/// Eine neue Frage mit diesen Optionen in der Recherche
fn change(options: Vec<BoardQuestionOption>, kind: QuestionChangeKind) -> QuestionChange {
    let research = match options.is_empty() {
        true => Vec::new(),
        false => vec![NotifiableResearch {
            kind: AuthorKind::Session,
            options,
        }],
    };
    QuestionChange {
        project_id: "p1".to_string(),
        project_name: "webshop".to_string(),
        kind,
        question: NotifiableBoardQuestion {
            id: "ARC-F0001".to_string(),
            title: "Welcher Speicher?".to_string(),
            occasion: Some("NXS-0042".to_string()),
            origin: NotifiableOrigin {
                kind: AuthorKind::Session,
            },
            research,
            project_key: "ARC".to_string(),
            project_name: "Arches".to_string(),
            repo_paths: Vec::new(),
        },
        route: None,
    }
}

/// Eine Option einer Recherche
fn option(id: &str) -> BoardQuestionOption {
    BoardQuestionOption {
        id: id.to_string(),
        title: format!("Vorschlag {id}"),
        text: "t".to_string(),
        pros: Vec::new(),
        cons: Vec::new(),
        recommended: false,
    }
}

/// Die Parameter aller `sendMessage`-Aufrufe der Attrappe
fn sent(parts: &ServiceParts) -> Vec<Value> {
    parts
        .telegram
        .calls_of("sendMessage")
        .into_iter()
        .map(|call| call.params)
        .collect()
}

/// Die Beschriftungen der Knöpfe eines `sendMessage`
fn labels(params: &Value) -> Vec<String> {
    params["reply_markup"]["inline_keyboard"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_array)
                .flatten()
                .map(|button| button["text"].as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn eine_review_meldung_kommt_mit_abnahme_knoepfen() {
    let parts = telegram_service(&config());

    parts
        .service
        .on_dispatch(
            "p1",
            &notice(DispatchNoticeKind::Review, "wartet auf Freigabe"),
        )
        .await;

    let sent = sent(&parts);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["chat_id"], CHAT);
    let text = sent[0]["text"].as_str().expect("Text");
    assert!(
        text.contains("<b>Dispatcher: wartet auf Review</b>"),
        "{text}"
    );
    assert!(text.contains("NXS-0042 Login bauen"), "{text}");
    assert_eq!(
        labels(&sent[0]),
        ["Übernehmen", "Zurück an den Agenten", "Geänderte Dateien"]
    );
}

#[tokio::test]
async fn ein_gescheiterter_merge_kommt_mit_merge_knoepfen() {
    let parts = telegram_service(&config());

    parts
        .service
        .on_dispatch(
            "p1",
            &notice(DispatchNoticeKind::Blocked, "Merge: Konflikt in src/a.rs"),
        )
        .await;

    assert_eq!(labels(&sent(&parts)[0]), ["Erneut mergen", "Grund"]);
}

#[tokio::test]
async fn ein_blockiertes_ticket_kommt_mit_blockiert_knoepfen() {
    let parts = telegram_service(&config());

    parts
        .service
        .on_dispatch("p1", &notice(DispatchNoticeKind::Blocked, "Zugang fehlt"))
        .await;

    assert_eq!(
        labels(&sent(&parts)[0]),
        ["Grund", "Kommentar schreiben", "Zurück nach Geplant"]
    );
}

#[tokio::test]
async fn eine_neue_frage_und_eine_fertige_recherche_kommen_mit_fragen_knoepfen() {
    let parts = telegram_service(&config());

    parts
        .service
        .on_question_change(&change(Vec::new(), QuestionChangeKind::FrageNeu))
        .await;
    parts
        .service
        .on_question_change(&change(
            vec![option("O1"), option("O2")],
            QuestionChangeKind::Recherche,
        ))
        .await;

    let sent = sent(&parts);
    assert_eq!(sent.len(), 2);
    assert_eq!(
        labels(&sent[0]),
        ["Eigener Text", "Recherche starten", "Verwerfen"]
    );
    assert_eq!(
        labels(&sent[1]),
        [
            "O1 · Vorschlag O1",
            "O2 · Vorschlag O2",
            "Eigener Text",
            "Recherche starten",
            "Verwerfen",
        ]
    );
    assert!(
        sent[1]["text"]
            .as_str()
            .expect("Text")
            .contains("Recherche ist da"),
        "{:?}",
        sent[1]
    );
}

#[tokio::test]
async fn merged_und_paused_kommen_nur_wenn_sie_eingeschaltet_sind() {
    let aus = telegram_service(&config());
    for kind in [DispatchNoticeKind::Merged, DispatchNoticeKind::Paused] {
        aus.service.on_dispatch("p1", &notice(kind, "egal")).await;
    }
    assert!(sent(&aus).is_empty());

    let an = telegram_service(&TelegramConfig {
        notify: TelegramNotify {
            merged: true,
            paused: true,
            ..TelegramNotify::default()
        },
        ..config()
    });
    for kind in [DispatchNoticeKind::Merged, DispatchNoticeKind::Paused] {
        an.service.on_dispatch("p1", &notice(kind, "egal")).await;
    }
    let sent = sent(&an);
    assert_eq!(sent.len(), 2);
    assert!(labels(&sent[0]).is_empty(), "als Info ohne Knöpfe");
}

#[tokio::test]
async fn ein_abgeschalteter_anlass_schickt_nichts() {
    let parts = telegram_service(&TelegramConfig {
        notify: TelegramNotify {
            review: false,
            ..TelegramNotify::default()
        },
        ..config()
    });

    parts
        .service
        .on_dispatch("p1", &notice(DispatchNoticeKind::Review, "wartet"))
        .await;

    assert!(sent(&parts).is_empty());
}

#[tokio::test]
async fn ohne_gekoppelten_chat_und_mit_ausgeschaltetem_bot_geht_nichts_hinaus() {
    for config in [
        TelegramConfig {
            chats: Vec::new(),
            ..config()
        },
        TelegramConfig {
            enabled: false,
            ..config()
        },
        TelegramConfig {
            bot_token: String::new(),
            ..config()
        },
    ] {
        let parts = telegram_service(&config);
        parts
            .service
            .on_dispatch("p1", &notice(DispatchNoticeKind::Review, "wartet"))
            .await;
        assert!(sent(&parts).is_empty(), "{:?}", config.chats);
    }
}

#[tokio::test]
async fn in_der_nachtruhe_geht_nichts_hinaus_auch_ueber_mitternacht() {
    // Die Nachtruhe geht nach Ortszeit; 22:00 bis 07:00 liegt über Mitternacht
    let ruhe = QuietHours {
        from: "22:00".to_string(),
        to: "07:00".to_string(),
    };
    let parts = telegram_service_at(
        &TelegramConfig {
            quiet_hours: Some(ruhe.clone()),
            ..config()
        },
        local_at(2),
    );

    parts
        .service
        .on_dispatch("p1", &notice(DispatchNoticeKind::Review, "wartet"))
        .await;

    assert!(sent(&parts).is_empty(), "02:00 liegt in der Nachtruhe");

    // Mittags geht dieselbe Meldung hinaus
    let wach = telegram_service_at(
        &TelegramConfig {
            quiet_hours: Some(ruhe),
            ..config()
        },
        local_at(12),
    );
    wach.service
        .on_dispatch("p1", &notice(DispatchNoticeKind::Review, "wartet"))
        .await;
    assert_eq!(sent(&wach).len(), 1);
}

#[tokio::test]
async fn ein_sendefehler_haelt_system_meldung_und_ntfy_nicht_auf() {
    let parts = service(UserSettings {
        notifications: NotificationMode::Windows,
        ntfy: true,
        ntfy_topic: "nexus".to_string(),
        ..UserSettings::default()
    });
    write_telegram(&parts.home, &config());
    parts.telegram.fail(
        "sendMessage",
        TelegramError::Api {
            code: 400,
            description: "Bad Request".to_string(),
        },
    );

    parts
        .service
        .on_dispatch("p1", &notice(DispatchNoticeKind::Review, "wartet"))
        .await;

    assert_eq!(parts.shown().len(), 1, "die Systemmeldung kommt trotzdem");
    assert_eq!(parts.ntfy.pushed().len(), 1, "ntfy bekommt sie auch");
    assert_eq!(sent(&parts).len(), 1, "Telegram wurde versucht");
}

#[tokio::test]
async fn telegram_meldet_auch_wenn_system_und_ntfy_aus_sind() {
    let parts = telegram_service(&config());

    parts
        .service
        .on_dispatch("p1", &notice(DispatchNoticeKind::Review, "wartet"))
        .await;

    assert!(parts.shown().is_empty());
    assert_eq!(sent(&parts).len(), 1);
}
