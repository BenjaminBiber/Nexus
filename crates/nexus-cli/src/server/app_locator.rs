//! Findet die installierte Nexus-App. Ist sie da, startet `nexus board` sie statt des
//! Node-Servers; den Port wählt dann die App selbst und schreibt ihn in `server.json`.
//!
//! Der Dateiname kommt aus `apps/desktop/src-tauri/tauri.conf.json` (`productName: "Nexus"`):
//! macOS `Nexus.app`, Windows `Nexus.exe`, Linux `nexus-app` bzw. `Nexus*.AppImage`.
//! Groß geschrieben kollidiert er nicht mit dem CLI-Binary `nexus`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Umgebungsvariable, die jeden Suchweg übergeht (Tests und eigene Installationen)
pub const APP_PATH_ENV: &str = "NEXUS_APP_PATH";

/// Name des Programms aus `tauri.conf.json` (`productName`)
pub const APP_NAME: &str = "Nexus";

/// Name des Programms unter Linux im PATH (klein geschrieben, mit Zusatz gegen die Kollision
/// mit dem CLI-Binary `nexus`)
pub const LINUX_APP_NAME: &str = "nexus-app";

/// Betriebssystem, für das gesucht wird; als Parameter, damit sich alle drei Wege testen lassen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Windows,
    Linux,
}

/// Das Betriebssystem, auf dem dieser Prozess läuft
pub const CURRENT: Platform = if cfg!(target_os = "macos") {
    Platform::MacOs
} else if cfg!(windows) {
    Platform::Windows
} else {
    Platform::Linux
};

/// Wie die gefundene App gestartet wird
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launch {
    /// macOS-Bündel: `open -a <pfad>`
    Bundle,
    /// Ausführbare Datei: direkt starten
    Program,
}

/// Eine gefundene App: Pfad und Startweg
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApp {
    pub path: PathBuf,
    pub launch: Launch,
}

/// Sucht die installierte Nexus-App an den Stellen, an die die Installationspakete sie legen.
#[derive(Debug, Clone)]
pub struct AppLocator {
    env: HashMap<String, String>,
    home: PathBuf,
}

impl AppLocator {
    /// Sucher mit der Umgebung und dem Benutzerordner dieses Prozesses
    pub fn from_process_env() -> Self {
        Self::new(std::env::vars().collect(), user_home())
    }

    /// Sucher mit frei wählbarer Umgebung und frei wählbarem Benutzerordner (Tests)
    pub fn new(env: HashMap<String, String>, home: impl Into<PathBuf>) -> Self {
        Self {
            env,
            home: home.into(),
        }
    }

    /// Die App, wenn sie installiert ist (erster Pfad, der es wirklich gibt)
    pub fn locate(&self) -> Option<DesktopApp> {
        self.locate_on(CURRENT)
    }

    /// Wie `locate`, aber für ein bestimmtes Betriebssystem (Tests)
    pub fn locate_on(&self, platform: Platform) -> Option<DesktopApp> {
        self.candidates(platform)
            .into_iter()
            .find(|app| app.path.exists())
    }

    /// Alle Stellen, an denen die App liegen kann, in der Reihenfolge der Suche
    pub fn candidates(&self, platform: Platform) -> Vec<DesktopApp> {
        if let Some(path) = self.var(APP_PATH_ENV) {
            let path = PathBuf::from(path);
            return vec![DesktopApp {
                launch: Self::launch_of(&path, platform),
                path,
            }];
        }
        match platform {
            Platform::MacOs => vec![
                Self::bundle(Path::new("/Applications").join(format!("{APP_NAME}.app"))),
                Self::bundle(
                    self.home
                        .join("Applications")
                        .join(format!("{APP_NAME}.app")),
                ),
            ],
            Platform::Windows => self
                .var("LOCALAPPDATA")
                .map(|local| {
                    vec![Self::program(
                        Path::new(&local)
                            .join(APP_NAME)
                            .join(format!("{APP_NAME}.exe")),
                    )]
                })
                .unwrap_or_default()
                .into_iter()
                .chain(nsis_candidates())
                .collect(),
            Platform::Linux => self
                .path_entries()
                .into_iter()
                .map(|dir| Self::program(dir.join(LINUX_APP_NAME)))
                .chain(self.app_images())
                .collect(),
        }
    }

