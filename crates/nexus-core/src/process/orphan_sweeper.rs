//! Räumt nach einem Dispatch-Lauf Prozesse ab, die aus dessen Worktree übrig geblieben sind;
//! portiert aus `packages/core/src/dispatch/OrphanSweeper.ts`.
//!
//! Etwa ein `nexus serve … &`, das der Agent für eine Prüfung gestartet und nie beendet hat.
//! Beendet werden nur verwaiste Prozesse (Elternprozess beendet) samt ihren Kindern; was noch an
//! einer Shell oder IDE hängt, bleibt.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::Path;
use std::time::Duration;

/// Ein laufender Prozess; `cwd` nur, wo das System ihn preisgibt (nicht unter Windows)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub command: String,
    pub cwd: Option<String>,
}

/// Wie ein Prozess beendet werden soll
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopSignal {
    Term,
    Kill,
}

/// Zugriff auf die Prozesse des Rechners; Tests setzen eine eigene Umsetzung ein
pub trait ProcessSystem: Send + Sync {
    fn list(&self) -> Vec<ProcessInfo>;
    fn cwds(&self, pids: &[u32]) -> HashMap<u32, String>;
    fn signal(&self, pid: u32, signal: StopSignal);
    fn alive(&self, pid: u32) -> bool;
    fn sleep(&self, pause: Duration);
}

/// So lange bekommen die Prozesse nach SIGTERM, bevor SIGKILL folgt
const TERM_WAIT: Duration = Duration::from_secs(3);
/// Abstand der Nachfragen, ob sie schon beendet sind
const POLL: Duration = Duration::from_millis(100);

/// Räumt übrig gebliebene Prozesse eines Worktrees ab
pub struct OrphanSweeper {
    system: Box<dyn ProcessSystem>,
}

impl Default for OrphanSweeper {
    fn default() -> Self {
        Self::new(Box::new(LocalProcesses))
    }
}

impl OrphanSweeper {
    /// Mit eigenem Prozesssystem (für Tests)
    pub fn new(system: Box<dyn ProcessSystem>) -> Self {
        Self { system }
    }

    /// Beendet die übrig gebliebenen Prozesse aus `root` (erst SIGTERM, nach einer Frist
    /// SIGKILL) und liefert ihre Nummern
    pub fn sweep(&self, root: &str, own: u32) -> Vec<u32> {
        let all = self.system.list();
        let orphans: Vec<u32> = all
            .iter()
            .filter(|p| orphaned(p, &all) && p.pid != own)
            .map(|p| p.pid)
            .collect();
        let cwds = if orphans.is_empty() || cfg!(windows) {
            HashMap::new()
        } else {
            self.system.cwds(&orphans)
        };
        let with_cwd: Vec<ProcessInfo> = all
            .into_iter()
            .map(|mut p| {
                if let Some(cwd) = cwds.get(&p.pid) {
                    p.cwd = Some(cwd.clone());
                }
                p
            })
            .collect();
        // lsof und /proc melden echte Pfade (unter macOS etwa /private/var statt /var)
        let real = std::fs::canonicalize(root)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| root.to_string());
        let doomed: Vec<u32> = [root, real.as_str()]
            .iter()
            .flat_map(|r| select(&with_cwd, r, own))
            .collect::<BTreeSet<u32>>()
            .into_iter()
            .collect();
        if doomed.is_empty() {
            return doomed;
        }
        for pid in &doomed {
            self.system.signal(*pid, StopSignal::Term);
        }
        let mut waited = Duration::ZERO;
        while waited < TERM_WAIT && doomed.iter().any(|pid| self.system.alive(*pid)) {
            self.system.sleep(POLL);
            waited += POLL;
        }
        for pid in &doomed {
            if self.system.alive(*pid) {
                self.system.signal(*pid, StopSignal::Kill);
            }
        }
        doomed
    }
}

/// Ist `dir` ein verknüpfter Worktree (`.git` ist eine Datei)? Im Hauptordner wird nie aufgeräumt
pub fn linked_worktree(dir: &Path) -> bool {
    dir.join(".git").is_file()
}

