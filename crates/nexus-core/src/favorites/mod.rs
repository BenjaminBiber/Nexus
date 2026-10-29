//! Gemerkte Sessions: die Fachobjekte, der Zustandsspeicher, die Datei `favorites.json` und
//! der Befehl zum Fortsetzen. Portiert aus `packages/core/src/favorites`.

pub mod favorite;
pub mod persistence;
pub mod resume_command;
pub mod store;

pub use favorite::{Favorite, SessionSnapshot};
pub use persistence::{FavoritePersistence, FavoritesRead};
pub use resume_command::ResumeCommand;
pub use store::FavoriteStore;
