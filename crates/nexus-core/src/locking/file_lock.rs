//! Sperre über eine Datei, die atomar angelegt wird. Das Protokoll ist dasselbe wie in
//! `packages/core/src/locking/FileLock.ts`: gleicher Dateiname, gleicher Inhalt
//! (`{"pid":…,"at":…,"token":…}`), gleiche Wartezeiten und gleiches Aufräumen verwaister
//! Sperren. Nur so können ein Node- und ein Rust-Prozess dieselbe Sperrdatei nutzen und
//! sich gegenseitig ausschließen. Bewusst keine Bibliothek wie `fs4`: deren Sperren sind
//! andere (`flock`/`LockFileEx`) und für TypeScript unsichtbar.

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{Error, HarnessError, Result};
use crate::process;
use crate::random;

/// Fehler, die „die Sperre hält gerade jemand anderes“ bedeuten: EEXIST, EPERM und EACCES
/// haben in Rust eine eigene Art, für EBUSY gibt es nur die Nummer des Systemfehlers
#[cfg(unix)]
const BUSY_OS_ERRORS: &[i32] = &[16]; // EBUSY

/// Entsprechung unter Windows: die Datei ist von einem anderen Prozess gehalten
#[cfg(windows)]
const BUSY_OS_ERRORS: &[i32] = &[32, 33]; // ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION

/// Einstellungen der Sperre
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileLockOptions {
    /// Wie lange auf eine fremde Sperre gewartet wird (Standard 10 s)
    pub timeout_ms: u64,
    /// Ab welchem Alter eine Sperre als verwaist gilt (Standard 5 s)
    pub stale_ms: u64,
}

impl Default for FileLockOptions {
    fn default() -> Self {
        Self {
            timeout_ms: 10_000,
            stale_ms: 5_000,
        }
    }
}

/// Inhalt der Sperrdatei; Feldnamen und Reihenfolge wie in TypeScript
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockInfo {
    pid: u32,
    at: i64,
    token: String,
}

/// Testhaken: läuft einmal vor dem nächsten Umbenennen bzw. Verknüpfen der Sperrdatei
#[cfg(test)]
type Hook = Box<dyn FnMut() + Send>;

/// Sperre über eine Datei, die atomar angelegt wird (`create_new`).
/// Funktioniert unter Windows, macOS und Linux gleich.
pub struct FileLock {
    lock_path: PathBuf,
    timeout_ms: u64,
    stale_ms: u64,
    /// Gleichzeitige Aktionen im selben Prozess werden hintereinander gestellt
    queue: Mutex<()>,
    #[cfg(test)]
    before_rename: Mutex<Option<Hook>>,
    #[cfg(test)]
    before_link: Mutex<Option<Hook>>,
}

/// Von Hand geschrieben, weil die Testhaken keine Ausgabe haben
impl std::fmt::Debug for FileLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileLock")
            .field("lock_path", &self.lock_path)
            .field("timeout_ms", &self.timeout_ms)
            .field("stale_ms", &self.stale_ms)
            .finish()
    }
}

impl FileLock {
    /// Sperre an diesem Pfad; `FileLockOptions::default()` übernimmt die Werte aus TypeScript
    pub fn new(path: impl Into<PathBuf>, options: FileLockOptions) -> Self {
        Self {
            lock_path: path.into(),
            timeout_ms: options.timeout_ms,
            stale_ms: options.stale_ms,
            queue: Mutex::new(()),
            #[cfg(test)]
            before_rename: Mutex::new(None),
            #[cfg(test)]
            before_link: Mutex::new(None),
        }
    }

    /// Pfad der Sperrdatei
    pub fn path(&self) -> &Path {
        &self.lock_path
    }

    /// Führt `action` unter der Sperre aus und gibt sie danach immer wieder frei,
    /// auch wenn die Aktion scheitert
    pub fn with_lock<T>(&self, action: impl FnOnce() -> Result<T>) -> Result<T> {
        // Eine vergiftete Warteschlange heißt nur, dass eine frühere Aktion abgestürzt ist
        let _queued = self
            .queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let token = self.acquire()?;
        let _held = Held {
            lock: self,
            token: &token,
        };
        action()
    }

