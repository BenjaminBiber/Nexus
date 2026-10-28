//! Der Dispatcher: Vorsilben seiner Kommentare, die Git-Bausteine für den Merge, später
//! Profile und Auswahlregeln.

pub mod git;
pub mod markers;
pub mod merge_strategy;

pub use merge_strategy::MergeStrategy;
