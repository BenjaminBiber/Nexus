//! Systemmeldungen für Tests und den Vertragslauf: aufzeichnen statt zeigen.
//!
//! Kein Test und kein Vertragsszenario darf eine echte Meldung auslösen; `NEXUS_NOTIFIER=fake`
//! schaltet den Server deshalb auf diese Attrappe.

use std::sync::Mutex;

use async_trait::async_trait;

use super::{Notifier, ToastRequest};

/// Systemmeldungen für Tests: merkt sie sich, statt sie zu zeigen
#[derive(Default)]
pub struct FakeNotifier {
    pub shown: Mutex<Vec<ToastRequest>>,
}

impl FakeNotifier {
    /// Attrappe ohne aufgezeichnete Meldungen
    pub fn new() -> Self {
        Self::default()
    }

    /// Die aufgezeichneten Meldungen in der Reihenfolge des Aufrufs
    pub fn shown(&self) -> Vec<ToastRequest> {
        self.shown.lock().expect("Sperre der Meldungen").clone()
    }
}

#[async_trait]
impl Notifier for FakeNotifier {
    async fn show(&self, toast: &ToastRequest) -> Option<String> {
        self.shown
            .lock()
            .expect("Sperre der Meldungen")
            .push(toast.clone());
        None
    }
}
