//! Prüft, ob ein Prozess mit einer bestimmten Nummer noch läuft. Das entspricht
//! `process.kill(pid, 0)` in Node und entscheidet, ob eine Sperrdatei verwaist ist.
//! Der Workspace verbietet `unsafe`, deshalb kommt der Systemaufruf aus einer
//! Bibliothek mit sicherer Schnittstelle: unter Unix `nix`, unter Windows `sysinfo`.

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
