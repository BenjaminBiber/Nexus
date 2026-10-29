//! Der lokale Server: ihn finden, starten und die installierte Nexus-App erkennen.

pub mod app_locator;
pub mod client;
pub mod dispatch_client;
pub mod launcher;
pub mod session_notice;

pub use app_locator::{AppLocator, DesktopApp, Launch, Platform};
pub use client::ServerClient;
pub use dispatch_client::DispatchClient;
pub use launcher::{PortState, ServerLauncher, StartCommand};
pub use session_notice::ServerSessionNotice;
