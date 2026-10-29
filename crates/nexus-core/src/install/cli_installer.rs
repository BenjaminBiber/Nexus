//! Richtet `nexus` für das normale Terminal ein: kopiert das Binary nach `~/.nexus/bin`,
//! die Regeln nach `~/.nexus/rules`, die Anpassungen nach `~/.nexus/customizations`, legt
//! die Startdateien an und trägt den Startordner in den Benutzer-PATH ein.
//! Portiert aus `packages/core/src/install/CliInstaller.ts`; statt `lib/cli.mjs` wird das
//! Rust-Binary kopiert, `lib/` und `web/` nur noch, solange es den Node-Server gibt.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, HarnessError, Result};
use crate::install::launcher_writer::{LauncherWriter, BINARY_NAME};
use crate::install::user_path::UserPath;
use crate::registry::HarnessHome;

/// Ordner des Plugins, die neben dem Binary mitkopiert werden (für den Node-Server bis Phase 5)
const APP_DIRS: &[&str] = &["lib", "web"];

/// Datei, an der eine Plugin-Wurzel zu erkennen ist
const PLUGIN_MANIFEST: [&str; 2] = [".claude-plugin", "plugin.json"];

/// Was `install` tun soll
#[derive(Debug, Clone, Copy)]
pub struct InstallOptions {
    /// PATH des Benutzers ergänzen (Standard: ja; `--no-path` schaltet es ab)
    pub register_path: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            register_path: true,
        }
    }
}

/// Ergebnis der Installation; die Schlüssel im JSON sind dieselben wie in TypeScript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    /// Ordner mit Binary und Startdateien (liegt im PATH)
    pub bin_dir: String,
    /// Ordner mit dem kopierten Server und der Oberfläche
    pub app_dir: String,
    /// Ordner mit den zentralen Regeln, die Projekte importieren
    pub rules_dir: String,
    /// Angelegte Startdateien
    pub launchers: Vec<String>,
    /// true, wenn der PATH-Eintrag neu gesetzt wurde
    pub path_added: bool,
    /// true, wenn der PATH nicht angefasst wurde (Option `--no-path`)
    pub path_skipped: bool,
}

/// Installiert das Binary und die Startdateien im Benutzerordner.
pub struct CliInstaller<'a> {
    home: HarnessHome,
    user_path: &'a dyn UserPath,
    launchers: LauncherWriter,
    legacy_home: Option<PathBuf>,
}

impl<'a> CliInstaller<'a> {
    /// Installer für einen Benutzerordner und einen PATH-Eintrag
    pub fn new(home: HarnessHome, user_path: &'a dyn UserPath) -> Self {
        Self {
            home,
            user_path,
            launchers: LauncherWriter::new(),
            legacy_home: None,
        }
    }

    /// Alter Benutzerordner (`~/.harness`): dessen Startdateien werden durch Weiterleitungen
    /// auf das neue Binary ersetzt, damit ein alter PATH-Eintrag nicht die alte Fassung startet
    pub fn with_legacy_home(mut self, dir: impl Into<PathBuf>) -> Self {
        self.legacy_home = Some(dir.into());
        self
    }

    /// Plugin-Wurzel zum laufenden Binary (Aufbau `<wurzel>/bin/nexus[.exe]`, daneben
    /// `.claude-plugin/plugin.json`); sonst `None`
    pub fn bundle_root(exe: &Path) -> Option<PathBuf> {
        if exe.file_name()? != BINARY_NAME || exe.parent()?.file_name()? != "bin" {
            return None;
        }
        let root = exe.parent()?.parent()?;
        let manifest = root.join(PLUGIN_MANIFEST[0]).join(PLUGIN_MANIFEST[1]);
        manifest.exists().then(|| root.to_path_buf())
    }

