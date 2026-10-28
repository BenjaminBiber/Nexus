//! Dateisperre und atomares Schreiben: die Grundlage dafür, dass mehrere Prozesse –
//! auch ein TypeScript- und ein Rust-Prozess gleichzeitig – dieselben Dateien bearbeiten.

pub mod atomic_writer;
pub mod file_lock;

pub use atomic_writer::AtomicWriter;
pub use file_lock::{FileLock, FileLockOptions};