    /// Ordner des Suchpfads (`PATH`)
    fn path_entries(&self) -> Vec<PathBuf> {
        self.var("PATH")
            .map(|path| std::env::split_paths(&path).collect())
            .unwrap_or_default()
    }

    /// AppImages unter `~/Applications`, nach Namen sortiert, damit die Wahl wiederholbar ist
    fn app_images(&self) -> Vec<DesktopApp> {
        let dir = self.home.join("Applications");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default();
                name.starts_with(APP_NAME) && name.to_ascii_lowercase().ends_with(".appimage")
            })
            .collect();
        files.sort();
        files.into_iter().map(Self::program).collect()
    }

    /// Startweg eines ausdrücklich genannten Pfads: unter macOS ist ein `.app` ein Bündel
    fn launch_of(path: &Path, platform: Platform) -> Launch {
        let bundle = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("app"));
        if platform == Platform::MacOs && bundle {
            Launch::Bundle
        } else {
            Launch::Program
        }
    }

    /// Ein macOS-Bündel
    fn bundle(path: PathBuf) -> DesktopApp {
        DesktopApp {
            path,
            launch: Launch::Bundle,
        }
    }

    /// Eine ausführbare Datei
    fn program(path: PathBuf) -> DesktopApp {
        DesktopApp {
            path,
            launch: Launch::Program,
        }
    }

    /// Eine Umgebungsvariable, leere zählen wie keine
    fn var(&self, name: &str) -> Option<String> {
        self.env
            .get(name)
            .filter(|value| !value.is_empty())
            .cloned()
    }
}

/// Installationsordner aus dem Uninstall-Eintrag des NSIS-Installers (nur Windows).
/// Der Eintrag steht bei `installMode: "currentUser"` unter `HKCU`, bei einer Installation für
/// alle Benutzer unter `HKLM`; gelesen wird er mit `reg query`, damit keine zusätzliche
/// Abhängigkeit nötig ist.
#[cfg(windows)]
fn nsis_candidates() -> Vec<DesktopApp> {
    const KEYS: [&str; 2] = [
        "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Nexus",
        "HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Nexus",
    ];
    let mut out = Vec::new();
    for key in KEYS {
        let output = std::process::Command::new("reg")
            .args([key, "/v", "InstallLocation"])
            .output();
        let Ok(output) = output else { continue };
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if let Some((_, value)) = line.trim().split_once("REG_SZ") {
                let dir = value.trim();
                if !dir.is_empty() {
                    out.push(AppLocator::program(
                        Path::new(dir).join(format!("{APP_NAME}.exe")),
                    ));
                }
            }
        }
    }
    out
}

/// Außerhalb von Windows gibt es keinen Uninstall-Eintrag
#[cfg(not(windows))]
fn nsis_candidates() -> Vec<DesktopApp> {
    Vec::new()
}