/// Die zu beendenden Prozesse: verwaiste Prozesse aus `root` und alle ihre Nachfahren, nie
/// `own` oder seine Vorfahren
pub fn select(processes: &[ProcessInfo], root: &str, own: u32) -> Vec<u32> {
    let by_pid: HashMap<u32, &ProcessInfo> = processes.iter().map(|p| (p.pid, p)).collect();
    let mut protected = HashSet::new();
    let mut at = by_pid.get(&own);
    while let Some(p) = at {
        if !protected.insert(p.pid) {
            break;
        }
        at = by_pid.get(&p.ppid);
    }
    let mut chosen = Vec::new();
    let mut seen = HashSet::new();
    let mut queue: VecDeque<u32> = processes
        .iter()
        .filter(|p| orphaned(p, processes) && from_root(p, root))
        .map(|p| p.pid)
        .collect();
    while let Some(pid) = queue.pop_front() {
        if protected.contains(&pid) || !seen.insert(pid) {
            continue;
        }
        chosen.push(pid);
        queue.extend(processes.iter().filter(|p| p.ppid == pid).map(|p| p.pid));
    }
    chosen
}

/// Verwaist: der Elternprozess ist init/launchd (PID 1) oder läuft nicht mehr (Windows hängt
/// Waisen nicht um)
pub fn orphaned(p: &ProcessInfo, processes: &[ProcessInfo]) -> bool {
    if p.pid <= 1 {
        return false;
    }
    p.ppid == 1 || !processes.iter().any(|q| q.pid == p.ppid)
}

/// Stammt der Prozess aus `root`: Pfad in der Befehlszeile oder Arbeitsordner darin
pub fn from_root(p: &ProcessInfo, root: &str) -> bool {
    let base = root.trim_end_matches(['/', '\\']);
    let inside = |path: &str| {
        path == base
            || path
                .strip_prefix(base)
                .is_some_and(|rest| rest.starts_with(['/', '\\']))
    };
    if p.cwd.as_deref().is_some_and(inside) {
        return true;
    }
    let Some(at) = p.command.find(base) else {
        return false;
    };
    matches!(
        p.command[at + base.len()..].chars().next(),
        None | Some('/' | '\\' | ' ' | '"' | '\'')
    )
}

/// Die Prozesse dieses Rechners: `ps` unter Unix, CIM unter Windows; Arbeitsordner über /proc
/// bzw. lsof
struct LocalProcesses;

impl ProcessSystem for LocalProcesses {
    fn list(&self) -> Vec<ProcessInfo> {
        #[cfg(windows)]
        {
            windows_list()
        }
        #[cfg(not(windows))]
        {
            let Ok(output) = std::process::Command::new("ps")
                .args(["-A", "-o", "pid=,ppid=,command="])
                .output()
            else {
                return Vec::new();
            };
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(parse_ps_line)
                .collect()
        }
    }

    fn cwds(&self, pids: &[u32]) -> HashMap<u32, String> {
        let mut found = HashMap::new();
        if pids.is_empty() {
            return found;
        }
        if cfg!(target_os = "linux") {
            for pid in pids {
                if let Ok(cwd) = std::fs::read_link(format!("/proc/{pid}/cwd")) {
                    found.insert(*pid, cwd.to_string_lossy().into_owned());
                }
            }
            return found;
        }
        let list = pids
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        // lsof endet mit Code 1, sobald ein Prozess fehlt; die Ausgabe gilt trotzdem
        let Ok(output) = std::process::Command::new("lsof")
            .args(["-a", "-d", "cwd", "-Fn", "-p", &list])
            .output()
        else {
            return found;
        };
        let mut pid = 0;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(number) = line.strip_prefix('p') {
                pid = number.parse().unwrap_or(0);
            } else if let Some(path) = line.strip_prefix('n') {
                if pid != 0 {
                    found.insert(pid, path.to_string());
                }
            }
        }
        found
    }

    fn signal(&self, pid: u32, signal: StopSignal) {
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            if let Ok(number) = i32::try_from(pid) {
                let signal = match signal {
                    StopSignal::Term => Signal::SIGTERM,
                    StopSignal::Kill => Signal::SIGKILL,
                };
                let _ = kill(Pid::from_raw(number), signal);
            }
        }
        #[cfg(windows)]
        {
            let _ = signal;
            super::terminate(pid);
        }
    }

    fn alive(&self, pid: u32) -> bool {
        super::is_alive(pid)
    }

    fn sleep(&self, pause: Duration) {
        std::thread::sleep(pause);
    }
}