    /// Wartet mit wachsender Pause, bis die Sperre angelegt werden kann oder die Zeit abläuft
    fn acquire(&self) -> Result<String> {
        let deadline = now_ms() + i64::try_from(self.timeout_ms).unwrap_or(i64::MAX);
        let mut pause = 5.0_f64;
        loop {
            if let Some(token) = self.try_create()? {
                return Ok(token);
            }
            if let Some(stale) = self.stale_lock() {
                self.remove_stale(stale.as_deref());
                continue;
            }
            if now_ms() >= deadline {
                return Err(HarnessError::new(
                    "lock-timeout",
                    format!(
                        "Eine andere Aktion hält die Sperre ({}). Bitte später erneut versuchen.",
                        self.lock_path.display()
                    ),
                )
                .into());
            }
            sleep(Duration::from_millis(
                (pause + random::fraction() * pause) as u64,
            ));
            pause = (pause * 2.0).min(100.0);
        }
    }

    /// Legt die Sperrdatei atomar an; liefert das Kennzeichen oder None, wenn sie schon existiert
    fn try_create(&self) -> Result<Option<String>> {
        let token = random::hex(8);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.lock_path)
        {
            Ok(mut file) => {
                let info = LockInfo {
                    pid: std::process::id(),
                    at: now_ms(),
                    token: token.clone(),
                };
                let json = serde_json::to_string(&info).map_err(Error::unexpected)?;
                file.write_all(json.as_bytes()).map_err(Error::unexpected)?;
                Ok(Some(token))
            }
            Err(error) if is_busy(&error) => Ok(None),
            Err(error) => Err(Error::unexpected(error)),
        }
    }

    /// Verwaist: Prozess existiert nicht mehr oder die Sperre ist älter als `stale_ms`.
    /// `Some(Some(kennzeichen))` ist eine verwaiste Sperre, `Some(None)` eine verwaiste
    /// ohne lesbares Kennzeichen, `None` heißt „nicht verwaist“.
    fn stale_lock(&self) -> Option<Option<String>> {
        let info = read_info(&self.lock_path);
        if let Some(found) = &info {
            if !process::is_alive(found.pid) {
                return Some(Some(found.token.clone()));
            }
        }
        // Ist die Datei inzwischen weg, klappt der nächste Versuch von allein
        let age = now_ms() - modified_ms(&self.lock_path)?;
        if age > i64::try_from(self.stale_ms).unwrap_or(i64::MAX) {
            Some(info.map(|found| found.token))
        } else {
            None
        }
    }

    /// Entfernt eine verwaiste Sperre. Sie wird zuerst beiseite umbenannt (das gelingt nur einem
    /// Prozess), erst dann wird das Kennzeichen aus der beiseite gelegten Datei gelesen. Trägt sie
    /// inzwischen ein anderes Kennzeichen (Halter hat freigegeben, ein anderer Prozess hat sie neu
    /// geholt) oder ist sie unlesbar und noch jung, wird sie per Verknüpfung zurückgestellt; eine
    /// Verknüpfung überschreibt nie: Liegt dort schon eine neue Sperre, gilt diese, und die
    /// beiseite gelegte Kopie entfällt.
    /// Die Vorprüfung vor dem Umbenennen ist nötig: Ohne sie würde eine längst neu vergebene Sperre
    /// kurz beiseite gelegt, und in dieser Lücke könnte ein dritter Prozess eine zweite Sperre anlegen.
    fn remove_stale(&self, expected_token: Option<&str>) {
        if read_info(&self.lock_path).map(|info| info.token).as_deref() != expected_token {
            return;
        }
        let aside = self.aside_path();
        #[cfg(test)]
        self.run_hook(&self.before_rename);
        if fs::rename(&self.lock_path, &aside).is_err() {
            return; // Ein anderer Prozess war schneller oder die Sperre ist schon weg
        }
        if !self.is_stale_aside(&aside, expected_token) {
            self.restore(&aside);
        }
        let _ = fs::remove_file(&aside);
    }

    /// Name der beiseite gelegten Sperre; die Prozessnummer und der Zufallsteil trennen
    /// gleichzeitige Entferner voneinander
    fn aside_path(&self) -> PathBuf {
        let mut name = OsString::from(self.lock_path.as_os_str());
        name.push(format!(".stale.{}.{}", std::process::id(), random::hex(4)));
        PathBuf::from(name)
    }

    /// Ist die beiseite gelegte Sperre noch die erwartete verwaiste?
    /// Ohne lesbares Kennzeichen zählt nur das Alter.
    fn is_stale_aside(&self, aside: &Path, expected_token: Option<&str>) -> bool {
        let token = read_info(aside).map(|info| info.token);
        if token.as_deref() != expected_token {
            return false;
        }
        if token.is_some() {
            return true;
        }
        match modified_ms(aside) {
            Some(modified) => {
                now_ms() - modified > i64::try_from(self.stale_ms).unwrap_or(i64::MAX)
            }
            None => false,
        }
    }

    /// Stellt die beiseite gelegte Sperre zurück, ohne eine inzwischen neu angelegte zu überschreiben
    fn restore(&self, aside: &Path) {
        #[cfg(test)]
        self.run_hook(&self.before_link);
        // Liegt dort schon eine neue Sperre, scheitert das Verknüpfen und die neue gilt. Bei anderen
        // Fehlern lieber die alte Sperre verlieren als eine fremde überschreiben.
        let _ = fs::hard_link(aside, &self.lock_path);
    }

    /// Gibt die Sperre frei, aber nur, wenn sie noch unser Kennzeichen trägt
    fn release(&self, token: &str) {
        if let Some(info) = read_info(&self.lock_path) {
            if info.token != token {
                return;
            }
        }
        for _ in 0..20 {
            match fs::remove_file(&self.lock_path) {
                Ok(()) => return,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return,
                // Windows: Datei kurz von Virenscanner oder Indexdienst gehalten
                Err(_) => sleep(Duration::from_millis(10)),
            }
        }
    }

    /// Setzt den Testhaken vor dem nächsten Umbenennen der Sperrdatei
    #[cfg(test)]
    pub(crate) fn set_before_rename(&self, hook: impl FnMut() + Send + 'static) {
        *self.before_rename.lock().unwrap() = Some(Box::new(hook));
    }

    /// Setzt den Testhaken vor dem nächsten Verknüpfen der Sperrdatei
    #[cfg(test)]
    pub(crate) fn set_before_link(&self, hook: impl FnMut() + Send + 'static) {
        *self.before_link.lock().unwrap() = Some(Box::new(hook));
    }

    /// Führt einen gesetzten Testhaken genau einmal aus (wie der Mock in TypeScript)
    #[cfg(test)]
    fn run_hook(&self, slot: &Mutex<Option<Hook>>) {
        let hook = slot.lock().unwrap().take();
        if let Some(mut hook) = hook {
            hook();
        }
    }
}

