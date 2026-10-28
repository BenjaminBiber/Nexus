//! Startet ein Programm so, dass es sich später samt allem, was es selbst gestartet hat,
//! wieder beenden lässt – auf jeder Plattform auf dem dort üblichen Weg:
//!
//! - **Unix:** eigene Prozessgruppe (`process_group(0)`), Abbruch mit `killpg(SIGKILL)`.
//!   Das entspricht `process.kill(-pid, 'SIGKILL')` in Node.
//! - **Windows:** `CREATE_NO_WINDOW` beim Start, danach ein eigenes Job-Objekt mit
//!   „Kill on job close“. Der Abbruch schließt das Job-Objekt und beendet damit jeden
//!   Prozess darin. Scheitert die Zuordnung zum Job (z. B. weil der Server selbst in einem
//!   Job ohne Breakaway läuft), bleibt `taskkill /pid <pid> /T /F` als Rückfall – wie heute
//!   in TypeScript.
//!
//! Zwischen Start und Zuordnung zum Job liegt unter Windows ein kurzes Fenster, in dem ein
//! sofort gestartetes Enkelkind nicht im Job landet. Das ist hinnehmbar: Claude und die
//! Einrichtungsbefehle starten in den ersten Millisekunden keine Kinder.
//!
//! Das Job-Objekt lebt so lange wie die `ProcessGroup`. Endet der Worker unerwartet,
//! schließt Windows das Handle und beendet dank „Kill on job close“ auch Claude.

use std::io;
use std::process::ExitStatus;

use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

/// Ein gestartetes Programm samt seiner Prozessgruppe bzw. seinem Job-Objekt
#[derive(Debug)]
pub struct ProcessGroup {
    child: Child,
    /// Das Job-Objekt des Prozesses; `None`, wenn die Zuordnung scheiterte (dann `taskkill`)
    #[cfg(windows)]
    job: Option<win32job::Job>,
}

