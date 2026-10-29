//! Fremde Prozesse: Läuft eine Prozessnummer noch (verwaiste Sperrdatei)? Und das
//! Starten eines Programms samt allem, was es selbst startet, mit einem Abbruch, der
//! den ganzen Baum erwischt (`process_group`).
//! Der Workspace verbietet `unsafe`, deshalb kommen die Systemaufrufe aus Bibliotheken
//! mit sicherer Schnittstelle: unter Unix `nix`, unter Windows `sysinfo` und `win32job`.

pub mod process_group;

pub use process_group::ProcessGroup;

/// `CREATE_NO_WINDOW`: Kindprozesse unter Windows ohne Konsolenfenster starten.
/// Ohne das Kennzeichen blitzt bei jedem Aufruf kurz ein schwarzes Fenster auf.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Startet den Befehl unter Windows ohne Konsolenfenster (sonst passiert nichts)
pub fn hide_console(command: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

/// Bittet einen einzelnen Prozess um ein geordnetes Ende (SIGTERM unter Unix, unter Windows
/// `taskkill`). Ein Prozess, den es nicht mehr gibt, ist kein Fehler – wie `process.kill` in
/// TypeScript, dessen Fehler der Aufrufer verschluckt.
pub fn terminate(pid: u32) {
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        if let Ok(number) = i32::try_from(pid) {
            let _ = kill(Pid::from_raw(number), Signal::SIGTERM);
        }
    }
    #[cfg(windows)]
    {
        let mut taskkill = std::process::Command::new("taskkill");
        taskkill.args(["/pid", &pid.to_string(), "/T", "/F"]);
        taskkill.stdin(std::process::Stdio::null());
        taskkill.stdout(std::process::Stdio::null());
        taskkill.stderr(std::process::Stdio::null());
        hide_console(&mut taskkill);
        let _ = taskkill.status();
    }
}

/// Läuft ein Prozess mit dieser Nummer? Gehört er jemand anderem, zählt er als laufend.
#[cfg(unix)]
pub(crate) fn is_alive(pid: u32) -> bool {
    use nix::errno::Errno;
    use nix::sys::signal::kill;
    use nix::unistd::Pid;

    let Ok(number) = i32::try_from(pid) else {
        return false;
    };
    match kill(Pid::from_raw(number), None) {
        Ok(()) => true,
        // EPERM: der Prozess läuft, gehört aber einem anderen Benutzer
        Err(Errno::EPERM) => true,
        Err(_) => false,
    }
}

/// Läuft ein Prozess mit dieser Nummer? Gefragt wird nur nach dieser einen Nummer,
/// damit nicht die ganze Prozessliste eingelesen wird.
#[cfg(windows)]
pub(crate) fn is_alive(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

    let wanted = Pid::from_u32(pid);
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[wanted]),
        true,
        ProcessRefreshKind::nothing(),
    );
    system.process(wanted).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erkennt_den_eigenen_prozess() {
        assert!(is_alive(std::process::id()));
    }

    #[test]
    fn erkennt_eine_nummer_ohne_prozess() {
        // Dieselbe Nummer wie im TypeScript-Test: hoch genug, dass sie nie vergeben ist
        assert!(!is_alive(2_147_483_646));
    }
}
