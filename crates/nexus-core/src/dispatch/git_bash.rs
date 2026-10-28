//! Sucht die `bash.exe` von Git for Windows; portiert aus
//! `packages/core/src/dispatch/GitBashLocator.ts`.
//!
//! Dateisystem und Befehlssuche sind austauschbar, damit die Tests die Suche auf jeder
//! Plattform durchspielen können. Die Pfade sind Windows-Pfade, deshalb rechnen die
//! Hilfsfunktionen hier mit `\` und `/` statt mit `std::path` (das unter Unix `\` nicht
//! als Trenner kennt).

type Exists = Box<dyn Fn(&str) -> bool + Send + Sync>;
type Which = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Findet die Git Bash: Einstellung, Git aus dem Suchpfad, bekannte Installationsorte
pub struct GitBashLocator {
    exists: Exists,
    which: Which,
}

impl Default for GitBashLocator {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for GitBashLocator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GitBashLocator")
    }
}

impl GitBashLocator {
    /// Sucht im echten Dateisystem und mit `where`
    pub fn new() -> Self {
        Self {
            exists: Box::new(|path| std::path::Path::new(path).exists()),
            which: Box::new(where_command),
        }
    }

    /// Mit eigenem Dateisystem und eigener Befehlssuche (für Tests)
    pub fn with(exists: Exists, which: Which) -> Self {
        Self { exists, which }
    }

