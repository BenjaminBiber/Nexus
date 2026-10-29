//! Der Ereignisstrom des Servers: Verteiler, Watcher und die Brücke zum Node-Server.
//! Portiert aus `packages/server/src/events/`.

pub mod bridge;
pub mod hub;
pub mod spec_docs;
pub mod watcher;

pub use bridge::{Bridge, BRIDGED_EVENTS, SESSIONS_EVENT};
pub use hub::{EventHub, LiveEvent, Subscription};
pub use spec_docs::{SignatureOf, SpecDocWatcher, WatchedDocs};
pub use watcher::{ChangeWatcher, MarkerOf, Watched};
