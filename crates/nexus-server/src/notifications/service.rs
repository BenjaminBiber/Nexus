//! Entscheidet, ob eine Meldung erscheint, und verteilt sie auf Tab, System und ntfy;
//! portiert aus `packages/server/src/notifications/NotificationService.ts`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

use nexus_core::board::question_types::NotifiableBoardQuestion;
use nexus_core::sessions::session::{DisplayState, SessionView};
use nexus_core::settings::{NotificationMode, UserSettings, UserSettingsStore};
use nexus_platform::{Notifier, ToastRequest};

use crate::dispatch::notifier::DispatchNotice;
use crate::log::ServerLog;
use crate::notices::{BrowserNotice, BrowserNoticeHub};
use crate::notifications::attention::{AttentionKind, AttentionTracker};
use crate::notifications::model::{ToastModel, ToastOptions};
use crate::notifications::ntfy::{NtfySender, NtfyTarget};
use crate::notifications::question_changes::QuestionChangeKind;
use crate::notifications::texts::{QuestionInput, SingleInput, TicketRef, ToastTexts};
use crate::notifications::toast_xml::ToastXmlBuilder;

/// Dateiname des App-Symbols in den Meldungsbildern
pub const APP_ICON: &str = "logo-app.png";

/// Ein Zustandswechsel einer Session
#[derive(Debug, Clone)]
pub struct SessionChange {
    pub project_id: String,
    pub project_name: String,
    pub before: Option<DisplayState>,
    pub view: SessionView,
    pub ticket: Option<TicketRef>,
}

/// Eine neue Frage oder eine neue Recherche
#[derive(Debug, Clone)]
pub struct QuestionChange {
    /// Harness-Projekt der Adresse; leer: Startseite der Oberfläche
    pub project_id: String,
    pub project_name: String,
    pub kind: QuestionChangeKind,
    pub question: NotifiableBoardQuestion,
    /// Reiter der Fragen der Board-Datenbank
    pub route: Option<String>,
}

/// Entscheidet bei Zustandswechseln, ob eine Systemmeldung erscheint, und zeigt sie; ist ntfy
/// eingeschaltet, geht dieselbe Meldung zusätzlich als Push raus (unabhängig von der Art der
/// Meldung).
pub struct NotificationService {
    settings: UserSettingsStore,
    notifier: Arc<dyn Notifier>,
    assets_dir: PathBuf,
    host: String,
    /// Der öffentliche Port; bei `--port 0` steht er erst beim Start fest
    port: AtomicU16,
    log: ServerLog,
    tabs: Option<Arc<BrowserNoticeHub>>,
    ntfy: NtfySender,
    tracker: Mutex<AttentionTracker>,
    texts: ToastTexts,
    xml: ToastXmlBuilder,
}

/// Alles, was der Dienst zum Melden braucht (ein eigener Typ, damit die Aufrufstelle
/// benennt, was sie übergibt)
pub struct NotificationSetup {
    pub settings: UserSettingsStore,
    pub notifier: Arc<dyn Notifier>,
    /// Ordner mit den Bildern der Meldung
    pub assets_dir: PathBuf,
    pub host: String,
    /// Der öffentliche Port; bei `--port 0` wird er mit [`NotificationService::set_port`]
    /// nachgetragen
    pub port: u16,
    pub log: ServerLog,
    /// Offene Tabs mit `notify=1`; `None` = immer der Weg des Betriebssystems
    pub tabs: Option<Arc<BrowserNoticeHub>>,
    pub ntfy: NtfySender,
}

impl NotificationService {
    /// Dienst mit allem, was er zum Melden braucht
    pub fn new(setup: NotificationSetup) -> Self {
        Self {
            settings: setup.settings,
            notifier: setup.notifier,
            assets_dir: setup.assets_dir,
            host: setup.host,
            port: AtomicU16::new(setup.port),
            log: setup.log,
            tabs: setup.tabs,
            ntfy: setup.ntfy,
            tracker: Mutex::new(AttentionTracker::new()),
            texts: ToastTexts,
            xml: ToastXmlBuilder,
        }
    }

    /// Trägt den wirklich gehörten Port nach (`--port 0`); Links in Meldungen zeigen dorthin
    pub fn set_port(&self, port: u16) {
        self.port.store(port, Ordering::Relaxed);
    }

