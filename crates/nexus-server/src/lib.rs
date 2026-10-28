//! Server von Nexus: HTTP-API, Dispatch-Worker und Benachrichtigungen. Bisher enthält das
//! Crate nur die Prozess-Bausteine des Dispatchers; die Routen und das axum-Gerüst folgen.

pub mod dispatch;
