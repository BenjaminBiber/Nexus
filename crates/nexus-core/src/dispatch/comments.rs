//! Der kleine Ausschnitt eines Kommentars, den der Dispatcher braucht: Autor und Text.
//!
//! In TypeScript steht dafür `Pick<Comment, 'authorName' | 'body'>`; in Rust reicht ein Trait,
//! damit Zähler und Rückmeldungen sowohl mit `Comment` als auch mit Testpaaren arbeiten.

use crate::board::Comment;

/// Ein Kommentar, von dem nur Autor und Text zählen
pub trait CommentLike {
    fn author_name(&self) -> &str;
    fn body(&self) -> &str;
}

impl CommentLike for Comment {
    fn author_name(&self) -> &str {
        &self.author_name
    }

    fn body(&self) -> &str {
        &self.body
    }
}

impl CommentLike for (&str, &str) {
    fn author_name(&self) -> &str {
        self.0
    }

    fn body(&self) -> &str {
        self.1
    }
}

impl CommentLike for (String, String) {
    fn author_name(&self) -> &str {
        &self.0
    }

    fn body(&self) -> &str {
        &self.1
    }
}

impl<T: CommentLike> CommentLike for &T {
    fn author_name(&self) -> &str {
        (*self).author_name()
    }

    fn body(&self) -> &str {
        (*self).body()
    }
}
