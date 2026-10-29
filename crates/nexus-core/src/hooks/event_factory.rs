//! Bildet die Eingabe eines Claude-Code-Hooks auf ein normalisiertes Ereignis ab.
//! Portiert aus `packages/core/src/hooks/HookEventFactory.ts`.

use serde_json::Value;

use crate::sessions::{HookEvent, HookKind};

/// Erzeugt [`HookEvent`] aus der rohen Eingabe von Claude Code
pub struct HookEventFactory;

impl HookEventFactory {
    /// Liefert nichts, wenn die Eingabe unbrauchbar ist (keine Kennung, kein Objekt)
    pub fn create(
        kind: HookKind,
        payload: &Value,
        fallback_cwd: &str,
        tickets: Vec<String>,
    ) -> Option<HookEvent> {
        let payload = payload.as_object()?;
        let text = |name: &str| {
            payload
                .get(name)
                .and_then(Value::as_str)
                .filter(|wert| !wert.trim().is_empty())
                .map(str::to_string)
        };
        let session_id = text("session_id")?;
        Some(HookEvent {
            kind,
            session_id,
            cwd: slashes(&text("cwd").unwrap_or_else(|| fallback_cwd.to_string())),
            at: None,
            tickets: Some(tickets),
            agent_id: text("agent_id"),
            agent_type: text("agent_type"),
            notification_type: text("notification_type").or_else(|| text("type")),
            title: text("session_title"),
            model: text("model"),
            source: text("source"),
        })
    }
}

/// Schrägstriche statt Rückwärtsstrichen
fn slashes(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn bildet_die_eingabe_von_claude_code_auf_ein_normalisiertes_ereignis_ab() {
        let event = HookEventFactory::create(
            HookKind::SubagentStart,
            &json!({
                "session_id": "abc",
                "cwd": "D:\\Entwicklung\\Projekt",
                "agent_id": "a1",
                "agent_type": "Explore",
                "model": "m",
                "session_title": "Titel",
            }),
            "C:/fallback",
            vec!["T-0001".into()],
        )
        .unwrap();
        assert_eq!(
            event,
            HookEvent {
                kind: HookKind::SubagentStart,
                session_id: "abc".into(),
                cwd: "D:/Entwicklung/Projekt".into(),
                at: None,
                tickets: Some(vec!["T-0001".into()]),
                agent_id: Some("a1".into()),
                agent_type: Some("Explore".into()),
                notification_type: None,
                title: Some("Titel".into()),
                model: Some("m".into()),
                source: None,
            }
        );
    }

    #[test]
    fn liest_die_art_der_benachrichtigung_aus_notification_type_oder_type() {
        let art = |payload| {
            HookEventFactory::create(HookKind::Notification, &payload, "/x", Vec::new())
                .unwrap()
                .notification_type
        };
        assert_eq!(
            art(json!({ "session_id": "s", "notification_type": "permission_prompt" })),
            Some("permission_prompt".into())
        );
        assert_eq!(
            art(json!({ "session_id": "s", "type": "idle_prompt" })),
            Some("idle_prompt".into())
        );
    }

    #[test]
    fn ubernimmt_die_quelle_des_sessionstarts() {
        let quelle = |payload| {
            HookEventFactory::create(HookKind::SessionStart, &payload, "/x", Vec::new())
                .unwrap()
                .source
        };
        assert_eq!(
            quelle(json!({ "session_id": "s", "source": "resume" })),
            Some("resume".into())
        );
        assert_eq!(quelle(json!({ "session_id": "s" })), None);
    }

    #[test]
    fn nimmt_ohne_cwd_das_ausweichverzeichnis() {
        assert_eq!(
            HookEventFactory::create(
                HookKind::Stop,
                &json!({ "session_id": "s" }),
                "D:\\wt",
                Vec::new()
            )
            .unwrap()
            .cwd,
            "D:/wt"
        );
    }

    #[test]
    fn liefert_ohne_session_kennung_nichts() {
        for payload in [
            json!({}),
            json!({ "session_id": "" }),
            json!({ "session_id": 42 }),
        ] {
            assert!(HookEventFactory::create(HookKind::Stop, &payload, "/x", Vec::new()).is_none());
        }
    }

    #[test]
    fn ignoriert_eingaben_die_kein_objekt_sind() {
        for payload in [Value::Null, json!("text")] {
            assert!(HookEventFactory::create(HookKind::Stop, &payload, "/x", Vec::new()).is_none());
        }
    }

    #[test]
    fn weist_unbekannte_ereignisse_ab() {
        // In Rust gibt es keine unbekannte Art: der Name wird vorher zu [`HookKind`]
        assert_eq!(crate::sessions::HookKind::from_str_opt("quatsch"), None);
    }
}