/// Benutzerordner wie `os.homedir()` in Node
fn user_home() -> PathBuf {
    #[allow(deprecated)]
    std::env::home_dir().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locator(paare: &[(&str, &str)], home: &Path) -> AppLocator {
        AppLocator::new(
            paare
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            home,
        )
    }

    #[test]
    fn nexus_app_path_geht_vor_allem_anderen() {
        let locator = locator(
            &[(APP_PATH_ENV, "/woanders/Nexus.app")],
            Path::new("/u/ben"),
        );
        let candidates = locator.candidates(Platform::MacOs);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].path, Path::new("/woanders/Nexus.app"));
        assert_eq!(candidates[0].launch, Launch::Bundle);
    }

    #[test]
    fn ein_genannter_pfad_ohne_app_endung_ist_ein_programm() {
        let locator = locator(&[(APP_PATH_ENV, "/woanders/Nexus")], Path::new("/u/ben"));
        assert_eq!(
            locator.candidates(Platform::MacOs)[0].launch,
            Launch::Program
        );
    }

    #[test]
    fn unter_macos_sucht_sie_in_beiden_programmordnern() {
        let locator = locator(&[], Path::new("/u/ben"));
        let pfade: Vec<PathBuf> = locator
            .candidates(Platform::MacOs)
            .into_iter()
            .map(|app| app.path)
            .collect();
        assert_eq!(
            pfade,
            vec![
                PathBuf::from("/Applications/Nexus.app"),
                PathBuf::from("/u/ben/Applications/Nexus.app"),
            ]
        );
        assert!(locator
            .candidates(Platform::MacOs)
            .iter()
            .all(|app| app.launch == Launch::Bundle));
    }

    #[test]
    fn unter_windows_liegt_sie_im_lokalen_anwendungsordner() {
        let locator = locator(
            &[("LOCALAPPDATA", "C:\\Users\\ben\\AppData\\Local")],
            Path::new("C:\\Users\\ben"),
        );
        let candidates = locator.candidates(Platform::Windows);
        assert_eq!(
            candidates[0].path,
            Path::new("C:\\Users\\ben\\AppData\\Local")
                .join("Nexus")
                .join("Nexus.exe")
        );
        assert_eq!(candidates[0].launch, Launch::Program);
    }

    #[test]
    fn ohne_localappdata_sucht_sie_nicht_im_anwendungsordner() {
        // Ohne die Variable bleibt nur der Uninstall-Eintrag des Installers (nur Windows)
        assert!(locator(&[], Path::new("C:\\Users\\ben"))
            .candidates(Platform::Windows)
            .iter()
            .all(|app| !app.path.to_string_lossy().contains("AppData")));
    }

    #[test]
    fn unter_linux_sucht_sie_im_pfad_und_dann_die_appimage() {
        let dir = tempfile::Builder::new()
            .prefix("nexus-app-")
            .tempdir()
            .unwrap();
        let applications = dir.path().join("Applications");
        std::fs::create_dir_all(&applications).unwrap();
        std::fs::write(applications.join("Nexus_0.1.0.AppImage"), b"x").unwrap();
        std::fs::write(applications.join("Fremd.AppImage"), b"x").unwrap();

        let pfad = std::env::join_paths(["/bin", "/usr/bin"]).unwrap();
        let locator = locator(&[("PATH", pfad.to_str().unwrap())], dir.path());
        let pfade: Vec<PathBuf> = locator
            .candidates(Platform::Linux)
            .into_iter()
            .map(|app| app.path)
            .collect();
        assert_eq!(
            pfade,
            vec![
                PathBuf::from("/bin/nexus-app"),
                PathBuf::from("/usr/bin/nexus-app"),
                applications.join("Nexus_0.1.0.AppImage"),
            ]
        );
    }

    #[test]
    fn gefunden_wird_nur_ein_pfad_den_es_gibt() {
        let dir = tempfile::Builder::new()
            .prefix("nexus-app-")
            .tempdir()
            .unwrap();
        let app = dir.path().join("Nexus.app");

        let fehlt = locator(&[(APP_PATH_ENV, app.to_str().unwrap())], dir.path());
        assert_eq!(fehlt.locate_on(Platform::MacOs), None);

        std::fs::create_dir(&app).unwrap();
        assert_eq!(
            fehlt.locate_on(Platform::MacOs),
            Some(DesktopApp {
                path: app,
                launch: Launch::Bundle
            })
        );
    }

    #[test]
    fn ein_leeres_nexus_app_path_zahlt_wie_keines() {
        let locator = locator(&[(APP_PATH_ENV, "")], Path::new("/u/ben"));
        assert_eq!(locator.candidates(Platform::MacOs).len(), 2);
    }
}
