//! Der Ereignisstrom des Servers: Verteiler und Watcher.
//! Portiert aus `packages/server/src/events/`.

pub mod hub;
pub mod spec_docs;
pub mod watcher;

pub use hub::{EventHub, LiveEvent, Subscription};
pub use spec_docs::{SignatureOf, SpecDocWatcher, WatchedDocs};
pub use watcher::{ChangeWatcher, MarkerOf, Watched};
