//! Zustandsspeicher aller Sessions. Portiert aus `packages/core/src/sessions/SessionStore.ts`.

use crate::clock::Clock;
use crate::error::{HarnessError, Result};

use super::session::{
    DisplayState, HookKind, Session, SessionClient, SessionState, SessionView, Subagent,
    SubagentState,
};

/// Ohne Meldung gilt eine Session nach einer halben Stunde als veraltet
pub const STALE_MS: i64 = 30 * 60 * 1000;

/// Nach sieben Tagen ohne Meldung wird eine Session vergessen
pub const RETENTION_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// Benachrichtigungen, die ein Warten auf den Benutzer bedeuten
const WAITING_NOTIFICATIONS: [&str; 2] = ["permission_prompt", "idle_prompt"];

/// Zustandsspeicher aller Sessions: wendet Hook-Ereignisse an und liefert Ansichten
/// (reine Logik, ohne Dateien).
#[derive(Debug)]
pub struct SessionStore {
    sessions: Vec<Session>,
    clock: Clock,
}

impl SessionStore {
    /// Speicher über diesem Bestand; die Uhr liefert den Zeitpunkt fehlender Angaben
    pub fn new(sessions: Vec<Session>, clock: Clock) -> Self {
        Self { sessions, clock }
    }

    /// Leerer Speicher mit dieser Uhr
    pub fn empty(clock: Clock) -> Self {
        Self::new(Vec::new(), clock)
    }

    /// Wendet ein Ereignis an (legt unbekannte Sessions an) und bereinigt alte
    pub fn apply(&mut self, project_id: &str, event: &super::session::HookEvent) -> Session {
        let at = event.at.clone().unwrap_or_else(|| self.clock.iso());
        let index = match self
            .sessions
            .iter()
            .position(|s| s.project_id == project_id && s.id == event.session_id)
        {
            Some(index) => index,
            None => {
                self.sessions.push(Session {
                    id: event.session_id.clone(),
                    project_id: project_id.to_string(),
                    tickets: Vec::new(),
                    cwd: event.cwd.clone(),
                    state: SessionState::Idle,
                    started_at: at.clone(),
                    last_event_at: at.clone(),
                    ended_at: None,
                    title: None,
                    model: None,
                    subagents: Vec::new(),
                    transcript_path: None,
                    client: None,
                    run_name: None,
                });
                self.sessions.len() - 1
            }
        };
        let session = &mut self.sessions[index];
        session.last_event_at = at.clone();
        if !event.cwd.is_empty() {
            session.cwd = event.cwd.clone();
        }
        if let Some(title) = event.title.clone() {
            session.title = Some(title);
        }
        if let Some(model) = event.model.clone() {
            session.model = Some(model);
        }
        for ticket in event.tickets.iter().flatten() {
            if !session.tickets.contains(ticket) {
                session.tickets.push(ticket.clone());
            }
        }
        take_origin(session, event);
        transition(session, event, &at);
        // Die zuletzt gemeldete Session ans Ende: so ordnet `list` Sessions mit gleichem Zeitpunkt
        // (Meldungen in derselben Millisekunde) nach der Reihenfolge der Meldungen
        let session = self.sessions.remove(index);
        self.sessions.push(session.clone());
        self.prune();
        session
    }

    /// Ordnet einer bekannten Session ein Ticket zu
    pub fn claim(
        &mut self,
        project_id: &str,
        session_id: &str,
        ticket_id: &str,
    ) -> Result<Session> {
        let session = self
            .sessions
            .iter_mut()
            .find(|s| s.project_id == project_id && s.id == session_id)
            .ok_or_else(|| {
                HarnessError::new(
                    "session-not-found",
                    format!("Session {session_id} gibt es nicht."),
                )
            })?;
        if !session.tickets.iter().any(|id| id == ticket_id) {
            session.tickets.push(ticket_id.to_string());
        }
        Ok(session.clone())
    }

