//! Benutzerweiter Ordner des Harness: Pfade, Serverangaben und die Liste der
//! angemeldeten Projekte.

pub mod home;
pub mod project_registry;
pub mod server_defaults;
pub mod server_info;

pub use home::HarnessHome;
pub use project_registry::{AddedProject, ProjectRegistry, ProjectState, RegisteredProject};
pub use server_defaults::DEFAULT_SERVER_PORT;
pub use server_info::{ServerInfo, ServerInfoFile};
