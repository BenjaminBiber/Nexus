//! Favoriten des Servers: der Zustandsspeicher samt `favorites.json` und das Antwortformat.
//!
//! Portiert aus `packages/server/src/favorites/`.

pub mod favorite_dto;
pub mod favorite_service;

pub use favorite_dto::FavoriteDto;
pub use favorite_service::FavoriteService;