    /// Basis-Adresse der Links in Meldungen
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port.load(Ordering::Relaxed))
    }

    /// Verarbeitet eine Zustandsänderung; gezeigt wird nur im Modus „windows“ oder mit ntfy
    pub async fn on_session_change(&self, change: &SessionChange) {
        let label = change
            .ticket
            .as_ref()
            .map(|ticket| ticket.id.clone())
            .or_else(|| change.view.title.clone())
            .unwrap_or_else(|| change.project_name.clone());
        // Die Sperre endet vor dem ersten `await`: der Zustand ist nur ein Gedächtnis
        let (kind, pending) = {
            let mut tracker = self.tracker.lock().expect("Sperre der Sessions");
            match tracker.observe(change.before, &change.view, &label) {
                Some(kind) => (kind, tracker.list()),
                None => return,
            }
        };
        let settings = self.settings.read();
        if !self.wanted(&settings) {
            return;
        }
        let base = self.base_url();
        let model = if pending.len() >= 2 {
            self.texts.collected(
                &pending,
                &format!("{base}/p/{}/sessions", change.project_id),
            )
        } else {
            self.texts.single(SingleInput {
                kind,
                project_name: &change.project_name,
                ticket: change.ticket.as_ref(),
                session_title: change.view.title.as_deref(),
                session_model: change.view.model.as_deref(),
                open_url: &self.ticket_url(
                    &change.project_id,
                    change.ticket.as_ref().map(|ticket| ticket.id.as_str()),
                ),
            })
        };
        self.deliver(&model, &settings, None).await;
    }

    /// Meldet eine neue Frage oder Recherche eines Agenten; gezeigt wird nur im Modus
    /// „windows“ oder mit ntfy
    pub async fn on_question_change(&self, change: &QuestionChange) {
        let settings = self.settings.read();
        if !self.wanted(&settings) {
            return;
        }
        let question = &change.question;
        let base = self.base_url();
        let open_url = match change.project_id.is_empty() {
            true => base.clone(),
            false => format!(
                "{base}/p/{}/{}/{}",
                change.project_id,
                change.route.as_deref().unwrap_or("questions"),
                question.id
            ),
        };
        let input = QuestionInput {
            project_name: &change.project_name,
            question_id: &question.id,
            question_title: &question.title,
            occasion: question.occasion.as_deref(),
            research_count: question.research.len(),
            open_url: &open_url,
        };
        let latest = question.research.last();
        let model = match (change.kind, latest) {
            (QuestionChangeKind::Recherche, Some(latest)) => {
                self.texts.research(&input, &latest.options)
            }
            _ => self.texts.question_new(&input),
        };
        let project = match change.project_id.is_empty() {
            true => "db",
            false => &change.project_id,
        };
        self.deliver(
            &model,
            &settings,
            Some(format!("question-{project}-{}", question.id)),
        )
        .await;
    }

    /// Meldet ein Ereignis des Dispatchers; gezeigt wird nur im Modus „windows“ oder mit ntfy
    pub async fn on_dispatch(&self, project_id: &str, notice: &DispatchNotice) {
        let settings = self.settings.read();
        if !self.wanted(&settings) {
            return;
        }
        let open_url = match project_id.is_empty() {
            true => self.base_url(),
            false => self.ticket_url(project_id, notice.ticket_id.as_deref()),
        };
        let tag = format!(
            "dispatch-{}",
            notice
                .ticket_id
                .clone()
                .unwrap_or_else(|| notice.kind.as_str().to_string())
        );
        let model = self.texts.dispatch(notice, &open_url);
        self.deliver(&model, &settings, Some(tag)).await;
    }

    /// Zeigt eine Beispielmeldung (Schaltfläche „Testmeldung“); liefert den Fehlertext oder
    /// `None`
    pub async fn send_test(&self) -> Option<String> {
        let settings = self.settings.read();
        self.show(&self.example(), settings.hero_images, None).await
    }

    /// Schickt die Beispielmeldung an ntfy; liefert den Fehlertext oder `None`
    pub async fn send_ntfy_test(&self) -> Option<String> {
        let settings = self.settings.read();
        if settings.ntfy_topic.is_empty() {
            return Some("Für ntfy ist kein Thema eingetragen.".to_string());
        }
        self.push(&self.example(), &settings).await
    }

    /// Meldung der Testschaltflächen
    fn example(&self) -> ToastModel {
        let ticket = TicketRef {
            id: "ARC-0042".to_string(),
            title: "Favoriten".to_string(),
        };
        self.texts.single(SingleInput {
            kind: AttentionKind::Wartet,
            project_name: "Beispielprojekt",
            ticket: Some(&ticket),
            session_title: Some("Testmeldung des Harness"),
            session_model: None,
            open_url: &self.base_url(),
        })
    }

    /// Ob überhaupt gemeldet wird: Modus „windows“ oder eingeschaltetes ntfy mit Thema
    fn wanted(&self, settings: &UserSettings) -> bool {
        settings.notifications == NotificationMode::Windows || Self::ntfy_on(settings)
    }

    /// Ist ntfy eingeschaltet und ein Thema eingetragen?
    fn ntfy_on(settings: &UserSettings) -> bool {
        settings.ntfy && !settings.ntfy_topic.is_empty()
    }

    /// Verteilt die Meldung auf die eingeschalteten Wege (System bzw. Tab, ntfy)
    async fn deliver(&self, model: &ToastModel, settings: &UserSettings, tag: Option<String>) {
        let system = async {
            if settings.notifications == NotificationMode::Windows {
                self.show(model, settings.hero_images, tag.clone()).await;
            }
        };
        let push = async {
            if Self::ntfy_on(settings) {
                self.push(model, settings).await;
            }
        };
        tokio::join!(system, push);
    }

    /// Zeigt die Meldung in einem offenen Tab, sonst baut es das XML und übergibt es dem
    /// System
    async fn show(
        &self,
        model: &ToastModel,
        hero_images: bool,
        tag: Option<String>,
    ) -> Option<String> {
        let tag = tag.unwrap_or_else(|| nexus_platform::notifier::DEFAULT_TAG.to_string());
        if let Some(tabs) = &self.tabs {
            if tabs.deliver(self.notice(model, &tag)) {
                return None;
            }
        }
        let xml = self.xml.build(
            model,
            &ToastOptions {
                assets_dir: self.assets_dir.clone(),
                hero_images,
            },
        );
        let error = self
            .notifier
            .show(&ToastRequest {
                title: model.title.clone(),
                body: body_of(model),
                url: Some(model.open_url.clone()),
                tag: Some(tag),
                xml: Some(xml),
                icon: Some(self.assets_dir.join(APP_ICON)),
            })
            .await;
        if let Some(error) = &error {
            self.log
                .write(&format!("Windows-Benachrichtigung fehlgeschlagen: {error}"));
        }
        error
    }

    /// Schickt die Meldung an ntfy; Fehler landen im Protokoll
    async fn push(&self, model: &ToastModel, settings: &UserSettings) -> Option<String> {
        let error = self
            .ntfy
            .send(
                &NtfyTarget {
                    server: settings.ntfy_server.clone(),
                    topic: settings.ntfy_topic.clone(),
                    token: settings.ntfy_token.clone(),
                },
                model,
            )
            .await;
        if let Some(error) = &error {
            self.log
                .write(&format!("ntfy-Benachrichtigung fehlgeschlagen: {error}"));
        }
        error
    }

    /// Meldung für einen Tab: Text in zwei Zeilen, Adresse als Pfad der Oberfläche
    fn notice(&self, model: &ToastModel, tag: &str) -> BrowserNotice {
        let base = self.base_url();
        let url = match model.open_url.strip_prefix(&base) {
            Some("") | None => "/".to_string(),
            Some(path) => path.to_string(),
        };
        BrowserNotice {
            kind: model.kind,
            tag: tag.to_string(),
            title: model.title.clone(),
            body: body_of(model),
            url,
        }
    }

    /// Adresse des Tickets im Panel des DB-Boards, ohne Ticket die Sessions des Projekts
    fn ticket_url(&self, project_id: &str, ticket_id: Option<&str>) -> String {
        let base = format!("{}/p/{project_id}", self.base_url());
        match ticket_id {
            Some(ticket) => format!("{base}/board/t/{}", encode_segment(ticket)),
            None => format!("{base}/sessions"),
        }
    }
}

/// Nimmt die Meldungen der Fragen-Beobachtung an; ihr Fehler hält die übrigen nicht auf
#[async_trait::async_trait]
impl crate::notifications::db_question_watch::DbQuestionSink for NotificationService {
    async fn on_question_change(&self, change: &QuestionChange) -> nexus_core::Result<()> {
        NotificationService::on_question_change(self, change).await;
        Ok(())
    }
}

/// Der Text der Meldung: die beiden Zeilen, leere fallen weg
fn body_of(model: &ToastModel) -> String {
    [model.line1.as_str(), model.line2.as_str()]
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ein Pfadstück wie `encodeURIComponent` in JavaScript
fn encode_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => encoded.push(*byte as char),
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}