    /// Ansichten, neueste Aktivität zuerst; ohne Projekt alle
    pub fn list(&self, project_id: Option<&str>) -> Vec<SessionView> {
        // Rückwärts, weil `apply` die zuletzt gemeldete Session ans Ende hängt: bei gleichem Zeitpunkt
        // steht sie so vorn (`sort_by` ist stabil)
        let mut views: Vec<SessionView> = self
            .sessions
            .iter()
            .rev()
            .filter(|s| project_id.is_none_or(|id| s.project_id == id))
            .map(|s| self.view(s))
            .collect();
        // `localeCompare` auf ISO-Zeitpunkten ist derselbe Vergleich wie auf den Zeichenketten
        views.sort_by(|a, b| b.last_event_at.cmp(&a.last_event_at));
        views
    }

    /// Sessions, die einem Ticket zugeordnet sind
    pub fn for_ticket(&self, project_id: &str, ticket_id: &str) -> Vec<SessionView> {
        self.list(Some(project_id))
            .into_iter()
            .filter(|s| s.tickets.iter().any(|id| id == ticket_id))
            .collect()
    }

    /// Entfernt Sessions, deren letzte Meldung länger als 7 Tage zurückliegt; liefert die Anzahl
    pub fn prune(&mut self) -> usize {
        let limit = self.now_ms() - RETENTION_MS;
        let before = self.sessions.len();
        self.sessions
            .retain(|s| millis(s.ended_at.as_deref().unwrap_or(&s.last_event_at)) >= limit);
        before - self.sessions.len()
    }

    /// Alle Sessions zum Speichern
    pub fn to_vec(&self) -> Vec<Session> {
        self.sessions.clone()
    }

    /// Jetzt in Millisekunden seit der Epoche
    fn now_ms(&self) -> i64 {
        self.clock.now().timestamp_millis()
    }

    /// Berechnet Anzeigezustand, Lauschen und Laufzeit
    fn view(&self, session: &Session) -> SessionView {
        let now = self.now_ms();
        let stale =
            session.state != SessionState::Ended && now - millis(&session.last_event_at) > STALE_MS;
        let display_state = if stale {
            DisplayState::Stale
        } else {
            session.state.into()
        };
        let end = session.ended_at.as_deref().map(millis).unwrap_or(now);
        SessionView {
            session: session.clone(),
            display_state,
            listening: matches!(
                display_state,
                DisplayState::Idle | DisplayState::Running | DisplayState::Waiting
            ),
            runtime_ms: (end - millis(&session.started_at)).max(0),
        }
    }
}

/// Übernimmt Transcript, Lauf und Quelle; leere Angaben überschreiben nichts, und eine Session
/// des Dispatchers (`nexus`) bleibt es
fn take_origin(session: &mut Session, event: &super::session::HookEvent) {
    let given = |value: &Option<String>| value.clone().filter(|text| !text.trim().is_empty());
    if let Some(path) = given(&event.transcript_path) {
        session.transcript_path = Some(path);
    }
    if let Some(run) = given(&event.run) {
        session.run_name = Some(run);
        session.client = Some(SessionClient::Nexus);
    }
    match event.client {
        Some(SessionClient::Unknown) | None => {}
        Some(client) if session.client != Some(SessionClient::Nexus) => {
            session.client = Some(client);
        }
        Some(_) => {}
    }
}

/// Zustandsübergang je Ereignis (siehe data-model.md)
fn transition(session: &mut Session, event: &super::session::HookEvent, at: &str) {
    match event.kind {
        HookKind::SessionStart => {
            session.state = SessionState::Idle;
            session.ended_at = None;
        }
        HookKind::UserPrompt => session.state = SessionState::Running,
        HookKind::Notification => {
            let art = event.notification_type.as_deref().unwrap_or_default();
            if WAITING_NOTIFICATIONS.contains(&art) && !is_idle_while_subagents_run(session, event)
            {
                session.state = SessionState::Waiting;
            }
        }
        HookKind::Stop => {
            session.state = SessionState::Idle;
            end_subagents(session, at);
        }
        HookKind::SubagentStart => {
            session.state = SessionState::Running;
            upsert_subagent(session, event, SubagentState::Running, at);
        }
        HookKind::SubagentStop => upsert_subagent(session, event, SubagentState::Ended, at),
        HookKind::SessionEnd => {
            session.state = SessionState::Ended;
            session.ended_at = Some(at.to_string());
            end_subagents(session, at);
        }
    }
}