    /// Installiert (oder aktualisiert) das CLI aus der Plugin-Wurzel; wiederholbar
    pub fn install(&self, root: &Path, options: InstallOptions) -> Result<InstallResult> {
        let app_dir = self.home.dir().join("app");
        let bin_dir = self.home.dir().join("bin");
        let source = root.join("bin").join(BINARY_NAME);
        if !source.exists() {
            return Err(HarnessError::new(
                "install-source-missing",
                format!("Im Ordner {} fehlt bin/{BINARY_NAME}.", root.display()),
            )
            .into());
        }
        self.copy_app(root, &app_dir)?;
        self.copy_rules(root)?;
        self.copy_customizations(root)?;
        let launchers = self.launchers.write(&bin_dir)?;
        let binary = bin_dir.join(BINARY_NAME);
        copy_binary(&source, &binary)?;
        self.redirect_legacy_launchers(&bin_dir, &binary)?;
        let path_added = if options.register_path {
            self.user_path.add(&bin_dir)?
        } else {
            false
        };
        Ok(InstallResult {
            bin_dir: text(&bin_dir),
            app_dir: text(&app_dir),
            rules_dir: text(&self.home.rules_dir()),
            launchers: launchers.iter().map(|file| text(file)).collect(),
            path_added,
            path_skipped: !options.register_path,
        })
    }

    /// Ersetzt die Startdateien im alten Ordner `~/.harness/bin` durch Weiterleitungen auf das
    /// neue Binary. Gibt es den alten Startordner nicht oder ist es derselbe wie der neue,
    /// bleibt alles, wie es ist; Daten werden nie angefasst.
    fn redirect_legacy_launchers(&self, bin_dir: &Path, binary: &Path) -> Result<()> {
        let Some(legacy) = &self.legacy_home else {
            return Ok(());
        };
        let legacy_bin = legacy.join("bin");
        if !legacy_bin.is_dir() || is_same(&legacy_bin, bin_dir) {
            return Ok(());
        }
        self.launchers.write_forwarders(&legacy_bin, binary)?;
        Ok(())
    }

    /// Kopiert `lib/` und `web/` frisch nach `<home>/app`, falls das Plugin sie mitbringt
    /// (Node-Server und Oberfläche bis Phase 5); alte Dateien verschwinden dabei
    fn copy_app(&self, root: &Path, app_dir: &Path) -> Result<()> {
        if is_same(root, app_dir) {
            return Ok(());
        }
        let sources: Vec<PathBuf> = APP_DIRS
            .iter()
            .map(|name| root.join(name))
            .filter(|dir| dir.is_dir())
            .collect();
        if sources.is_empty() {
            return Ok(());
        }
        remove_dir(app_dir)?;
        fs::create_dir_all(app_dir).map_err(Error::unexpected)?;
        for source in sources {
            let name = source.file_name().unwrap_or_default();
            copy_dir(&source, &app_dir.join(name))?;
        }
        Ok(())
    }

    /// Legt die Regeln des Plugins frisch unter `~/.nexus/rules` ab, dem festen Importziel der
    /// Projekte. Der Ordner des Plugins in Claude Code taugt dafür nicht, er wechselt mit jeder
    /// Version.
    fn copy_rules(&self, root: &Path) -> Result<()> {
        self.copy_into(&root.join("rules"), &self.home.rules_dir())
    }

    /// Legt die angewendeten Anpassungen frisch unter `~/.nexus/customizations` ab; Hook und
    /// Server lesen dort
    fn copy_customizations(&self, root: &Path) -> Result<()> {
        self.copy_into(
            &root.join("customizations"),
            &self.home.customizations_dir(),
        )
    }

    /// Kopiert einen Ordner des Plugins frisch in den Benutzerordner, falls es ihn gibt
    fn copy_into(&self, source: &Path, target: &Path) -> Result<()> {
        if !source.is_dir() || is_same(source, target) {
            return Ok(());
        }
        remove_dir(target)?;
        copy_dir(source, target)
    }
}

/// Kopiert das Binary; unter Windows über einen temporären Namen, falls die alte Datei gerade
/// läuft und deshalb gesperrt ist
fn copy_binary(source: &Path, target: &Path) -> Result<()> {
    if is_same(source, target) {
        return Ok(());
    }
    match fs::copy(source, target) {
        Ok(_) => Ok(()),
        Err(error) if cfg!(windows) => {
            let parked = target.with_extension("alt");
            let _ = fs::remove_file(&parked);
            fs::rename(target, &parked).map_err(|_| Error::unexpected(error))?;
            fs::copy(source, target).map_err(Error::unexpected)?;
            // Die geparkte Datei lässt sich erst nach dem Ende des alten Prozesses löschen
            let _ = fs::remove_file(&parked);
            Ok(())
        }
        Err(error) => Err(Error::unexpected(error)),
    }
}