/// Eine Zeile aus `ps -o pid=,ppid=,command=`
#[cfg_attr(windows, allow(dead_code))]
fn parse_ps_line(line: &str) -> Option<ProcessInfo> {
    let mut parts = line.trim_start().splitn(2, char::is_whitespace);
    let pid = parts.next()?.parse().ok()?;
    let rest = parts.next()?.trim_start();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let ppid = parts.next()?.parse().ok()?;
    let command = parts.next().unwrap_or("").trim().to_string();
    Some(ProcessInfo {
        pid,
        ppid,
        command,
        cwd: None,
    })
}

/// Prozessliste unter Windows über PowerShell und CIM
#[cfg(windows)]
fn windows_list() -> Vec<ProcessInfo> {
    let script = "Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,CommandLine | ConvertTo-Json -Compress";
    let mut command = std::process::Command::new("powershell.exe");
    command.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    super::hide_console(&mut command);
    let Ok(output) = command.output() else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return Vec::new();
    };
    let rows = match value {
        serde_json::Value::Array(rows) => rows,
        row => vec![row],
    };
    rows.iter()
        .filter_map(|row| {
            Some(ProcessInfo {
                pid: u32::try_from(row.get("ProcessId")?.as_u64()?).ok()?,
                ppid: u32::try_from(row.get("ParentProcessId")?.as_u64()?).ok()?,
                command: row
                    .get("CommandLine")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string(),
                cwd: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    const ROOT: &str = "/repo-worktrees/ARC-0001";

    fn p(pid: u32, ppid: u32, command: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            ppid,
            command: command.to_string(),
            cwd: None,
        }
    }

    /// Server (10), Worker (11, Kind des Servers), Shell des Nutzers (20)
    fn basis() -> Vec<ProcessInfo> {
        vec![
            p(1, 0, "/sbin/launchd"),
            p(10, 1, "node /home/.harness/app/lib/server.mjs --port 4711"),
            p(
                11,
                10,
                "node /home/.harness/app/lib/server.mjs --dispatch-worker",
            ),
            p(20, 1, "/bin/zsh -l"),
        ]
    }

    /// Prozesssystem zum Zusehen: merkt sich Signale; Prozesse sterben auf SIGTERM, außer die
    /// zähen
    struct Fake {
        processes: Vec<ProcessInfo>,
        cwds: HashMap<u32, String>,
        zaeh: Vec<u32>,
        tot: Mutex<HashSet<u32>>,
        signale: Mutex<Vec<String>>,
    }

    impl Fake {
        fn neu(processes: Vec<ProcessInfo>) -> Self {
            Self {
                processes,
                cwds: HashMap::new(),
                zaeh: Vec::new(),
                tot: Mutex::new(HashSet::new()),
                signale: Mutex::new(Vec::new()),
            }
        }
    }

    impl ProcessSystem for std::sync::Arc<Fake> {
        fn list(&self) -> Vec<ProcessInfo> {
            self.processes.clone()
        }
        fn cwds(&self, pids: &[u32]) -> HashMap<u32, String> {
            pids.iter()
                .filter_map(|pid| self.cwds.get(pid).map(|cwd| (*pid, cwd.clone())))
                .collect()
        }
        fn signal(&self, pid: u32, signal: StopSignal) {
            let name = match signal {
                StopSignal::Term => "SIGTERM",
                StopSignal::Kill => "SIGKILL",
            };
            self.signale.lock().unwrap().push(format!("{name} {pid}"));
            if signal == StopSignal::Kill || !self.zaeh.contains(&pid) {
                self.tot.lock().unwrap().insert(pid);
            }
        }
        fn alive(&self, pid: u32) -> bool {
            !self.tot.lock().unwrap().contains(&pid)
        }
        fn sleep(&self, _: Duration) {}
    }

    fn sortiert(mut pids: Vec<u32>) -> Vec<u32> {
        pids.sort_unstable();
        pids
    }

    #[test]
    fn waehlt_verwaiste_prozesse_mit_dem_worktree_in_der_befehlszeile_samt_kindern() {
        let mut processes = basis();
        processes.push(p(
            30,
            1,
            &format!("{ROOT}/target/debug/nexus serve --port 4710 --node-sidecar {ROOT}/packages/server/dist/main.js"),
        ));
        processes.push(p(31, 30, "node main.js --port 0"));
        assert_eq!(sortiert(select(&processes, ROOT, 11)), vec![30, 31]);
    }

    #[test]
    fn waehlt_verwaiste_prozesse_deren_arbeitsordner_im_worktree_liegt() {
        let mut processes = basis();
        let mut node = p(40, 1, "node packages/server/dist/main.js");
        node.cwd = Some(format!("{ROOT}/packages"));
        processes.push(node);
        assert_eq!(select(&processes, ROOT, 11), vec![40]);
    }

    #[test]
    fn laesst_prozesse_mit_lebendem_elternprozess_stehen() {
        let mut processes = basis();
        let mut cargo = p(
            50,
            20,
            &format!("cargo test --manifest-path {ROOT}/Cargo.toml"),
        );
        cargo.cwd = Some(ROOT.to_string());
        processes.push(cargo);
        assert!(select(&processes, ROOT, 11).is_empty());
    }

    #[test]
    fn verwechselt_keinen_worktree_mit_gleichem_anfang() {
        let mut processes = basis();
        let mut fremd = p(60, 1, &format!("{ROOT}0/target/debug/nexus serve"));
        fremd.cwd = Some(format!("{ROOT}0"));
        processes.push(fremd);
        assert!(select(&processes, ROOT, 11).is_empty());
    }

    #[test]
    fn beendet_nie_sich_selbst_oder_seine_vorfahren() {
        let mut worker = p(70, 1, "node server.mjs --dispatch-worker");
        worker.cwd = Some(ROOT.to_string());
        let mut selbst = p(71, 70, "nexus sweep");
        selbst.cwd = Some(ROOT.to_string());
        let processes = vec![p(1, 0, "/sbin/launchd"), worker, selbst];
        assert!(select(&processes, ROOT, 71).is_empty());
    }

    #[test]
    fn gilt_unter_windows_als_verwaist_wenn_der_elternprozess_fehlt() {
        let processes = vec![
            p(4, 0, "System"),
            p(80, 7777, r"C:\wt\ARC-0001\target\debug\nexus.exe serve"),
        ];
        assert_eq!(select(&processes, r"C:\wt\ARC-0001", 4), vec![80]);
    }

    #[test]
    fn schickt_erst_sigterm_und_nur_zaehen_danach_sigkill() {
        let mut processes = basis();
        processes.push(p(30, 1, &format!("{ROOT}/target/debug/nexus serve")));
        processes.push(p(31, 1, &format!("node {ROOT}/scripts/dev.mjs")));
        let mut fake = Fake::neu(processes);
        fake.zaeh = vec![31];
        let fake = std::sync::Arc::new(fake);

        let pids = OrphanSweeper::new(Box::new(fake.clone())).sweep(ROOT, 11);

        assert_eq!(pids, vec![30, 31]);
        assert_eq!(
            *fake.signale.lock().unwrap(),
            vec!["SIGTERM 30", "SIGTERM 31", "SIGKILL 31"]
        );
    }

    #[test]
    fn findet_waisen_ueber_den_arbeitsordner_den_das_system_liefert() {
        let mut processes = basis();
        processes.push(p(40, 1, "node main.js"));
        let mut fake = Fake::neu(processes);
        fake.cwds.insert(40, ROOT.to_string());
        let fake = std::sync::Arc::new(fake);

        assert_eq!(OrphanSweeper::new(Box::new(fake)).sweep(ROOT, 11), vec![40]);
    }

    #[test]
    fn tut_nichts_ohne_waisen_aus_dem_worktree() {
        let fake = std::sync::Arc::new(Fake::neu(basis()));
        assert!(OrphanSweeper::new(Box::new(fake.clone()))
            .sweep(ROOT, 11)
            .is_empty());
        assert!(fake.signale.lock().unwrap().is_empty());
    }

    #[test]
    fn erkennt_einen_verknuepften_worktree_an_der_datei_git() {
        let dir = tempfile::tempdir().unwrap();
        let linked = dir.path().join("linked");
        let main = dir.path().join("main");
        std::fs::create_dir(&linked).unwrap();
        std::fs::create_dir_all(main.join(".git")).unwrap();
        std::fs::write(linked.join(".git"), "gitdir: /x/.git/worktrees/linked\n").unwrap();

        assert!(linked_worktree(&linked));
        assert!(!linked_worktree(&main));
        assert!(!linked_worktree(&dir.path().join("fehlt")));
    }

    #[test]
    fn liest_eine_zeile_von_ps() {
        assert_eq!(
            parse_ps_line("  4711     1 /usr/bin/node /a b/main.js --port 0"),
            Some(p(4711, 1, "/usr/bin/node /a b/main.js --port 0"))
        );
        assert_eq!(parse_ps_line("kaputt"), None);
    }
}
