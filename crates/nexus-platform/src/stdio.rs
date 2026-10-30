//! Standard-Ein- und -Ausgaben dieses Prozesses: Kindprozesse bekommen sie nur ausdrücklich.
//!
//! Windows gibt einem Kindprozess alle vererbbaren Handles mit, nicht nur die drei, die ihm
//! `Command` zuweist. Die Standard-Handles, die ein Programm von seinem Aufrufer bekommt, sind
//! vererbbar – bei Claude Code oder `spawnSync` in Node etwa die Pipes, auf deren Ende der
//! Aufrufer wartet. Startet `nexus` dann abgelöst den Server, hält der diese Pipes offen, und der
//! Aufrufer wartet bis zum Ende des Servers. Node schaltet die Vererbung deshalb beim Start ab
//! (`uv_disable_stdio_inheritance`); [`disable_inheritance`] macht das nach.
//!
//! Wer einem Kind die Standard-Handles mitgeben will (`Stdio::inherit()`), bekommt sie weiter:
//! die Standardbibliothek dupliziert sie dafür als vererbbar.
//!
//! Die stabile Standardbibliothek kann das noch nicht selbst (`CommandExt::inherit_handles` ist
//! instabil, rust-lang/rust#146407). Deshalb steht hier die einzige Ausnahme vom
//! `unsafe`-Verbot des Workspace.

/// Schaltet die Vererbung der Standard-Handles dieses Prozesses ab; gehört an den Anfang von
/// `main`, vor den ersten Kindprozess. Unter Unix gibt es nichts zu tun: die Standardbibliothek
/// setzt die Ein- und Ausgaben des Kindes selbst und öffnet ihre Dateien mit `O_CLOEXEC`.
pub fn disable_inheritance() {
    #[cfg(windows)]
    clear_inherit_flags();
}

/// Nimmt Ein-, Aus- und Fehlerausgabe das Merkmal „vererbbar“
#[cfg(windows)]
#[allow(unsafe_code)]
fn clear_inherit_flags() {
    use std::os::windows::io::AsRawHandle;

    use ::windows::Win32::Foundation::{
        SetHandleInformation, HANDLE, HANDLE_FLAGS, HANDLE_FLAG_INHERIT,
    };

    for handle in [
        std::io::stdin().as_raw_handle(),
        std::io::stdout().as_raw_handle(),
        std::io::stderr().as_raw_handle(),
    ] {
        if handle.is_null() {
            continue;
        }
        // SAFETY: Die Handles gehören diesem Prozess und bleiben offen; geändert wird nur ihr
        // Merkmal „vererbbar“. Scheitert der Aufruf (kein gültiges Handle), bleibt alles, wie
        // es war.
        let _ =
            unsafe { SetHandleInformation(HANDLE(handle), HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laesst_die_standardausgabe_benutzbar() {
        use std::io::Write;

        disable_inheritance();
        // Geändert ist nur die Vererbung; schreiben geht weiter
        assert!(std::io::stdout().flush().is_ok());
        assert!(std::io::stderr().flush().is_ok());
    }
}
