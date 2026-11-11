//! Systemmeldungen (Toasts) auf diesem Rechner; portiert aus
//! `packages/server/src/notifications/WindowsToastSender.ts`.
//!
//! Unter Windows geht die Meldung über WinRT (kein PowerShell-Kindprozess mehr), unter macOS
//! und Linux über `notify-rust`. Was in der Meldung steht, entscheidet der Server: er baut
//! Titel, Text und – für Windows – das fertige Toast-XML und übergibt beides als
//! [`ToastRequest`].
//!
//! Abweichung vom Migrationsplan: dort trägt `ToastRequest` das Toast-Modell samt Bildordner,
//! und der Windows-Notifier baut das XML selbst. Das XML entsteht hier schon im Server
//! (`nexus_server::notifications::toast_xml`), weil das Modell und seine Texte zum Fachkern
//! der Benachrichtigungen gehören und dieses Crate sonst von `nexus-server` abhinge.

pub mod fake;

#[cfg(not(windows))]
pub mod desktop;
#[cfg(windows)]
pub mod windows;

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Kennung der App in Windows; unter ihr erscheint die Meldung im Info-Center
pub const TOAST_APP_ID: &str = "AI.Harness";

/// Anzeigename der App in Windows
pub const TOAST_APP_NAME: &str = "Nexus";

/// Gruppe, in der Windows die Meldungen von Nexus zusammenfasst
pub const TOAST_GROUP: &str = "harness";

/// Kennung, unter der sich Meldungen gegenseitig ersetzen, wenn keine angegeben ist
pub const DEFAULT_TAG: &str = "attention";

/// Eine Systemmeldung: Titel, Text, Ziel beim Klick und die Gruppe, in der sich Meldungen
/// gegenseitig ersetzen
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ToastRequest {
    pub title: String,
    pub body: String,
    /// Adresse, die der Klick öffnet (leer: nichts öffnen)
    pub url: Option<String>,
    /// Gleiche Kennung ersetzt eine frühere Meldung (Standard [`DEFAULT_TAG`])
    pub tag: Option<String>,
    /// Fertiges Toast-XML für Windows; ohne das zeigt Windows nichts
    pub xml: Option<String>,
    /// Symbol der Meldung (`logo-app.png`): unter Windows das Symbol der App-Registrierung,
    /// unter Linux das Bild der Meldung
    pub icon: Option<PathBuf>,
}

impl ToastRequest {
    /// Die Kennung der Meldung, sonst [`DEFAULT_TAG`]
    pub fn tag(&self) -> &str {
        self.tag.as_deref().unwrap_or(DEFAULT_TAG)
    }
}

/// Systemmeldung zeigen; `None` = gezeigt, `Some(text)` = nicht gezeigt mit Grund
#[async_trait]
pub trait Notifier: Send + Sync {
    async fn show(&self, toast: &ToastRequest) -> Option<String>;
}

/// Zeigt keine Systemmeldungen (Vertragsmodus und Rechner ohne Meldungsdienst)
pub struct SilentNotifier;

#[async_trait]
impl Notifier for SilentNotifier {
    async fn show(&self, _toast: &ToastRequest) -> Option<String> {
        Some("Systemmeldungen gibt es auf diesem Rechner nicht.".into())
    }
}

/// Name der Umgebungsvariable, die statt echter Meldungen die Attrappe einsetzt
pub const NOTIFIER_ENV: &str = "NEXUS_NOTIFIER";

/// Die Systemmeldungen dieses Rechners: WinRT unter Windows, `notify-rust` sonst.
/// Mit `NEXUS_NOTIFIER=fake` wird nichts gezeigt, sondern nur aufgezeichnet (Vertragslauf).
pub fn local() -> Box<dyn Notifier> {
    from_env(&|name| std::env::var(name).ok())
}

/// Wie [`local`], mit frei wählbarer Umgebung (Tests)
pub fn from_env(env: &dyn Fn(&str) -> Option<String>) -> Box<dyn Notifier> {
    if env(NOTIFIER_ENV).is_some_and(|value| value.trim() == "fake") {
        return Box::new(fake::FakeNotifier::new());
    }
    #[cfg(windows)]
    {
        Box::new(windows::WindowsNotifier)
    }
    #[cfg(not(windows))]
    {
        Box::new(desktop::DesktopNotifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_kennung_einer_meldung_fehlt_nie() {
        assert_eq!(ToastRequest::default().tag(), DEFAULT_TAG);
        assert_eq!(
            ToastRequest {
                tag: Some("question-db-ARC-F0001".into()),
                ..ToastRequest::default()
            }
            .tag(),
            "question-db-ARC-F0001"
        );
    }

    #[tokio::test]
    async fn nexus_notifier_fake_zeigt_nichts_und_zeichnet_nur_auf() {
        // Der Vertragslauf darf keine echte Meldung auslösen
        let notifier = from_env(&|name| match name {
            NOTIFIER_ENV => Some(" fake ".to_string()),
            _ => None,
        });
        assert_eq!(notifier.show(&ToastRequest::default()).await, None);
    }
}
