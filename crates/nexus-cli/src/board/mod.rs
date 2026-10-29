//! Was die CLI rund um das Board braucht: die Auswahl des Boards, die Auswertung der
//! Optionen, die Textausgaben und das Öffnen im Browser.

pub mod browser_opener;
pub mod comment_author_kind;
pub mod option_parser;
pub mod selection;
pub mod text_formatter;

pub use browser_opener::BrowserOpener;
pub use comment_author_kind::CommentAuthorKind;
pub use option_parser::BoardOptionParser;
pub use selection::BoardSelection;
pub use text_formatter::{BoardTextFormatter, ShowTextOptions};
