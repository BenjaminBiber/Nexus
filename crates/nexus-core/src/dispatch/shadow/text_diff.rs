//! Unified-Diff zweier Texte – für die Arbeitsanweisung, deren Abweichung man nur als Diff
//! lesen kann.
//!
//! Die Arbeitsanweisungen sind einige hundert Zeilen lang; die längste gemeinsame Teilfolge
//! über eine Tabelle reicht dafür. Ab [`MAX_LINES`] Zeilen wird nicht mehr zeilenweise
//! verglichen, sondern der ganze Text ersetzt – sonst wüchse die Tabelle ins Unermessliche.

/// Zeilen, bis zu denen zeilenweise verglichen wird
pub const MAX_LINES: usize = 4_000;

/// Zeilen Zusammenhang um jede Änderung
const CONTEXT: usize = 3;

/// Unified-Diff von `node` nach `rust`; bei gleichen Texten ein leerer Text
pub fn unified(node: &str, rust: &str) -> String {
    if node == rust {
        return String::new();
    }
    let left: Vec<&str> = node.lines().collect();
    let right: Vec<&str> = rust.lines().collect();
    if left.len() > MAX_LINES || right.len() > MAX_LINES {
        return format!(
            "@@ -1,{} +1,{} @@\n{}{}",
            left.len(),
            right.len(),
            left.iter()
                .map(|line| format!("-{line}\n"))
                .collect::<String>(),
            right
                .iter()
                .map(|line| format!("+{line}\n"))
                .collect::<String>(),
        );
    }
    render(&left, &right, &edits(&left, &right))
}

/// Eine Stelle des Vergleichs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edit {
    Keep,
    Remove,
    Add,
}

/// Die Folge der Stellen über die längste gemeinsame Teilfolge
fn edits(left: &[&str], right: &[&str]) -> Vec<Edit> {
    let (rows, cols) = (left.len() + 1, right.len() + 1);
    let mut table = vec![0usize; rows * cols];
    for i in (0..left.len()).rev() {
        for j in (0..right.len()).rev() {
            table[i * cols + j] = if left[i] == right[j] {
                table[(i + 1) * cols + j + 1] + 1
            } else {
                table[(i + 1) * cols + j].max(table[i * cols + j + 1])
            };
        }
    }
    let mut steps = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < left.len() && j < right.len() {
        if left[i] == right[j] {
            steps.push(Edit::Keep);
            i += 1;
            j += 1;
        } else if table[(i + 1) * cols + j] >= table[i * cols + j + 1] {
            steps.push(Edit::Remove);
            i += 1;
        } else {
            steps.push(Edit::Add);
            j += 1;
        }
    }
    steps.extend(std::iter::repeat_n(Edit::Remove, left.len() - i));
    steps.extend(std::iter::repeat_n(Edit::Add, right.len() - j));
    steps
}

/// Die Stellen als Unified-Diff mit Abschnitten um jede Änderung
fn render(left: &[&str], right: &[&str], steps: &[Edit]) -> String {
    // Zu jeder Stelle die Zeilennummern, auf die sie zeigt
    let mut out = String::new();
    let mut index = 0;
    while index < steps.len() {
        let Some(start) = (index..steps.len()).find(|at| steps[*at] != Edit::Keep) else {
            break;
        };
        let from = start.saturating_sub(CONTEXT);
        // Bis zum Ende des Abschnitts: eine Änderung, die höchstens 2*CONTEXT gleiche Zeilen
        // entfernt liegt, gehört noch dazu
        let mut end = start;
        let mut at = start;
        while at < steps.len() {
            if steps[at] != Edit::Keep {
                end = at;
                at += 1;
                continue;
            }
            if (at + 1..steps.len())
                .take(2 * CONTEXT)
                .any(|next| steps[next] != Edit::Keep)
            {
                at += 1;
                continue;
            }
            break;
        }
        let to = (end + CONTEXT + 1).min(steps.len());
        out.push_str(&hunk(left, right, steps, from, to));
        index = to;
    }
    out
}

/// Ein Abschnitt des Diffs von Stelle `from` bis `to`
fn hunk(left: &[&str], right: &[&str], steps: &[Edit], from: usize, to: usize) -> String {
    let (mut i, mut j) = (0usize, 0usize);
    for step in &steps[..from] {
        match step {
            Edit::Keep => {
                i += 1;
                j += 1;
            }
            Edit::Remove => i += 1,
            Edit::Add => j += 1,
        }
    }
    let (start_left, start_right) = (i, j);
    let mut body = String::new();
    for step in &steps[from..to] {
        match step {
            Edit::Keep => {
                body.push_str(&format!(" {}\n", left[i]));
                i += 1;
                j += 1;
            }
            Edit::Remove => {
                body.push_str(&format!("-{}\n", left[i]));
                i += 1;
            }
            Edit::Add => {
                body.push_str(&format!("+{}\n", right[j]));
                j += 1;
            }
        }
    }
    format!(
        "@@ -{},{} +{},{} @@\n{body}",
        start_left + 1,
        i - start_left,
        start_right + 1,
        j - start_right,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gleiche_texte_ergeben_keinen_diff() {
        assert_eq!(unified("a\nb\n", "a\nb\n"), "");
    }

    #[test]
    fn eine_geaenderte_zeile_steht_mit_zusammenhang_da() {
        let node = "eins\nzwei\ndrei\nvier\nfünf\nsechs\nsieben\nacht\n";
        let rust = "eins\nzwei\ndrei\nVIER\nfünf\nsechs\nsieben\nacht\n";
        assert_eq!(
            unified(node, rust),
            "@@ -1,7 +1,7 @@\n eins\n zwei\n drei\n-vier\n+VIER\n fünf\n sechs\n sieben\n"
        );
    }

    #[test]
    fn eine_neue_zeile_am_ende_steht_als_zugang_da() {
        assert_eq!(
            unified("eins\nzwei\n", "eins\nzwei\ndrei\n"),
            "@@ -1,2 +1,3 @@\n eins\n zwei\n+drei\n"
        );
    }

    #[test]
    fn weit_auseinander_liegende_aenderungen_werden_zwei_abschnitte() {
        let node: String = (1..=20).map(|n| format!("z{n}\n")).collect();
        let rust: String = (1..=20)
            .map(|n| match n {
                2 => "geändert\n".to_string(),
                19 => "auch geändert\n".to_string(),
                _ => format!("z{n}\n"),
            })
            .collect();
        let diff = unified(&node, &rust);
        assert_eq!(diff.matches("@@").count(), 4, "zwei Abschnitte: {diff}");
        assert!(diff.contains("-z2\n+geändert\n"), "{diff}");
        assert!(diff.contains("-z19\n+auch geändert\n"), "{diff}");
    }

    #[test]
    fn sehr_lange_texte_werden_ganz_ersetzt() {
        let node: String = (0..MAX_LINES + 1).map(|n| format!("{n}\n")).collect();
        let diff = unified(&node, "kurz\n");
        assert!(
            diff.starts_with(&format!("@@ -1,{} +1,1 @@\n", MAX_LINES + 1)),
            "{}",
            &diff[..40]
        );
        assert!(diff.ends_with("+kurz\n"));
    }
}