/// Gibt die Sperre frei, sobald die Aktion endet – auch bei Fehler oder Absturz der Aktion
struct Held<'a> {
    lock: &'a FileLock,
    token: &'a str,
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        self.lock.release(self.token);
    }
}

/// Liest die Angaben in einer Sperrdatei (None, wenn sie fehlt oder unlesbar ist)
fn read_info(path: &Path) -> Option<LockInfo> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// Änderungszeit der Datei in Millisekunden seit 1970 (None, wenn es sie nicht gibt)
fn modified_ms(path: &Path) -> Option<i64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let since = modified.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(since.as_millis()).ok()
}

/// Millisekunden seit 1970, wie `Date.now()` in JavaScript
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since| i64::try_from(since.as_millis()).ok())
        .unwrap_or(0)
}

/// Hält gerade jemand anderes die Sperrdatei?
fn is_busy(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::AlreadyExists | io::ErrorKind::PermissionDenied
    ) || matches!(error.raw_os_error(), Some(code) if BUSY_OS_ERRORS.contains(&code))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// Temporärer Ordner und Pfad der Sperre darin
    fn temp_lock(prefix: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
        let path = dir.path().join("x.lock");
        (dir, path)
    }

    /// Schreibt eine fremde Sperrdatei mit diesem Kennzeichen
    fn write_lock(path: &Path, token: &str, at: i64) {
        let info = LockInfo {
            pid: std::process::id(),
            at,
            token: token.into(),
        };
        fs::write(path, serde_json::to_string(&info).unwrap()).unwrap();
    }

    /// Kennzeichen in der Sperrdatei
    fn token_of(path: &Path) -> String {
        read_info(path).unwrap().token
    }

    /// Beiseite gelegte Sperren, die liegen geblieben sind
    fn leftovers(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".stale."))
            .collect();
        names.sort();
        names
    }

    /// Setzt die Änderungszeit einer Datei um `millis` zurück
    fn age_file(path: &Path, millis: u64) {
        let file = fs::File::options().write(true).open(path).unwrap();
        let old = SystemTime::now() - Duration::from_millis(millis);
        file.set_modified(old).unwrap();
    }

    #[test]
    fn legt_die_sperre_an_und_gibt_sie_nach_der_aktion_wieder_frei() {
        let (_dir, path) = temp_lock("harness-lock-");
        let lock = FileLock::new(&path, FileLockOptions::default());

        let result = lock
            .with_lock(|| {
                assert!(path.exists());
                Ok(42)
            })
            .unwrap();

        assert_eq!(result, 42);
        assert!(!path.exists());
    }

    #[test]
    fn sperrt_und_gibt_frei_in_einem_ordner_mit_sonderzeichen() {
        let base = tempfile::Builder::new()
            .prefix("harness-win-")
            .tempdir()
            .unwrap();
        let dir = base.path().join("Mein Ordner ä ö ü ß");
        fs::create_dir(&dir).unwrap();
        let lock = FileLock::new(dir.join("ä.lock"), FileLockOptions::default());

        lock.with_lock(|| Ok(())).unwrap();

        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    }

    #[test]
    fn gibt_die_sperre_auch_bei_einem_fehler_in_der_aktion_frei() {
        let (_dir, path) = temp_lock("harness-lock-");
        let lock = FileLock::new(&path, FileLockOptions::default());

        let error = lock
            .with_lock(|| Err::<(), _>(Error::unexpected_message("kaputt")))
            .unwrap_err();

        assert_eq!(error.to_string(), "kaputt");
        assert!(!path.exists());
    }

    #[test]
    fn wartet_bis_eine_fremde_sperre_freigegeben_wird() {
        let (_dir, path) = temp_lock("harness-lock-");
        write_lock(&path, "fremd", now_ms());
        let lock = FileLock::new(
            &path,
            FileLockOptions {
                timeout_ms: 3000,
                ..FileLockOptions::default()
            },
        );

        std::thread::scope(|scope| {
            let freigeben = path.clone();
            scope.spawn(move || {
                sleep(Duration::from_millis(150));
                fs::remove_file(&freigeben).unwrap();
            });
            lock.with_lock(|| Ok(())).unwrap();
        });

        assert!(!path.exists());
    }

    #[test]
    fn raumt_eine_sperre_eines_beendeten_prozesses_auf() {
        let (_dir, path) = temp_lock("harness-lock-");
        let info = LockInfo {
            pid: 2_147_483_646,
            at: now_ms(),
            token: "tot".into(),
        };
        fs::write(&path, serde_json::to_string(&info).unwrap()).unwrap();
        let lock = FileLock::new(
            &path,
            FileLockOptions {
                timeout_ms: 3000,
                ..FileLockOptions::default()
            },
        );

        lock.with_lock(|| Ok(())).unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn raumt_eine_zu_alte_sperre_auf_auch_wenn_der_prozess_noch_lauft() {
        let (_dir, path) = temp_lock("harness-lock-");
        write_lock(&path, "alt", now_ms() - 60_000);
        age_file(&path, 60_000);
        let lock = FileLock::new(
            &path,
            FileLockOptions {
                timeout_ms: 3000,
                stale_ms: 500,
            },
        );

        lock.with_lock(|| Ok(())).unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn wirft_nach_der_wartezeit_einen_harness_fehler() {
        let (_dir, path) = temp_lock("harness-lock-");
        write_lock(&path, "frisch", now_ms());
        let lock = FileLock::new(
            &path,
            FileLockOptions {
                timeout_ms: 300,
                stale_ms: 60_000,
            },
        );

        let error = lock.with_lock(|| Ok(())).unwrap_err();

        assert_eq!(error.code(), Some("lock-timeout"));
        assert!(error.to_string().contains("Sperre"), "{error}");
    }

    #[test]
    fn serialisiert_gleichzeitige_aktionen_im_selben_prozess() {
        let (_dir, path) = temp_lock("harness-lock-");
        let lock = FileLock::new(
            &path,
            FileLockOptions {
                timeout_ms: 5000,
                ..FileLockOptions::default()
            },
        );
        let running = AtomicUsize::new(0);
        let max_running = AtomicUsize::new(0);

        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    lock.with_lock(|| {
                        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                        max_running.fetch_max(now, Ordering::SeqCst);
                        sleep(Duration::from_millis(10));
                        running.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
                    })
                    .unwrap();
                });
            }
        });

        assert_eq!(max_running.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn entfernt_die_sperre_wenn_das_kennzeichen_noch_das_verwaiste_ist() {
        let (dir, path) = temp_lock("harness-lock-");
        write_lock(&path, "alt", now_ms());

        FileLock::new(&path, FileLockOptions::default()).remove_stale(Some("alt"));

        assert!(!path.exists());
        assert!(leftovers(dir.path()).is_empty());
    }

    #[test]
    fn kennzeichen_geandert_lasst_die_sperre_stehen() {
        let (dir, path) = temp_lock("harness-lock-");
        write_lock(&path, "alt", now_ms());
        let lock = FileLock::new(&path, FileLockOptions::default());
        // Der alte Halter gibt frei, ein anderer Prozess holt die Sperre neu – genau vor dem Umbenennen
        let changed = path.clone();
        lock.set_before_rename(move || {
            fs::remove_file(&changed).unwrap();
            write_lock(&changed, "neu", now_ms());
        });

        lock.remove_stale(Some("alt"));

        assert!(path.exists());
        assert_eq!(token_of(&path), "neu");
        assert!(leftovers(dir.path()).is_empty());
    }

    #[test]
    fn legt_eine_schon_neu_vergebene_sperre_gar_nicht_erst_beiseite() {
        let (_dir, path) = temp_lock("harness-lock-");
        write_lock(&path, "neu", now_ms());
        let lock = FileLock::new(&path, FileLockOptions::default());
        let renamed = std::sync::Arc::new(AtomicUsize::new(0));
        let gemerkt = renamed.clone();
        lock.set_before_rename(move || {
            gemerkt.fetch_add(1, Ordering::SeqCst);
        });

        lock.remove_stale(Some("alt"));

        assert_eq!(renamed.load(Ordering::SeqCst), 0);
        assert_eq!(token_of(&path), "neu");
    }

    #[test]
    fn zwei_entferner_gewinnen_nicht_beide() {
        let (dir, path) = temp_lock("harness-lock-");
        write_lock(&path, "alt", now_ms());
        let first = FileLock::new(&path, FileLockOptions::default());
        // Beide haben „alt“ als verwaist erkannt. Während der erste umbenennen will, räumt der
        // zweite auf und ein dritter Prozess holt die Sperre neu.
        let zweiter_pfad = path.clone();
        first.set_before_rename(move || {
            let second = FileLock::new(&zweiter_pfad, FileLockOptions::default());
            second.remove_stale(Some("alt"));
            assert!(!zweiter_pfad.exists());
            write_lock(&zweiter_pfad, "neu", now_ms());
        });

        first.remove_stale(Some("alt"));

        assert!(path.exists());
        assert_eq!(token_of(&path), "neu");
        assert!(leftovers(dir.path()).is_empty());
    }

    #[test]
    fn gleichzeitige_entferner_enden_ohne_fehler_und_ohne_reste() {
        let (dir, path) = temp_lock("harness-lock-");
        write_lock(&path, "alt", now_ms());

        std::thread::scope(|scope| {
            for _ in 0..2 {
                let pfad = path.clone();
                scope.spawn(move || {
                    FileLock::new(&pfad, FileLockOptions::default()).remove_stale(Some("alt"));
                });
            }
        });

        assert!(!path.exists());
        assert!(leftovers(dir.path()).is_empty());
    }

    #[test]
    fn stellt_die_sperre_wieder_her_ohne_eine_neue_zu_uberschreiben() {
        let (dir, path) = temp_lock("harness-lock-");
        write_lock(&path, "alt", now_ms());
        let lock = FileLock::new(&path, FileLockOptions::default());
        // Vor dem Umbenennen wechselt das Kennzeichen; während die Sperre beiseite liegt,
        // holt ein anderer sie neu
        let zwischen = path.clone();
        lock.set_before_rename(move || write_lock(&zwischen, "zwischen", now_ms()));
        let dritter = path.clone();
        lock.set_before_link(move || write_lock(&dritter, "dritter", now_ms()));

        lock.remove_stale(Some("alt"));

        assert_eq!(token_of(&path), "dritter");
        assert!(leftovers(dir.path()).is_empty());
    }

    #[test]
    fn unlesbare_sperre_nur_entfernen_wenn_sie_zu_alt_ist() {
        let (dir, path) = temp_lock("harness-lock-");
        fs::write(&path, "").unwrap();
        let options = FileLockOptions {
            stale_ms: 60_000,
            ..FileLockOptions::default()
        };

        FileLock::new(&path, options).remove_stale(None);
        assert!(path.exists());
        assert!(leftovers(dir.path()).is_empty());

        age_file(&path, 120_000);
        FileLock::new(&path, options).remove_stale(None);
        assert!(!path.exists());
        assert!(leftovers(dir.path()).is_empty());
    }
}