    /// Pfad der Git Bash oder `None` (nicht unter Windows oder nichts gefunden).
    /// `platform` ist der Name der Plattform wie in Node (`win32`, `darwin`, `linux`).
    pub fn locate(&self, env: &dyn EnvLookup, platform: &str) -> Option<String> {
        if platform != "win32" {
            return None;
        }
        if let Some(given) = env.get("CLAUDE_CODE_GIT_BASH_PATH") {
            let given = given.trim().to_string();
            if !given.is_empty() && (self.exists)(&given) {
                return Some(given);
            }
        }
        if let Some(neben_git) = self.neben_git() {
            return Some(neben_git);
        }
        let local = env
            .get("LOCALAPPDATA")
            .map(|dir| win_join(&dir, "Programs"));
        let roots = [env.get("ProgramFiles"), env.get("ProgramFiles(x86)"), local];
        for root in roots.into_iter().flatten() {
            let candidate = win_join(&win_join(&root, "Git"), "bin\\bash.exe");
            if (self.exists)(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    /// `bash.exe` neben dem im Suchpfad gefundenen `git` (…\cmd, …\mingw64\bin oder …\bin → Git-Wurzel)
    fn neben_git(&self) -> Option<String> {
        let git = (self.which)("git")?;
        let mut dir = win_dirname(&git);
        let leaf = |path: &str| win_basename(path).to_lowercase();
        if leaf(&dir) == "bin"
            && ["mingw64", "mingw32", "usr"].contains(&leaf(&win_dirname(&dir)).as_str())
        {
            dir = win_dirname(&dir);
        }
        let root = win_dirname(&dir);
        let candidate = win_join(&root, "bin\\bash.exe");
        (self.exists)(&candidate).then_some(candidate)
    }
}

/// Umgebungsvariablen, aus denen die Suche liest; als Trait, damit Tests eine Karte
/// übergeben können und der Server die echte Umgebung.
pub trait EnvLookup {
    /// Wert der Variablen oder `None`
    fn get(&self, name: &str) -> Option<String>;
}

/// Die echte Umgebung des Prozesses
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessEnv;

impl EnvLookup for ProcessEnv {
    fn get(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

impl EnvLookup for std::collections::BTreeMap<String, String> {
    fn get(&self, name: &str) -> Option<String> {
        std::collections::BTreeMap::get(self, name).cloned()
    }
}

impl EnvLookup for std::collections::HashMap<String, String> {
    fn get(&self, name: &str) -> Option<String> {
        std::collections::HashMap::get(self, name).cloned()
    }
}

/// Erster Treffer von `where <Befehl>`; `None`, wenn es den Befehl nicht gibt.
/// Startet ohne Konsolenfenster.
fn where_command(command: &str) -> Option<String> {
    let mut where_exe = std::process::Command::new("where");
    where_exe.arg(command);
    where_exe.stdin(std::process::Stdio::null());
    where_exe.stderr(std::process::Stdio::null());
    crate::process::hide_console(&mut where_exe);
    let output = where_exe.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

/// Hängt zwei Windows-Pfadteile mit `\` aneinander
fn win_join(left: &str, right: &str) -> String {
    let left = left.trim_end_matches(['\\', '/']);
    format!("{left}\\{right}")
}

/// Ordner eines Windows-Pfads (ohne abschließenden Trenner)
fn win_dirname(path: &str) -> String {
    let trimmed = path.trim_end_matches(['\\', '/']);
    match trimmed.rfind(['\\', '/']) {
        Some(index) => trimmed[..index].to_string(),
        None => trimmed.to_string(),
    }
}

/// Letzter Teil eines Windows-Pfads
fn win_basename(path: &str) -> String {
    let trimmed = path.trim_end_matches(['\\', '/']);
    match trimmed.rfind(['\\', '/']) {
        Some(index) => trimmed[index + 1..].to_string(),
        None => trimmed.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Sucher, der nur die genannten Dateien kennt und `git` an einem festen Ort findet
    fn locator(files: &[&str], git: Option<&str>) -> GitBashLocator {
        let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
        let git = git.map(str::to_string);
        GitBashLocator::with(
            Box::new(move |path| files.iter().any(|file| file == path)),
            Box::new(move |_| git.clone()),
        )
    }

    /// Umgebung aus Paaren
    fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn liefert_unter_anderen_systemen_null() {
        let found = locator(&["/bin/bash"], None)
            .locate(&env(&[("CLAUDE_CODE_GIT_BASH_PATH", "/bin/bash")]), "linux");
        assert_eq!(found, None);
    }

    #[test]
    fn nimmt_die_einstellung_wenn_die_datei_existiert() {
        let gesetzt = env(&[("CLAUDE_CODE_GIT_BASH_PATH", "D:\\g\\bash.exe")]);
        assert_eq!(
            locator(&["D:\\g\\bash.exe"], None).locate(&gesetzt, "win32"),
            Some("D:\\g\\bash.exe".to_string())
        );
        assert_eq!(locator(&[], None).locate(&gesetzt, "win32"), None);
    }

    #[test]
    fn leitet_die_git_wurzel_aus_dem_gefundenen_git_ab() {
        for git in [
            "C:\\G\\cmd\\git.exe",
            "C:\\G\\mingw64\\bin\\git.exe",
            "C:\\G\\bin\\git.exe",
        ] {
            let found = locator(&["C:\\G\\bin\\bash.exe"], Some(git)).locate(&env(&[]), "win32");
            assert_eq!(
                found,
                Some("C:\\G\\bin\\bash.exe".to_string()),
                "Git-Wurzel aus {git}"
            );
        }
    }

    #[test]
    fn faellt_auf_bekannte_orte_zurueck() {
        let local = "C:\\Users\\u\\AppData\\Local\\Programs\\Git\\bin\\bash.exe";
        assert_eq!(
            locator(&[local], None).locate(
                &env(&[("LOCALAPPDATA", "C:\\Users\\u\\AppData\\Local")]),
                "win32"
            ),
            Some(local.to_string())
        );
        let program_files = "C:\\Program Files\\Git\\bin\\bash.exe";
        assert_eq!(
            locator(&[program_files], None)
                .locate(&env(&[("ProgramFiles", "C:\\Program Files")]), "win32"),
            Some(program_files.to_string())
        );
        assert_eq!(locator(&[], None).locate(&env(&[]), "win32"), None);
    }
}
