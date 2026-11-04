//! Zeilen-Diff zweier Texte (gemeinsamer Anfang und Schluss, dazwischen längste gemeinsame
//! Teilfolge) und seine Zerlegung in Abschnitte mit Kontext wie bei `git diff`.

use super::types::{DiffHunk, DiffLine, DiffLineKind};

/// Größte Tabelle der Teilfolge (Zeilen alt × neu); darüber: alles weg, alles neu
const MAX_CELLS: usize = 4_000_000;
/// Kontextzeilen um jede Änderung
pub const CONTEXT: usize = 3;

/// Ein Schritt des Diffs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op<'a> {
    Same(&'a str),
    Remove(&'a str),
    Add(&'a str),
}

/// Die Zeilen eines Textes (ohne Zeilenumbruch, `\r\n` wie `\n`)
pub fn split(text: &str) -> Vec<&str> {
    text.lines().collect()
}

/// Diff der Zeilen von `old` nach `new`
pub fn diff<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<Op<'a>> {
    let prefix = old
        .iter()
        .zip(new.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let rest_old = &old[prefix..];
    let rest_new = &new[prefix..];
    let suffix = rest_old
        .iter()
        .rev()
        .zip(rest_new.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let middle_old = &rest_old[..rest_old.len() - suffix];
    let middle_new = &rest_new[..rest_new.len() - suffix];
    let mut ops: Vec<Op<'a>> = old[..prefix].iter().map(|line| Op::Same(line)).collect();
    ops.extend(middle(middle_old, middle_new));
    ops.extend(
        rest_old[rest_old.len() - suffix..]
            .iter()
            .map(|line| Op::Same(line)),
    );
    ops
}

/// Hinzugefügte und entfernte Zeilen
pub fn counts(ops: &[Op<'_>]) -> (i64, i64) {
    ops.iter().fold((0, 0), |(added, removed), op| match op {
        Op::Add(_) => (added + 1, removed),
        Op::Remove(_) => (added, removed + 1),
        Op::Same(_) => (added, removed),
    })
}

/// Zerlegt den Diff in Abschnitte mit [`CONTEXT`] Zeilen Kontext; ohne Änderung keine
pub fn hunks(ops: &[Op<'_>]) -> Vec<DiffHunk> {
    let changed: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter(|(_, op)| !matches!(op, Op::Same(_)))
        .map(|(index, _)| index)
        .collect();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for index in changed {
        let start = index.saturating_sub(CONTEXT);
        let end = (index + CONTEXT + 1).min(ops.len());
        match ranges.last_mut() {
            Some(last) if start <= last.1 => last.1 = end,
            _ => ranges.push((start, end)),
        }
    }
    ranges
        .into_iter()
        .map(|(start, end)| hunk(ops, start, end))
        .collect()
}

/// Ein Abschnitt aus `ops[start..end]` mit Kopfzeile
fn hunk(ops: &[Op<'_>], start: usize, end: usize) -> DiffHunk {
    // Zeilennummern vor dem Abschnitt
    let (mut old_line, mut new_line) = (1usize, 1usize);
    for op in &ops[..start] {
        match op {
            Op::Same(_) => {
                old_line += 1;
                new_line += 1;
            }
            Op::Remove(_) => old_line += 1,
            Op::Add(_) => new_line += 1,
        }
    }
    let lines: Vec<DiffLine> = ops[start..end].iter().map(|op| line(*op)).collect();
    let old_count = lines
        .iter()
        .filter(|line| line.kind != DiffLineKind::Add)
        .count();
    let new_count = lines
        .iter()
        .filter(|line| line.kind != DiffLineKind::Remove)
        .count();
    DiffHunk {
        header: header(old_line, old_count, new_line, new_count),
        lines,
    }
}

/// Kopfzeile `@@ -a,b +c,d @@`; ein leerer Bereich beginnt wie bei Git eine Zeile davor
pub fn header(old_start: usize, old_count: usize, new_start: usize, new_count: usize) -> String {
    let start = |line: usize, count: usize| if count == 0 { line - 1 } else { line };
    format!(
        "@@ -{},{} +{},{} @@",
        start(old_start, old_count),
        old_count,
        start(new_start, new_count),
        new_count
    )
}

/// Ein Schritt als Diff-Zeile
pub fn line(op: Op<'_>) -> DiffLine {
    let (kind, text) = match op {
        Op::Same(text) => (DiffLineKind::Context, text),
        Op::Remove(text) => (DiffLineKind::Remove, text),
        Op::Add(text) => (DiffLineKind::Add, text),
    };
    DiffLine {
        kind,
        text: text.to_string(),
    }
}

/// Der Mittelteil: längste gemeinsame Teilfolge, bei zu großen Teilen alles weg, alles neu
fn middle<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<Op<'a>> {
    if old.is_empty() || new.is_empty() || old.len().saturating_mul(new.len()) > MAX_CELLS {
        let mut ops: Vec<Op<'a>> = old.iter().map(|line| Op::Remove(line)).collect();
        ops.extend(new.iter().map(|line| Op::Add(line)));
        return ops;
    }
    let (n, m) = (old.len(), new.len());
    // table[i][j] = Länge der Teilfolge von old[i..] und new[j..]
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[at(i, j)] = if old[i] == new[j] {
                table[at(i + 1, j + 1)] + 1
            } else {
                table[at(i + 1, j)].max(table[at(i, j + 1)])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut ops = Vec::with_capacity(n + m);
    while i < n && j < m {
        if old[i] == new[j] {
            ops.push(Op::Same(old[i]));
            i += 1;
            j += 1;
        } else if table[at(i + 1, j)] >= table[at(i, j + 1)] {
            ops.push(Op::Remove(old[i]));
            i += 1;
        } else {
            ops.push(Op::Add(new[j]));
            j += 1;
        }
    }
    ops.extend(old[i..].iter().map(|line| Op::Remove(line)));
    ops.extend(new[j..].iter().map(|line| Op::Add(line)));
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(hunk: &DiffHunk) -> String {
        hunk.lines
            .iter()
            .map(|line| match line.kind {
                DiffLineKind::Add => '+',
                DiffLineKind::Remove => '-',
                DiffLineKind::Context => ' ',
            })
            .collect()
    }

    #[test]
    fn findet_eine_geaenderte_zeile_mit_kontext() {
        let old = split("a\nb\nc\nd\ne\nf\ng\nh\n");
        let new = split("a\nb\nc\nd\nE\nf\ng\nh\n");
        let ops = diff(&old, &new);
        assert_eq!(counts(&ops), (1, 1));
        let hunks = hunks(&ops);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].header, "@@ -2,7 +2,7 @@");
        assert_eq!(kinds(&hunks[0]), "   -+   ");
        assert_eq!(hunks[0].lines[3].text, "e");
        assert_eq!(hunks[0].lines[4].text, "E");
    }

    #[test]
    fn trennt_weit_auseinander_liegende_aenderungen() {
        let old: Vec<String> = (1..=20).map(|n| n.to_string()).collect();
        let mut new = old.clone();
        new[1] = "zwei".into();
        new[17] = "achtzehn".into();
        let old: Vec<&str> = old.iter().map(String::as_str).collect();
        let new: Vec<&str> = new.iter().map(String::as_str).collect();
        let hunks = hunks(&diff(&old, &new));
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].header, "@@ -1,5 +1,5 @@");
        assert_eq!(hunks[1].header, "@@ -15,6 +15,6 @@");
    }

    #[test]
    fn neue_datei_beginnt_bei_null() {
        let ops = diff(&[], &split("x\ny"));
        assert_eq!(counts(&ops), (2, 0));
        assert_eq!(hunks(&ops)[0].header, "@@ -0,0 +1,2 @@");
    }

    #[test]
    fn ohne_aenderung_keine_abschnitte() {
        let lines = split("a\nb");
        assert!(hunks(&diff(&lines, &lines)).is_empty());
    }

    #[test]
    fn findet_eingefuegte_zeilen_in_der_mitte() {
        let ops = diff(&split("a\nb\nc\nd"), &split("a\nx\nb\nc\ny\nd"));
        assert_eq!(counts(&ops), (2, 0));
    }
}
