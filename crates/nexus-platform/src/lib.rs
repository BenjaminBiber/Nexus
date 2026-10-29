//! Systemfunktionen des Rechners, auf dem Nexus läuft: Ordnerdialog, Dateien im
//! Explorer/Finder oder in VS Code öffnen und Systemmeldungen (Toasts).
//!
//! Hier stehen nur die Verträge samt einer „nicht verfügbar“-Umsetzung und Attrappen für
//! Tests. Die echten Umsetzungen kommen später (Ordnerdialog, Öffnen, Windows-Toasts) und
//! ersetzen nur die Trait-Objekte im Zustand des Servers. Vorlagen in TypeScript:
//! `packages/server/src/dbboard/FolderPicker.ts`, `packages/server/src/docs/FileOpener.ts`
//! und `packages/server/src/notifications/WindowsToastSender.ts`.

pub mod windows;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use nexus_core::HarnessError;
use serde::{Deserialize, Serialize};

/// Fehler einer Systemfunktion; trägt Code und deutsche Meldung, damit der Server ihn
/// unverändert als API-Fehler ausgeben kann
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct PlatformError {
    pub code: String,
    pub message: String,
}

impl PlatformError {
    /// Fehler mit stabilem Code und Meldung
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<PlatformError> for HarnessError {
    fn from(error: PlatformError) -> Self {
        HarnessError::new(error.code, error.message)
    }
}

/// Ergebnis einer Systemfunktion
pub type Result<T> = std::result::Result<T, PlatformError>;

/// Wo eine Datei geöffnet wird: im Explorer/Finder (markiert) oder in VS Code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenTarget {
    Explorer,
    Vscode,
}

/// Eine Systemmeldung: Titel, Text, Ziel beim Klick und die Gruppe, in der sich Meldungen
/// gegenseitig ersetzen
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ToastRequest {
    pub title: String,
    pub body: String,
    /// Adresse, die der Klick öffnet (leer: nichts öffnen)
    pub url: Option<String>,
    /// Gleiche Kennung ersetzt eine frühere Meldung (Standard `attention`)
    pub tag: Option<String>,
}

/// Ordnerdialog auf diesem Rechner; `None` = abgebrochen
#[async_trait]
pub trait FolderPicker: Send + Sync {
    async fn pick(&self, start: Option<&str>) -> Result<Option<String>>;
}

/// Datei im Explorer/Finder bzw. in VS Code öffnen
#[async_trait]
pub trait FileOpener: Send + Sync {
    async fn open(&self, file: &Path, target: OpenTarget) -> Result<()>;
}

/// Systemmeldung zeigen; `None` = gezeigt, `Some(text)` = nicht gezeigt mit Grund
#[async_trait]
pub trait Notifier: Send + Sync {
    async fn show(&self, toast: &ToastRequest) -> Option<String>;
}

/// Alle Systemfunktionen zusammen, wie der Server sie hält
pub struct Platform {
    pub folders: Box<dyn FolderPicker>,
    pub files: Box<dyn FileOpener>,
    pub notifier: Box<dyn Notifier>,
}

impl Platform {
    /// Systemfunktionen, die es auf diesem Rechner (noch) nicht gibt: jeder Aufruf meldet
    /// denselben Fehler wie heute
    pub fn unavailable() -> Self {
        Self {
            folders: Box::new(UnavailableFolderPicker),
            files: Box::new(UnavailableFileOpener),
            notifier: Box::new(SilentNotifier),
        }
    }

    /// Die Systemfunktionen dieses Rechners: unter Windows der Explorer-Ordnerdialog, sonst
    /// die „nicht verfügbar“-Umsetzung (dort nutzt die App ab P1-4 den Tauri-Dialog)
    pub fn local() -> Self {
        Self {
            #[cfg(windows)]
            folders: Box::new(crate::windows::folder_picker::WindowsFolderPicker::new()),
            #[cfg(not(windows))]
            folders: Box::new(UnavailableFolderPicker),
            files: Box::new(UnavailableFileOpener),
            notifier: Box::new(SilentNotifier),
        }
    }

    /// Systemfunktionen im Vertragsmodus: kein Ordnerdialog (die Auswahl gilt als
    /// abgebrochen), kein Öffnen von Dateien, keine Systemmeldungen – wie
    /// `contract/servers/ts-server.mjs` sie stellt
    pub fn contract() -> Self {
        Self {
            folders: Box::new(FakeFolderPicker::new(None)),
            files: Box::new(FakeFileOpener::default()),
            notifier: Box::new(SilentNotifier),
        }
    }

    /// Attrappen für Tests: kein Dialog, kein Programmstart, gemerkte Meldungen
    pub fn fake() -> Self {
        Self {
            folders: Box::new(FakeFolderPicker::new(None)),
            files: Box::new(FakeFileOpener::default()),
            notifier: Box::new(FakeNotifier::default()),
        }
    }
}

