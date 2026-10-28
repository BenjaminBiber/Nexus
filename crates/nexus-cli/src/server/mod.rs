//! Der lokale Server: ihn finden, starten und die installierte Nexus-App erkennen.

pub mod app_locator;
pub mod launcher;

pub use app_locator::{AppLocator, DesktopApp, Launch, Platform};
pub use launcher::{PortState, ServerLauncher, StartCommand};
