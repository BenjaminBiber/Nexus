//! Themes: gemeinsame CSS-Prüfung und die CSS-Schnipsel des Nutzers.

pub mod css_check;
pub mod snippet_store;

pub use css_check::{check_css, check_snippet_css, CssViolation};
pub use snippet_store::{Snippet, SnippetStore, MAX_SNIPPET_BYTES};