/// Kein Ordnerdialog auf diesem Rechner
pub struct UnavailableFolderPicker;

#[async_trait]
impl FolderPicker for UnavailableFolderPicker {
    async fn pick(&self, _start: Option<&str>) -> Result<Option<String>> {
        Err(PlatformError::new(
            "folder-picker-unavailable",
            "Die Ordnerauswahl gibt es nur unter Windows; den Pfad bitte eintippen.",
        ))
    }
}

/// Dateien lassen sich auf diesem Rechner nicht öffnen
pub struct UnavailableFileOpener;

#[async_trait]
impl FileOpener for UnavailableFileOpener {
    async fn open(&self, _file: &Path, _target: OpenTarget) -> Result<()> {
        Err(PlatformError::new(
            "open-unavailable",
            "Dateien öffnen gibt es nur unter Windows.",
        ))
    }
}

/// Zeigt keine Systemmeldungen (z. B. außerhalb von Windows)
pub struct SilentNotifier;

#[async_trait]
impl Notifier for SilentNotifier {
    async fn show(&self, _toast: &ToastRequest) -> Option<String> {
        Some("Windows-Benachrichtigungen gibt es nur unter Windows.".into())
    }
}

/// Ordnerdialog für Tests: liefert immer denselben Ordner
pub struct FakeFolderPicker {
    answer: Option<String>,
    /// Startordner der Aufrufe, in der Reihenfolge des Aufrufs
    pub calls: Mutex<Vec<Option<String>>>,
}

impl FakeFolderPicker {
    /// Dialog, der stets `answer` liefert (`None` = abgebrochen)
    pub fn new(answer: Option<String>) -> Self {
        Self {
            answer,
            calls: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl FolderPicker for FakeFolderPicker {
    async fn pick(&self, start: Option<&str>) -> Result<Option<String>> {
        self.calls
            .lock()
            .expect("Sperre der Aufrufe")
            .push(start.map(str::to_string));
        Ok(self.answer.clone())
    }
}

/// Öffnen für Tests: merkt sich nur, was geöffnet werden sollte
#[derive(Default)]
pub struct FakeFileOpener {
    pub opened: Mutex<Vec<(PathBuf, OpenTarget)>>,
}

#[async_trait]
impl FileOpener for FakeFileOpener {
    async fn open(&self, file: &Path, target: OpenTarget) -> Result<()> {
        self.opened
            .lock()
            .expect("Sperre der geöffneten Dateien")
            .push((file.to_path_buf(), target));
        Ok(())
    }
}

/// Systemmeldungen für Tests: merkt sie sich, statt sie zu zeigen
#[derive(Default)]
pub struct FakeNotifier {
    pub shown: Mutex<Vec<ToastRequest>>,
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

/// Name des Crates, wie er in Protokollen und Diagnoseausgaben erscheint.
pub const CRATE_NAME: &str = "nexus-platform";

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn nicht_verfugbare_systemfunktionen_melden_die_heutigen_fehler() {
        let platform = Platform::unavailable();
        let dialog = platform.folders.pick(None).await.unwrap_err();
        assert_eq!(dialog.code, "folder-picker-unavailable");
        assert!(dialog.message.contains("nur unter Windows"));

        let open = platform
            .files
            .open(Path::new("a.md"), OpenTarget::Vscode)
            .await
            .unwrap_err();
        assert_eq!(open.code, "open-unavailable");

        let toast = platform.notifier.show(&ToastRequest::default()).await;
        assert_eq!(
            toast.as_deref(),
            Some("Windows-Benachrichtigungen gibt es nur unter Windows.")
        );
    }

    #[tokio::test]
    async fn attrappen_merken_sich_die_aufrufe() {
        let picker = FakeFolderPicker::new(Some("/tmp/x".into()));
        assert_eq!(
            picker.pick(Some("/start")).await.unwrap().as_deref(),
            Some("/tmp/x")
        );
        assert_eq!(
            picker.calls.lock().unwrap().as_slice(),
            [Some("/start".to_string())]
        );

        let opener = FakeFileOpener::default();
        opener
            .open(Path::new("a.md"), OpenTarget::Explorer)
            .await
            .unwrap();
        assert_eq!(opener.opened.lock().unwrap().len(), 1);

        let notifier = FakeNotifier::default();
        let toast = ToastRequest {
            title: "Frage".into(),
            body: "Bitte entscheiden".into(),
            ..ToastRequest::default()
        };
        assert_eq!(notifier.show(&toast).await, None);
        assert_eq!(notifier.shown.lock().unwrap().as_slice(), [toast]);
    }

    #[test]
    fn ein_plattformfehler_wird_unverandert_zum_harness_fehler() {
        let error = PlatformError::new("open-unavailable", "geht nicht");
        let harness: HarnessError = error.into();
        assert_eq!(harness.code, "open-unavailable");
        assert_eq!(harness.message, "geht nicht");
    }
}
