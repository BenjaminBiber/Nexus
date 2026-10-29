//! Systemmeldungen unter Windows über WinRT (`Windows.UI.Notifications`).
//!
//! Ersetzt den PowerShell-Weg aus `WindowsToastSender.ts`: kein Kindprozess mehr, dafür
//! derselbe Ablauf – „AI Harness“ als App anmelden (AUMID in der Registrierung), das
//! Toast-XML laden, `Tag` und `Group` setzen und die Meldung zeigen.

use async_trait::async_trait;
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows_registry::CURRENT_USER;

use super::{Notifier, ToastRequest, TOAST_APP_ID, TOAST_APP_NAME, TOAST_GROUP};

/// Schlüssel, unter dem Windows Name und Symbol der App nachschlägt
const AUMID_KEY: &str = concat!(r"Software\Classes\AppUserModelId\", "AI.Harness");

/// Systemmeldungen über WinRT
pub struct WindowsNotifier;

#[async_trait]
impl Notifier for WindowsNotifier {
    async fn show(&self, toast: &ToastRequest) -> Option<String> {
        let Some(xml) = toast.xml.clone() else {
            return Some("Für die Systemmeldung fehlt das Toast-XML.".into());
        };
        let icon = toast
            .icon
            .as_ref()
            .map(|icon| icon.to_string_lossy().to_string())
            .unwrap_or_default();
        let tag = toast.tag().to_string();
        // WinRT blockiert; deshalb im Blockier-Pool statt im Ereignis-Pool von tokio
        match tokio::task::spawn_blocking(move || display(&xml, &icon, &tag)).await {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error),
            Err(error) => Some(error.to_string()),
        }
    }
}

/// Meldet die App an und zeigt die Meldung; Fehler als Text
fn display(xml: &str, icon: &str, tag: &str) -> Result<(), String> {
    register(icon)?;
    let document = XmlDocument::new().map_err(text)?;
    document.LoadXml(&xml.into()).map_err(text)?;
    let toast = ToastNotification::CreateToastNotification(&document).map_err(text)?;
    toast.SetTag(&tag.into()).map_err(text)?;
    toast.SetGroup(&TOAST_GROUP.into()).map_err(text)?;
    ToastNotificationManager::CreateToastNotifierWithId(&TOAST_APP_ID.into())
        .map_err(text)?
        .Show(&toast)
        .map_err(text)
}

/// Trägt „AI Harness“ als App ein (Name und Symbol in der Meldung, nicht in den
/// Windows-Einstellungen)
fn register(icon: &str) -> Result<(), String> {
    let key = CURRENT_USER.create(AUMID_KEY).map_err(text)?;
    key.set_string("DisplayName", TOAST_APP_NAME)
        .map_err(text)?;
    if !icon.is_empty() {
        key.set_string("IconUri", icon).map_err(text)?;
    }
    key.set_u32("ShowInSettings", 0).map_err(text)
}

/// Fehlermeldung eines WinRT- oder Registrierungsaufrufs als Text
fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}
