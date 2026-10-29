//! Systemmeldungen unter macOS und Linux über `notify-rust`.
//!
//! Gezeigt werden Titel und die beiden Textzeilen; das Toast-XML aus dem Server bleibt hier
//! ungenutzt, es gehört zu Windows. Ohne Meldungsdienst (z. B. kein D-Bus) kommt der
//! Fehlertext zurück, statt dass etwas abbricht.
//!
//! Unter macOS erscheint die Meldung ohne App-Bundle unter einem fremden Absender; das ist
//! für den Betrieb ohne die Nexus-App hinnehmbar.

use async_trait::async_trait;

use super::{Notifier, ToastRequest};

/// Systemmeldungen über den Meldungsdienst des Rechners
pub struct DesktopNotifier;

#[async_trait]
impl Notifier for DesktopNotifier {
    async fn show(&self, toast: &ToastRequest) -> Option<String> {
        let title = toast.title.clone();
        let body = toast.body.clone();
        #[cfg(target_os = "linux")]
        let icon = toast
            .icon
            .as_ref()
            .map(|icon| icon.to_string_lossy().to_string());
        // `notify-rust` blockiert (D-Bus bzw. das Mitteilungszentrum), deshalb im Blockier-Pool
        let shown = tokio::task::spawn_blocking(move || {
            let mut notification = notify_rust::Notification::new();
            notification.summary(&title).body(&body);
            #[cfg(target_os = "linux")]
            {
                notification.appname(super::TOAST_APP_NAME);
                if let Some(icon) = icon.as_deref() {
                    notification.icon(icon);
                }
            }
            notification.show().map(|_| ()).map_err(|e| e.to_string())
        })
        .await;
        match shown {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error),
            Err(error) => Some(error.to_string()),
        }
    }
}