/// Der Leerlauf-Hinweis ist kein Warten auf den Benutzer, solange Subagenten arbeiten
fn is_idle_while_subagents_run(session: &Session, event: &super::session::HookEvent) -> bool {
    event.notification_type.as_deref() == Some("idle_prompt")
        && session
            .subagents
            .iter()
            .any(|a| a.state == SubagentState::Running)
}

/// Legt einen Subagenten an oder setzt seinen Zustand
fn upsert_subagent(
    session: &mut Session,
    event: &super::session::HookEvent,
    state: SubagentState,
    at: &str,
) {
    let id = event.agent_id.clone().unwrap_or_else(|| {
        format!(
            "{}-{}",
            event.agent_type.as_deref().unwrap_or("agent"),
            session.subagents.len() + 1
        )
    });
    let index = match session.subagents.iter().position(|a| a.id == id) {
        Some(index) => index,
        None => {
            session.subagents.push(Subagent {
                id,
                kind: event
                    .agent_type
                    .clone()
                    .unwrap_or_else(|| "unbekannt".to_string()),
                state,
                started_at: at.to_string(),
                ended_at: None,
            });
            session.subagents.len() - 1
        }
    };
    let agent = &mut session.subagents[index];
    agent.state = state;
    if state == SubagentState::Ended {
        agent.ended_at = Some(at.to_string());
    }
}

/// Beendet alle noch laufenden Subagenten
fn end_subagents(session: &mut Session, at: &str) {
    for agent in &mut session.subagents {
        if agent.state == SubagentState::Running {
            agent.state = SubagentState::Ended;
            agent.ended_at = Some(at.to_string());
        }
    }
}