impl ProcessGroup {
    /// Startet den Befehl in einer eigenen Prozessgruppe bzw. einem eigenen Job-Objekt.
    /// `stdin`, `stdout` und `stderr` legt der Aufrufer vorher fest.
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(unix)]
        {
            // Eigene Prozessgruppe: killpg erwischt später auch die Enkelkinder
            command.process_group(0);
            Ok(Self {
                child: command.spawn()?,
            })
        }
        #[cfg(windows)]
        {
            // creation_flags bringt tokio::process::Command unter Windows selbst mit
            command.creation_flags(super::CREATE_NO_WINDOW);
            let child = command.spawn()?;
            let job = Self::assign_job(&child);
            Ok(Self { child, job })
        }
        #[cfg(not(any(unix, windows)))]
        {
            Ok(Self {
                child: command.spawn()?,
            })
        }
    }

    /// Legt ein Job-Objekt an und ordnet den frisch gestarteten Prozess zu; `None`, wenn das
    /// Betriebssystem das ablehnt (dann beendet `kill_tree` den Baum mit `taskkill`)
    #[cfg(windows)]
    fn assign_job(child: &Child) -> Option<win32job::Job> {
        use win32job::{ExtendedLimitInfo, Job};

        let handle = child.raw_handle()?;
        let mut info = ExtendedLimitInfo::new();
        // Kill on job close: wird das Handle geschlossen, beendet Windows alles im Job
        info.limit_kill_on_job_close();
        let job = Job::create_with_limit_info(&info).ok()?;
        job.assign_process(handle as isize).ok()?;
        Some(job)
    }

    /// Prozessnummer, solange der Prozess läuft
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// Nimmt die Eingabe des Prozesses heraus (nur beim ersten Aufruf vorhanden)
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    /// Nimmt die Ausgabe des Prozesses heraus (nur beim ersten Aufruf vorhanden)
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    /// Nimmt die Fehlerausgabe des Prozesses heraus (nur beim ersten Aufruf vorhanden)
    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    /// Wartet auf das Ende des Prozesses
    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait().await
    }

    /// Bittet den Prozess samt allem, was er gestartet hat, um ein geordnetes Ende (SIGTERM an
    /// die Gruppe). Windows kennt kein SIGTERM; dort beendet das Job-Objekt den Baum sofort,
    /// deshalb ist das dort dasselbe wie [`Self::kill_tree`].
    pub fn terminate_tree(&mut self) {
        #[cfg(unix)]
        {
            use nix::sys::signal::{killpg, Signal};
            use nix::unistd::Pid;

            let Some(pid) = self.child.id() else { return };
            let Ok(number) = i32::try_from(pid) else {
                return;
            };
            let _ = killpg(Pid::from_raw(number), Signal::SIGTERM);
        }
        #[cfg(not(unix))]
        {
            self.kill_tree();
        }
    }

    /// Beendet den Prozess samt allem, was er gestartet hat (Tests, Builds).
    /// Ist schon alles beendet, passiert nichts.
    pub fn kill_tree(&mut self) {
        let Some(pid) = self.child.id() else { return };
        #[cfg(unix)]
        {
            use nix::sys::signal::{killpg, Signal};
            use nix::unistd::Pid;

            let Ok(number) = i32::try_from(pid) else {
                return;
            };
            // Fehler „schon beendet“ (ESRCH) ist der Normalfall und wird übergangen
            let _ = killpg(Pid::from_raw(number), Signal::SIGKILL);
        }
        #[cfg(windows)]
        {
            // Das Job-Objekt schließen beendet jeden Prozess darin (Kill on job close)
            if self.job.take().is_some() {
                return;
            }
            // Ohne Job-Objekt wie bisher: taskkill beendet den Baum, ohne Fenster zu zeigen
            let mut taskkill = std::process::Command::new("taskkill");
            taskkill.args(["/pid", &pid.to_string(), "/T", "/F"]);
            taskkill.stdin(std::process::Stdio::null());
            taskkill.stdout(std::process::Stdio::null());
            taskkill.stderr(std::process::Stdio::null());
            super::hide_console(&mut taskkill);
            let _ = taskkill.status();
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = pid;
        }
    }

    /// Beendet nur den gestarteten Prozess selbst (harter Abbruch, wenn der Baum nicht greift)
    pub async fn kill(&mut self) {
        let _ = self.child.kill().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::is_alive;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    use tokio::io::{AsyncBufReadExt, BufReader};

    /// Startet eine Prozesskette, deren Kind selbst ein Enkelkind startet; das Kind
    /// schreibt die Nummer des Enkelkinds auf die Ausgabe.
    #[cfg(unix)]
    fn mit_enkelkind() -> Command {
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg("sleep 600 & echo $!; sleep 600");
        command
    }

    /// Gleiches unter Windows: `Start-Process` startet das Enkelkind und liefert dessen Nummer
    #[cfg(windows)]
    fn mit_enkelkind() -> Command {
        let mut command = Command::new("powershell");
        command.args([
            "-NoProfile",
            "-Command",
            "$p = Start-Process -FilePath ping -ArgumentList '-n','600','127.0.0.1' -PassThru; Write-Output $p.Id; Start-Sleep -Seconds 600",
        ]);
        command
    }

    /// Wartet, bis die Nummer keinen laufenden Prozess mehr bezeichnet (höchstens `frist`)
    async fn warte_auf_ende(pid: u32, frist: Duration) -> bool {
        let bis = Instant::now() + frist;
        while Instant::now() < bis {
            if !is_alive(pid) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        !is_alive(pid)
    }

    #[tokio::test]
    async fn beendet_auch_das_enkelkind() {
        let mut command = mit_enkelkind();
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::null());
        let mut group = ProcessGroup::spawn(&mut command).expect("Start");
        let kind = group.id().expect("Prozessnummer");
        let mut zeilen = BufReader::new(group.take_stdout().expect("Ausgabe")).lines();
        let erste = tokio::time::timeout(Duration::from_secs(30), zeilen.next_line())
            .await
            .expect("Ausgabe des Kindes")
            .expect("Ausgabe lesbar")
            .expect("eine Zeile");
        let enkel: u32 = erste.trim().parse().expect("Nummer des Enkelkinds");
        assert!(is_alive(enkel), "Enkelkind läuft");

        group.kill_tree();
        // Auf das Ende warten, damit der beendete Prozess aus der Prozessliste verschwindet
        // (unter Unix bleibt er sonst als Zombie stehen und gilt weiter als laufend)
        let _ = tokio::time::timeout(Duration::from_secs(10), group.wait()).await;

        assert!(
            warte_auf_ende(kind, Duration::from_secs(10)).await,
            "Kind beendet"
        );
        assert!(
            warte_auf_ende(enkel, Duration::from_secs(10)).await,
            "Enkelkind beendet"
        );
    }

    #[tokio::test]
    async fn abbruch_eines_beendeten_prozesses_ist_still() {
        let mut command = Command::new(if cfg!(windows) { "cmd" } else { "/bin/sh" });
        if cfg!(windows) {
            command.args(["/d", "/s", "/c", "exit 0"]);
        } else {
            command.arg("-c").arg("exit 0");
        }
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        let mut group = ProcessGroup::spawn(&mut command).expect("Start");
        let status = group.wait().await.expect("Ende");
        assert_eq!(status.code(), Some(0));
        group.kill_tree();
        group.kill().await;
    }
}
