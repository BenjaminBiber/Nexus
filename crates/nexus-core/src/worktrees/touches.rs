//! Überschneidung der betroffenen Bereiche (`touches`) zweier Tickets.
//! Portiert aus `packages/core/src/worktrees/TouchesChecker.ts`.

/// Prüft, ob sich die betroffenen Bereiche (`touches`) zweier Tickets überschneiden.
#[derive(Debug, Clone, Copy, Default)]
pub struct TouchesChecker;

impl TouchesChecker {
    /// Neuer Prüfer (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Überschneidung: gleicher Pfad oder einer liegt innerhalb des anderen
    pub fn overlaps(&self, a: &[String], b: &[String]) -> bool {
        let left: Vec<String> = a.iter().map(|path| normalize(path)).collect();
        let right: Vec<String> = b.iter().map(|path| normalize(path)).collect();
        left.iter()
            .any(|x| right.iter().any(|y| contains(x, y) || contains(y, x)))
    }
}

/// `outer` ist gleich `inner` oder ein Oberordner davon (ganze Pfadteile, kein bloßer
/// Namensanfang)
fn contains(outer: &str, inner: &str) -> bool {
    outer == inner || inner.starts_with(&format!("{outer}/"))
}

/// Vereinheitlicht Schrägstriche, entfernt „./“ am Anfang und „/“ am Ende
fn normalize(path: &str) -> String {
    let slashed = path.replace('\\', "/");
    let without_dot = slashed.strip_prefix("./").unwrap_or(&slashed);
    without_dot.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Bereiche als Liste, wie sie am Ticket stehen
    fn paths(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn erkennt_gleiche_pfade_als_uberschneidung() {
        assert!(TouchesChecker::new().overlaps(&paths(&["src/login"]), &paths(&["src/login"])));
    }

    #[test]
    fn erkennt_einen_oberordner_als_uberschneidung() {
        let checker = TouchesChecker::new();
        assert!(checker.overlaps(&paths(&["src"]), &paths(&["src/login"])));
        assert!(checker.overlaps(&paths(&["src/a/b"]), &paths(&["src/a"])));
    }

    #[test]
    fn behandelt_ahnlich_beginnende_namen_nicht_als_uberschneidung() {
        assert!(!TouchesChecker::new().overlaps(&paths(&["src/log"]), &paths(&["src/login"])));
    }

    #[test]
    fn ignoriert_schreibweise_mit_backslash_punkt_und_schragstrich_am_ende() {
        assert!(TouchesChecker::new().overlaps(&paths(&["./src\\login/"]), &paths(&["src/login"])));
    }

    #[test]
    fn meldet_keine_uberschneidung_bei_getrennten_oder_leeren_bereichen() {
        let checker = TouchesChecker::new();
        assert!(!checker.overlaps(&paths(&["src/a"]), &paths(&["src/b"])));
        assert!(!checker.overlaps(&[], &paths(&["src/b"])));
    }
}