/// Millisekunden seit der Epoche eines ISO-Zeitpunkts; Unlesbares zählt als Epoche
fn millis(iso: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|at| at.timestamp_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::{DateTime, Duration, Utc};

    use super::super::session::HookEvent;
    use super::*;

    /// Ein Speicher mit steuerbarer Uhr
    struct Steuerbar {
        store: SessionStore,
        now: Arc<Mutex<DateTime<Utc>>>,
    }

    impl Steuerbar {
        fn neu(start: &str) -> Self {
            let now: Arc<Mutex<DateTime<Utc>>> =
                Arc::new(Mutex::new(start.parse().expect("der Zeitpunkt ist gültig")));
            let clock = Clock::from_fn({
                let now = Arc::clone(&now);
                move || *now.lock().expect("die Uhr ist nicht vergiftet")
            });
            Self {
                store: SessionStore::empty(clock),
                now,
            }
        }

        fn advance(&self, ms: i64) {
            let mut now = self.now.lock().expect("die Uhr ist nicht vergiftet");
            *now += Duration::milliseconds(ms);
        }
    }

    const P: &str = "proj1";

    fn clock() -> Steuerbar {
        Steuerbar::neu("2026-10-10T10:00:00.000Z")
    }

    /// Baut ein Ereignis; nicht genannte Felder haben Standardwerte
    fn ev(kind: HookKind) -> HookEvent {
        HookEvent::new(kind, "s1", "D:/wt/T-0042")
    }

    /// Dasselbe mit anderer Session-Kennung
    fn ev_id(kind: HookKind, session_id: &str) -> HookEvent {
        HookEvent::new(kind, session_id, "D:/wt/T-0042")
    }

    /// Eine Benachrichtigung dieser Art
    fn ev_note(art: &str) -> HookEvent {
        HookEvent {
            notification_type: Some(art.to_string()),
            ..ev(HookKind::Notification)
        }
    }

    /// Ein Subagenten-Ereignis
    fn ev_agent(kind: HookKind, agent_id: &str, agent_type: &str) -> HookEvent {
        HookEvent {
            agent_id: Some(agent_id.to_string()),
            agent_type: Some(agent_type.to_string()),
            ..ev(kind)
        }
    }

    #[test]
    fn legt_bei_session_start_eine_untatige_session_mit_tickets_und_verzeichnis_an() {
        let mut c = clock();
        let s = c.store.apply(
            P,
            &HookEvent {
                tickets: Some(vec!["T-0042".into()]),
                title: Some("Login".into()),
                model: Some("m".into()),
                ..ev(HookKind::SessionStart)
            },
        );
        assert_eq!(s.id, "s1");
        assert_eq!(s.project_id, P);
        assert_eq!(s.tickets, ["T-0042"]);
        assert_eq!(s.cwd, "D:/wt/T-0042");
        assert_eq!(s.state, SessionState::Idle);
        assert_eq!(s.title.as_deref(), Some("Login"));
        assert_eq!(s.model.as_deref(), Some("m"));
        assert_eq!(s.started_at, "2026-10-10T10:00:00.000Z");
    }

    #[test]
    fn wechselt_prompt_lauft_benachrichtigung_wartet_stop_untatig_ende_beendet() {
        let mut c = clock();
        c.store.apply(P, &ev(HookKind::SessionStart));
        c.advance(1000);
        assert_eq!(
            c.store.apply(P, &ev(HookKind::UserPrompt)).state,
            SessionState::Running
        );
        assert_eq!(
            c.store.apply(P, &ev_note("permission_prompt")).state,
            SessionState::Waiting
        );
        assert_eq!(
            c.store.apply(P, &ev(HookKind::UserPrompt)).state,
            SessionState::Running
        );
        assert_eq!(
            c.store.apply(P, &ev_note("idle_prompt")).state,
            SessionState::Waiting
        );
        assert_eq!(
            c.store.apply(P, &ev(HookKind::Stop)).state,
            SessionState::Idle
        );
        c.advance(5000);
        let ended = c.store.apply(P, &ev(HookKind::SessionEnd));
        assert_eq!(ended.state, SessionState::Ended);
        assert_eq!(ended.ended_at.as_deref(), Some("2026-10-10T10:00:06.000Z"));
    }

    #[test]
    fn lasst_den_zustand_bei_anderen_benachrichtigungen_unverandert() {
        let mut c = clock();
        c.store.apply(P, &ev(HookKind::UserPrompt));
        c.advance(60000);
        let s = c.store.apply(P, &ev_note("auth_success"));
        assert_eq!(s.state, SessionState::Running);
        assert_eq!(s.last_event_at, "2026-10-10T10:01:00.000Z");
    }

    #[test]
    fn legt_eine_unbekannte_session_bei_jedem_ereignis_an() {
        let mut c = clock();
        assert_eq!(
            c.store.apply(P, &ev_id(HookKind::UserPrompt, "neu")).state,
            SessionState::Running
        );
        assert_eq!(
            c.store.apply(P, &ev_id(HookKind::Stop, "noch-neu")).state,
            SessionState::Idle
        );
    }

    #[test]
    fn setzt_beim_erneuten_session_start_zuruck_ohne_den_beginn_zu_verlieren() {
        let mut c = clock();
        c.store.apply(P, &ev(HookKind::SessionStart));
        c.store.apply(P, &ev(HookKind::SessionEnd));
        c.advance(10000);
        let s = c.store.apply(P, &ev(HookKind::SessionStart));
        assert_eq!(s.state, SessionState::Idle);
        assert_eq!(s.ended_at, None);
        assert_eq!(s.started_at, "2026-10-10T10:00:00.000Z");
    }

    #[test]
    fn vereinigt_tickets_ohne_doppelungen() {
        let mut c = clock();
        c.store.apply(
            P,
            &HookEvent {
                tickets: Some(vec!["T-0001".into()]),
                ..ev(HookKind::SessionStart)
            },
        );
        let s = c.store.apply(
            P,
            &HookEvent {
                tickets: Some(vec!["T-0001".into(), "T-0002".into()]),
                ..ev(HookKind::UserPrompt)
            },
        );
        assert_eq!(s.tickets, ["T-0001", "T-0002"]);
    }

    #[test]
    fn halt_projekte_getrennt() {
        let mut c = clock();
        c.store.apply("a", &ev(HookKind::SessionStart));
        c.store.apply("b", &ev_id(HookKind::SessionStart, "s9"));
        assert_eq!(ids(&c.store.list(Some("a"))), ["s1"]);
        assert_eq!(ids(&c.store.list(Some("b"))), ["s9"]);
        assert_eq!(c.store.list(None).len(), 2);
    }

    /// Die Kennungen der Ansichten in ihrer Reihenfolge
    fn ids(views: &[SessionView]) -> Vec<&str> {
        views.iter().map(|s| s.id.as_str()).collect()
    }

    #[test]
    fn fuhrt_subagenten_mit_start_und_ende_und_setzt_die_session_auf_lauft() {
        let mut c = clock();
        c.store.apply(P, &ev(HookKind::SessionStart));
        c.advance(1000);
        let s = c.store.apply(
            P,
            &ev_agent(HookKind::SubagentStart, "a1", "general-purpose"),
        );
        assert_eq!(s.state, SessionState::Running);
        assert_eq!(
            s.subagents,
            [Subagent {
                id: "a1".into(),
                kind: "general-purpose".into(),
                state: SubagentState::Running,
                started_at: "2026-10-10T10:00:01.000Z".into(),
                ended_at: None,
            }]
        );
        c.advance(4000);
        let s = c.store.apply(
            P,
            &ev_agent(HookKind::SubagentStop, "a1", "general-purpose"),
        );
        assert_eq!(s.subagents[0].state, SubagentState::Ended);
        assert_eq!(
            s.subagents[0].ended_at.as_deref(),
            Some("2026-10-10T10:00:05.000Z")
        );
    }

    #[test]
    fn fuhrt_ein_subagenten_ende_ohne_start_als_beendeten_subagenten() {
        let mut c = clock();
        let s = c
            .store
            .apply(P, &ev_agent(HookKind::SubagentStop, "x", "Explore"));
        assert_eq!(s.subagents.len(), 1);
        assert_eq!(s.subagents[0].state, SubagentState::Ended);
    }

    #[test]
    fn bleibt_bei_lauft_wenn_ein_leerlauf_hinweis_kommt_wahrend_subagenten_arbeiten() {
        let mut c = clock();
        c.store
            .apply(P, &ev_agent(HookKind::SubagentStart, "a1", "t"));
        assert_eq!(
            c.store.apply(P, &ev_note("idle_prompt")).state,
            SessionState::Running
        );
        // Eine Rückfrage (Berechtigung) bleibt ein echtes Warten
        assert_eq!(
            c.store.apply(P, &ev_note("permission_prompt")).state,
            SessionState::Waiting
        );
        // Ohne aktive Subagenten gilt der Leerlauf-Hinweis wie bisher
        c.store
            .apply(P, &ev_agent(HookKind::SubagentStop, "a1", "t"));
        assert_eq!(
            c.store.apply(P, &ev_note("idle_prompt")).state,
            SessionState::Waiting
        );
    }

    #[test]
    fn beendet_laufende_subagenten_bei_stop_und_session_end() {
        let mut c = clock();
        c.store
            .apply(P, &ev_agent(HookKind::SubagentStart, "a1", "t"));
        assert_eq!(
            c.store.apply(P, &ev(HookKind::Stop)).subagents[0].state,
            SubagentState::Ended
        );
        c.store
            .apply(P, &ev_agent(HookKind::SubagentStart, "a2", "t"));
        assert!(c
            .store
            .apply(P, &ev(HookKind::SessionEnd))
            .subagents
            .iter()
            .all(|a| a.state == SubagentState::Ended));
    }

    #[test]
    fn berechnet_veraltet_nach_dreissig_minuten_ohne_meldung() {
        let mut c = clock();
        c.store.apply(P, &ev(HookKind::UserPrompt));
        c.store.apply(P, &ev_id(HookKind::SessionStart, "fertig"));
        c.store.apply(P, &ev_id(HookKind::SessionEnd, "fertig"));
        c.advance(STALE_MS + 1000);
        let views = c.store.list(Some(P));
        let s1 = views.iter().find(|s| s.id == "s1").unwrap();
        let fertig = views.iter().find(|s| s.id == "fertig").unwrap();
        assert_eq!(s1.display_state, DisplayState::Stale);
        assert_eq!(s1.state, SessionState::Running);
        assert_eq!(fertig.display_state, DisplayState::Ended);
    }

    #[test]
    fn meldet_listening_fur_untatige_laufende_und_wartende_sessions() {
        let mut c = clock();
        for (id, kind) in [
            ("i", HookKind::SessionStart),
            ("r", HookKind::UserPrompt),
            ("w", HookKind::Notification),
            ("e", HookKind::SessionEnd),
        ] {
            c.store.apply(
                P,
                &HookEvent {
                    notification_type: Some("permission_prompt".into()),
                    ..ev_id(kind, id)
                },
            );
        }
        let listening = |store: &SessionStore, id: &str| {
            store
                .list(Some(P))
                .into_iter()
                .find(|s| s.id == id)
                .expect("die Session wurde angelegt")
                .listening
        };
        assert_eq!(
            [
                listening(&c.store, "i"),
                listening(&c.store, "r"),
                listening(&c.store, "w"),
                listening(&c.store, "e"),
            ],
            [true, true, true, false]
        );
        c.advance(STALE_MS + 1000);
        assert!(!listening(&c.store, "r"));
    }

    #[test]
    fn berechnet_die_laufzeit_bis_zum_ende_oder_bis_jetzt() {
        let mut c = clock();
        c.store.apply(P, &ev(HookKind::SessionStart));
        c.advance(90000);
        assert_eq!(c.store.list(Some(P))[0].runtime_ms, 90000);
        c.store.apply(P, &ev(HookKind::SessionEnd));
        c.advance(60000);
        assert_eq!(c.store.list(Some(P))[0].runtime_ms, 90000);
    }

    #[test]
    fn sortiert_nach_letzter_aktivitat_neueste_zuerst() {
        let mut c = clock();
        c.store.apply(P, &ev_id(HookKind::SessionStart, "alt"));
        c.advance(1000);
        c.store.apply(P, &ev_id(HookKind::SessionStart, "neu"));
        assert_eq!(ids(&c.store.list(Some(P))), ["neu", "alt"]);
    }

    /// Meldungen in derselben Millisekunde (schneller Rechner): die zuletzt gemeldete steht vorn
    #[test]
    fn ordnet_bei_gleicher_zeit_die_zuletzt_gemeldete_session_zuerst() {
        let mut c = clock();
        c.store.apply(P, &ev_id(HookKind::SessionStart, "alt"));
        c.store.apply(P, &ev_id(HookKind::SessionStart, "neu"));
        assert_eq!(ids(&c.store.list(Some(P))), ["neu", "alt"]);

        c.store.apply(P, &ev_id(HookKind::UserPrompt, "alt"));
        assert_eq!(ids(&c.store.list(Some(P))), ["alt", "neu"]);
    }

    #[test]
    fn liefert_sessions_zu_einem_ticket() {
        let mut c = clock();
        c.store.apply(
            P,
            &HookEvent {
                tickets: Some(vec!["T-0001".into()]),
                ..ev(HookKind::SessionStart)
            },
        );
        c.store.apply(
            P,
            &HookEvent {
                tickets: Some(vec!["T-0002".into()]),
                ..ev_id(HookKind::SessionStart, "s2")
            },
        );
        assert_eq!(ids(&c.store.for_ticket(P, "T-0002")), ["s2"]);
        assert!(c.store.for_ticket(P, "T-0099").is_empty());
    }

    #[test]
    fn entfernt_sessions_nach_sieben_tagen_und_behalt_jungere() {
        let mut c = clock();
        c.store.apply(P, &ev_id(HookKind::SessionStart, "beendet"));
        c.store.apply(P, &ev_id(HookKind::SessionEnd, "beendet"));
        c.store.apply(P, &ev_id(HookKind::SessionStart, "stumm"));
        c.advance(RETENTION_MS - 1000);
        c.store.apply(P, &ev_id(HookKind::SessionStart, "frisch"));
        assert_eq!(c.store.prune(), 0);
        c.advance(2000);
        assert_eq!(c.store.prune(), 2);
        assert_eq!(ids(&c.store.list(Some(P))), ["frisch"]);
    }

    #[test]
    fn bereinigt_automatisch_bei_jedem_ereignis() {
        let mut c = clock();
        c.store.apply(P, &ev_id(HookKind::SessionStart, "alt"));
        c.advance(RETENTION_MS + 1000);
        c.store.apply(P, &ev_id(HookKind::SessionStart, "neu"));
        assert_eq!(
            c.store
                .to_vec()
                .iter()
                .map(|s| s.id.clone())
                .collect::<Vec<_>>(),
            ["neu"]
        );
    }

    #[test]
    fn ubernimmt_gespeicherte_sessions_und_gibt_sie_unverandert_zuruck() {
        let mut c = clock();
        c.store.apply(
            P,
            &HookEvent {
                tickets: Some(vec!["T-0001".into()]),
                ..ev(HookKind::SessionStart)
            },
        );
        let copy = SessionStore::new(
            c.store.to_vec(),
            Clock::fixed(
                "2026-10-10T10:05:00.000Z"
                    .parse()
                    .expect("der Zeitpunkt ist gültig"),
            ),
        );
        let view = &copy.list(Some(P))[0];
        assert_eq!(view.id, "s1");
        assert_eq!(view.tickets, ["T-0001"]);
    }

    #[test]
    fn ordnet_einer_bekannten_session_ein_ticket_zu() {
        let mut c = clock();
        c.store.apply(P, &ev(HookKind::SessionStart));
        assert_eq!(
            c.store.claim(P, "s1", "T-0042").unwrap().tickets,
            ["T-0042"]
        );
        assert_eq!(
            c.store.claim(P, "s1", "T-0042").unwrap().tickets,
            ["T-0042"]
        );
    }

    #[test]
    fn ubernimmt_transcript_und_quelle_und_uberschreibt_nicht_mit_leerem() {
        let mut c = clock();
        let s = c.store.apply(
            P,
            &HookEvent {
                transcript_path: Some("C:/t/s1.jsonl".into()),
                client: Some(SessionClient::ClaudeDesktop),
                ..ev(HookKind::SessionStart)
            },
        );
        assert_eq!(s.transcript_path.as_deref(), Some("C:/t/s1.jsonl"));
        assert_eq!(s.client, Some(SessionClient::ClaudeDesktop));
        assert_eq!(s.run_name, None);
        let s = c.store.apply(
            P,
            &HookEvent {
                transcript_path: Some("  ".into()),
                client: Some(SessionClient::Unknown),
                ..ev(HookKind::Stop)
            },
        );
        assert_eq!(s.transcript_path.as_deref(), Some("C:/t/s1.jsonl"));
        assert_eq!(s.client, Some(SessionClient::ClaudeDesktop));
    }

    #[test]
    fn eine_session_mit_lauf_bleibt_eine_session_von_nexus() {
        let mut c = clock();
        let s = c.store.apply(
            P,
            &HookEvent {
                run: Some("ARC-0001-20261104-091500.log".into()),
                client: Some(SessionClient::ClaudeCode),
                ..ev(HookKind::SessionStart)
            },
        );
        assert_eq!(s.client, Some(SessionClient::Nexus));
        assert_eq!(s.run_name.as_deref(), Some("ARC-0001-20261104-091500.log"));
        let s = c.store.apply(
            P,
            &HookEvent {
                client: Some(SessionClient::ClaudeDesktop),
                ..ev(HookKind::UserPrompt)
            },
        );
        assert_eq!(s.client, Some(SessionClient::Nexus));
        assert_eq!(s.run_name.as_deref(), Some("ARC-0001-20261104-091500.log"));
    }

    #[test]
    fn meldet_eine_unbekannte_session_verstandlich() {
        let mut c = clock();
        let error = c.store.claim(P, "gibtsnicht", "T-1").unwrap_err();
        assert_eq!(
            error.harness().map(|e| e.message.as_str()),
            Some("Session gibtsnicht gibt es nicht.")
        );
        assert_eq!(error.code(), Some("session-not-found"));
    }
}