/// Löscht einen Ordner samt Inhalt, falls es ihn gibt
fn remove_dir(dir: &Path) -> Result<()> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::unexpected(error)),
    }
}

/// Kopiert einen Ordner samt Unterordnern (wie `cp -r`)
fn copy_dir(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target).map_err(Error::unexpected)?;
    for entry in fs::read_dir(source).map_err(Error::unexpected)? {
        let entry = entry.map_err(Error::unexpected)?;
        let from = entry.path();
        let to = target.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(Error::unexpected)?;
        }
    }
    Ok(())
}

/// Quelle und Ziel gleich? (Aufruf aus der bereits installierten Kopie; Windows ohne
/// Groß-/Kleinschreibung)
fn is_same(a: &Path, b: &Path) -> bool {
    let norm = |path: &Path| {
        let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let text = absolute.to_string_lossy().into_owned();
        if cfg!(windows) {
            text.to_lowercase()
        } else {
            text
        }
    };
    norm(a) == norm(b)
}

/// Pfad als Text für das Ergebnis-JSON
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// Merkt sich die Aufrufe statt den PATH wirklich zu ändern
    #[derive(Debug, Default)]
    struct FakeUserPath {
        added: RefCell<Vec<PathBuf>>,
        result: bool,
    }

    impl FakeUserPath {
        fn new(result: bool) -> Self {
            Self {
                added: RefCell::new(Vec::new()),
                result,
            }
        }
    }

    impl UserPath for FakeUserPath {
        fn add(&self, dir: &Path) -> Result<bool> {
            self.added.borrow_mut().push(dir.to_path_buf());
            Ok(self.result)
        }
    }

    /// Minimales gebautes Plugin: `bin/nexus`, `.claude-plugin/plugin.json`, `lib/`, `web/`,
    /// `rules/docs.md`
    struct Plugin {
        base: tempfile::TempDir,
    }

    impl Plugin {
        fn new() -> Self {
            let base = tempfile::Builder::new()
                .prefix("harness-install-")
                .tempdir()
                .unwrap();
            let plugin = base.path().join("plugin");
            for dir in ["bin", "lib", "web", "rules", ".claude-plugin"] {
                fs::create_dir_all(plugin.join(dir)).unwrap();
            }
            fs::write(plugin.join("bin").join(BINARY_NAME), b"binary").unwrap();
            fs::write(
                plugin.join(".claude-plugin").join("plugin.json"),
                "{\"name\":\"ai-harness\"}",
            )
            .unwrap();
            fs::write(plugin.join("lib").join("server.mjs"), "console.log(1);").unwrap();
            fs::write(plugin.join("web").join("index.html"), "<html></html>").unwrap();
            fs::write(plugin.join("rules").join("docs.md"), "# Regeln").unwrap();
            Self { base }
        }

        fn root(&self) -> PathBuf {
            self.base.path().join("plugin")
        }

        fn home(&self) -> HarnessHome {
            HarnessHome::at(self.base.path().join("home"))
        }
    }

    #[test]
    fn erkennt_die_plugin_wurzel_nur_bei_vollstandigem_aufbau() {
        let plugin = Plugin::new();
        let exe = plugin.root().join("bin").join(BINARY_NAME);

        assert_eq!(CliInstaller::bundle_root(&exe), Some(plugin.root()));
        assert_eq!(
            CliInstaller::bundle_root(&plugin.root().join("bin").join("anderes")),
            None
        );
        fs::remove_dir_all(plugin.root().join(".claude-plugin")).unwrap();
        assert_eq!(CliInstaller::bundle_root(&exe), None);
    }

    #[test]
    fn kopiert_binary_server_und_oberflache_und_legt_beide_startdateien_an() {
        let plugin = Plugin::new();
        let path = FakeUserPath::new(true);
        let home = plugin.home();

        let result = CliInstaller::new(home, &path)
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();

        assert!(Path::new(&result.bin_dir).join(BINARY_NAME).exists());
        assert!(Path::new(&result.app_dir)
            .join("lib")
            .join("server.mjs")
            .exists());
        assert!(Path::new(&result.app_dir)
            .join("web")
            .join("index.html")
            .exists());
        let cmd = fs::read_to_string(Path::new(&result.bin_dir).join("harness.cmd")).unwrap();
        assert!(cmd.contains("nexus.exe"), "{cmd}");
        let sh = fs::read_to_string(Path::new(&result.bin_dir).join("harness")).unwrap();
        assert!(sh.contains("/nexus"), "{sh}");
    }

    #[test]
    fn ersetzt_die_startdateien_im_alten_ordner_durch_weiterleitungen() {
        let plugin = Plugin::new();
        let path = FakeUserPath::new(true);
        let alt = plugin.base.path().join("alt");
        fs::create_dir_all(alt.join("bin")).unwrap();
        fs::write(alt.join("bin").join("harness"), "#!/bin/sh\nnode alt.mjs\n").unwrap();
        fs::write(alt.join("board.db"), "daten").unwrap();

        let result = CliInstaller::new(plugin.home(), &path)
            .with_legacy_home(&alt)
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();

        let weiterleitung = fs::read_to_string(alt.join("bin").join("harness")).unwrap();
        let binary = Path::new(&result.bin_dir).join(BINARY_NAME);
        assert!(
            weiterleitung.contains(&binary.display().to_string()),
            "{weiterleitung}"
        );
        assert!(alt.join("bin").join("harness.cmd").exists());
        // Die Daten des alten Ordners bleiben unberührt
        assert_eq!(fs::read_to_string(alt.join("board.db")).unwrap(), "daten");
    }

    #[test]
    fn ohne_alten_startordner_gibt_es_nichts_umzuleiten() {
        let plugin = Plugin::new();
        let path = FakeUserPath::new(true);
        let alt = plugin.base.path().join("alt");

        CliInstaller::new(plugin.home(), &path)
            .with_legacy_home(&alt)
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();

        assert!(!alt.exists());
    }

    #[test]
    fn ist_der_alte_ordner_der_neue_bleiben_die_startdateien_wie_sie_sind() {
        let plugin = Plugin::new();
        let path = FakeUserPath::new(true);
        let home = plugin.home();

        let result = CliInstaller::new(home.clone(), &path)
            .with_legacy_home(home.dir())
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();

        let sh = fs::read_to_string(Path::new(&result.bin_dir).join("harness")).unwrap();
        assert!(sh.contains("$(dirname \"$0\")/nexus"), "{sh}");
    }

    #[test]
    fn tragt_den_startordner_in_den_path_ein_und_meldet_ob_er_neu_war() {
        let plugin = Plugin::new();
        let path = FakeUserPath::new(true);
        let result = CliInstaller::new(plugin.home(), &path)
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();
        assert_eq!(
            path.added.borrow().as_slice(),
            [PathBuf::from(&result.bin_dir)]
        );
        assert!(result.path_added);

        let again = FakeUserPath::new(false);
        let second = CliInstaller::new(plugin.home(), &again)
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();
        assert!(!second.path_added);
    }

    #[test]
    fn lasst_den_path_mit_no_path_unberuhrt() {
        let plugin = Plugin::new();
        let path = FakeUserPath::new(true);

        let result = CliInstaller::new(plugin.home(), &path)
            .install(
                &plugin.root(),
                InstallOptions {
                    register_path: false,
                },
            )
            .unwrap();

        assert!(path.added.borrow().is_empty());
        assert!(result.path_skipped);
        assert!(!result.path_added);
    }

    #[test]
    fn ist_wiederholbar_und_entfernt_veraltete_dateien_der_kopie() {
        let plugin = Plugin::new();
        let path = FakeUserPath::new(true);
        let installer = CliInstaller::new(plugin.home(), &path);
        let first = installer
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();
        fs::write(Path::new(&first.app_dir).join("lib").join("alt.mjs"), "alt").unwrap();
        fs::write(plugin.root().join("bin").join(BINARY_NAME), b"neu").unwrap();

        installer
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();

        assert!(!Path::new(&first.app_dir)
            .join("lib")
            .join("alt.mjs")
            .exists());
        assert_eq!(
            fs::read(Path::new(&first.bin_dir).join(BINARY_NAME)).unwrap(),
            b"neu"
        );
    }

    #[test]
    fn kopiert_nicht_auf_sich_selbst_wenn_der_aufruf_aus_der_installierten_kopie_kommt() {
        let plugin = Plugin::new();
        let path = FakeUserPath::new(true);
        let installer = CliInstaller::new(plugin.home(), &path);
        let first = installer
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();

        // Das installierte Binary liegt in `<home>/bin`; eine Wurzel daraus hat `bin/` und
        // (nach dem ersten Lauf) die Regeln daneben
        let second = installer
            .install(
                Path::new(&first.bin_dir).parent().unwrap(),
                InstallOptions::default(),
            )
            .unwrap();

        assert!(Path::new(&second.bin_dir).join(BINARY_NAME).exists());
        assert_eq!(
            fs::read_to_string(Path::new(&second.rules_dir).join("docs.md")).unwrap(),
            "# Regeln"
        );
    }

    #[test]
    fn legt_die_zentralen_regeln_unter_rules_ab() {
        let plugin = Plugin::new();
        let path = FakeUserPath::new(true);
        let home = plugin.home();
        let result = CliInstaller::new(home.clone(), &path)
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();

        assert_eq!(Path::new(&result.rules_dir), home.rules_dir());
        assert_eq!(
            fs::read_to_string(home.rules_dir().join("docs.md")).unwrap(),
            "# Regeln"
        );
    }

    #[test]
    fn ersetzt_die_regeln_beim_aktualisieren_und_entfernt_veraltete_dateien() {
        let plugin = Plugin::new();
        let path = FakeUserPath::new(true);
        let home = plugin.home();
        let installer = CliInstaller::new(home.clone(), &path);
        installer
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();
        fs::write(home.rules_dir().join("alt.md"), "alt").unwrap();
        fs::write(plugin.root().join("rules").join("docs.md"), "# Neu").unwrap();

        installer
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();

        assert!(!home.rules_dir().join("alt.md").exists());
        assert_eq!(
            fs::read_to_string(home.rules_dir().join("docs.md")).unwrap(),
            "# Neu"
        );
    }

    #[test]
    fn kommt_ohne_rules_im_plugin_aus() {
        let plugin = Plugin::new();
        fs::remove_dir_all(plugin.root().join("rules")).unwrap();
        let path = FakeUserPath::new(true);
        let home = plugin.home();

        CliInstaller::new(home.clone(), &path)
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();

        assert!(!home.rules_dir().exists());
    }

    #[test]
    fn legt_die_angewendeten_anpassungen_unter_customizations_ab() {
        let plugin = Plugin::new();
        fs::create_dir_all(plugin.root().join("customizations")).unwrap();
        fs::write(
            plugin.root().join("customizations").join("overview.json"),
            "{\"version\":1}",
        )
        .unwrap();
        let home = plugin.home();
        fs::create_dir_all(home.customizations_dir()).unwrap();
        fs::write(home.customizations_dir().join("workflow.json"), "[]").unwrap();
        let path = FakeUserPath::new(true);

        CliInstaller::new(home.clone(), &path)
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();

        assert_eq!(
            fs::read_to_string(home.customizations_dir().join("overview.json")).unwrap(),
            "{\"version\":1}"
        );
        assert!(!home.customizations_dir().join("workflow.json").exists());
    }

    #[test]
    fn meldet_eine_unvollstandige_quelle_verstandlich() {
        let plugin = Plugin::new();
        fs::remove_file(plugin.root().join("bin").join(BINARY_NAME)).unwrap();
        let path = FakeUserPath::new(true);

        let error = CliInstaller::new(plugin.home(), &path)
            .install(&plugin.root(), InstallOptions::default())
            .unwrap_err();

        assert_eq!(error.code(), Some("install-source-missing"));
        assert!(error.to_string().contains("bin/"), "{error}");
    }

    #[test]
    fn das_ergebnis_nennt_dieselben_schlussel_wie_typescript() {
        let plugin = Plugin::new();
        let path = FakeUserPath::new(true);
        let result = CliInstaller::new(plugin.home(), &path)
            .install(&plugin.root(), InstallOptions::default())
            .unwrap();

        let json = serde_json::to_value(&result).unwrap();
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "appDir",
                "binDir",
                "launchers",
                "pathAdded",
                "pathSkipped",
                "rulesDir"
            ]
        );
    }
}
