//! Speichert die Favoriten aller Projekte in `~/.harness/favorites.json` (atomar).
//! Portiert aus `packages/core/src/favorites/FavoritePersistence.ts`.

use std::fs;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::locking::AtomicWriter;
use crate::registry::HarnessHome;

use super::favorite::Favorite;

/// Form der Datei: ein Objekt mit der Liste der Favoriten
#[derive(Debug, Default, Serialize, Deserialize)]
struct FavoritesFile {
    #[serde(default)]
    favorites: Vec<Favorite>,
}

/// Gelesene Favoriten; `recovered` meldet, dass eine beschädigte Datei gesichert wurde
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FavoritesRead {
    pub favorites: Vec<Favorite>,
    pub recovered: bool,
}

/// Liest und schreibt `favorites.json`
#[derive(Debug)]
pub struct FavoritePersistence {
    home: HarnessHome,
    writer: AtomicWriter,
}

impl FavoritePersistence {
    /// Speicher im angegebenen Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            home,
            writer: AtomicWriter::new(),
        }
    }

    /// Liest die Favoriten; eine beschädigte Datei wird nach `.bak` gesichert, `recovered`
    /// meldet das
    pub fn read(&self) -> Result<FavoritesRead> {
        let file = self.home.favorites_file();
        if !file.exists() {
            return Ok(FavoritesRead {
                favorites: Vec::new(),
                recovered: false,
            });
        }
        let data = fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str::<FavoritesFile>(&text).ok());
        match data {
            Some(data) => Ok(FavoritesRead {
                favorites: data.favorites,
                recovered: false,
            }),
            None => {
                let backup = format!("{}.bak", file.display());
                fs::copy(&file, &backup).map_err(Error::unexpected)?;
                fs::remove_file(&file).map_err(Error::unexpected)?;
                Ok(FavoritesRead {
                    favorites: Vec::new(),
                    recovered: true,
                })
            }
        }
    }

    /// Schreibt die Favoriten; der Ordner wird bei Bedarf angelegt
    pub fn write(&self, favorites: &[Favorite]) -> Result<()> {
        fs::create_dir_all(self.home.dir()).map_err(Error::unexpected)?;
        let data = FavoritesFile {
            favorites: favorites.to_vec(),
        };
        self.writer.write(
            &self.home.favorites_file(),
            &serde_json::to_string_pretty(&data).map_err(Error::unexpected)?,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::sessions::SessionState;

    use super::*;

    /// Ein Favorit als Vorlage
    fn fav() -> Favorite {
        Favorite {
            project_id: "p".into(),
            session_id: "s1".into(),
            note: "N".into(),
            tickets: vec!["T-0001".into()],
            cwd: "D:/wt".into(),
            title: None,
            marked_at: "2026-10-10T10:00:00.000Z".into(),
            last_state: SessionState::Idle,
            last_event_at: "2026-10-10T10:00:00.000Z".into(),
            resumed_as: Vec::new(),
        }
    }

    #[test]
    fn hat_pfade_fur_favoriten_und_sicherungen_im_harness_ordner() {
        let home = HarnessHome::at("/tmp/neu");
        assert!(home
            .favorites_file()
            .to_string_lossy()
            .ends_with("neu/favorites.json"));
        assert!(home
            .backups_dir()
            .to_string_lossy()
            .ends_with("neu/backups"));
    }

    #[test]
    fn liefert_ohne_datei_eine_leere_liste() {
        let dir = tempfile::tempdir().unwrap();
        let p = FavoritePersistence::new(HarnessHome::at(dir.path().join("neu")));
        assert_eq!(
            p.read().unwrap(),
            FavoritesRead {
                favorites: Vec::new(),
                recovered: false
            }
        );
    }

    #[test]
    fn schreibt_und_liest_favoriten_auch_bei_noch_nicht_vorhandenem_ordner() {
        let dir = tempfile::tempdir().unwrap();
        let p = FavoritePersistence::new(HarnessHome::at(dir.path().join("neu")));
        p.write(&[fav()]).unwrap();
        assert_eq!(
            p.read().unwrap(),
            FavoritesRead {
                favorites: vec![fav()],
                recovered: false
            }
        );
    }

    #[test]
    fn sichert_eine_beschadigte_datei_nach_bak_und_startet_leer() {
        let dir = tempfile::tempdir().unwrap();
        let home = HarnessHome::at(dir.path().join("neu"));
        let p = FavoritePersistence::new(home.clone());
        fs::create_dir_all(home.dir()).unwrap();
        fs::write(home.favorites_file(), "{kaputt").unwrap();
        assert_eq!(
            p.read().unwrap(),
            FavoritesRead {
                favorites: Vec::new(),
                recovered: true
            }
        );
        let backup = format!("{}.bak", home.favorites_file().display());
        assert_eq!(fs::read_to_string(backup).unwrap(), "{kaputt");
        assert!(!home.favorites_file().exists());
    }
}
