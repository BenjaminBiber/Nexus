//! Kleiner HTTP-Client für den eigenen Server auf `127.0.0.1`. Die CLI spricht nur mit dem
//! lokalen Server und braucht dafür weder TLS noch eine asynchrone Laufzeit; in TypeScript
//! macht das `fetch` aus Node.

pub mod local_client;

pub use local_client::{LocalHttp, LocalResponse};
