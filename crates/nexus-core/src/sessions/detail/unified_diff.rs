//! Liest die Ausgabe von `git diff` einer Datei in Abschnitte und Zeilen.

use super::types::{DiffHunk, DiffLine, DiffLineKind, FileDiff};
use super::MAX_DIFF_LINES;

/// Abschnitte des Diffs; nach [`MAX_DIFF_LINES`] Zeilen wird gekürzt
pub fn parse(path: &str, patch: &str) -> FileDiff {
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut count = 0usize;
    let mut truncated = false;
    for raw in patch.lines() {
        if raw.starts_with("@@") {
            if count >= MAX_DIFF_LINES {
                truncated = true;
                break;
            }
            hunks.push(DiffHunk {
                header: raw.to_string(),
                lines: Vec::new(),
            });
            continue;
        }
        let Some(hunk) = hunks.last_mut() else {
            // Kopf des Diffs (diff --git, index, ---, +++)
            continue;
        };
        let kind = match raw.chars().next() {
            Some('+') => DiffLineKind::Add,
            Some('-') => DiffLineKind::Remove,
            Some(' ') | None => DiffLineKind::Context,
            // „\ No newline at end of file“ und Unbekanntes
            _ => continue,
        };
        if count >= MAX_DIFF_LINES {
            truncated = true;
            break;
        }
        hunk.lines.push(DiffLine {
            kind,
            text: raw.get(1..).unwrap_or("").to_string(),
        });
        count += 1;
    }
    FileDiff {
        path: path.to_string(),
        hunks,
        truncated,
    }
}

/// Diff einer neuen Datei: alle Zeilen hinzugefügt
pub fn all_added(path: &str, content: &str, header_note: Option<&str>) -> FileDiff {
    let mut lines: Vec<DiffLine> = content
        .lines()
        .map(|text| DiffLine {
            kind: DiffLineKind::Add,
            text: text.to_string(),
        })
        .collect();
    let count = lines.len();
    let truncated = lines.len() > MAX_DIFF_LINES;
    lines.truncate(MAX_DIFF_LINES);
    let mut header = super::line_diff::header(1, 0, 1, count);
    if let Some(note) = header_note {
        header.push(' ');
        header.push_str(note);
    }
    FileDiff {
        path: path.to_string(),
        hunks: if lines.is_empty() {
            Vec::new()
        } else {
            vec![DiffHunk { header, lines }]
        },
        truncated,
    }
}

/// Kürzt die Abschnitte auf [`MAX_DIFF_LINES`] Zeilen
pub fn limit(path: &str, hunks: Vec<DiffHunk>) -> FileDiff {
    let mut left = MAX_DIFF_LINES;
    let mut kept = Vec::new();
    let mut truncated = false;
    for mut hunk in hunks {
        if left == 0 {
            truncated = true;
            break;
        }
        if hunk.lines.len() > left {
            hunk.lines.truncate(left);
            truncated = true;
        }
        left -= hunk.lines.len();
        kept.push(hunk);
        if truncated {
            break;
        }
    }
    FileDiff {
        path: path.to_string(),
        hunks: kept,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "diff --git a/src/a.txt b/src/a.txt\n\
index 1111111..2222222 100644\n\
--- a/src/a.txt\n\
+++ b/src/a.txt\n\
@@ -1,3 +1,3 @@ fn kopf\n \
eins\n\
-zwei\n\
+ZWEI\n \
drei\n\
@@ -10,2 +10,3 @@\n \
zehn\n\
+neu\n \
elf\n\
\\ No newline at end of file\n";

    #[test]
    fn liest_abschnitte_und_zeilen_ohne_praefix() {
        let diff = parse("src/a.txt", PATCH);
        assert_eq!(diff.path, "src/a.txt");
        assert!(!diff.truncated);
        assert_eq!(diff.hunks.len(), 2);
        assert_eq!(diff.hunks[0].header, "@@ -1,3 +1,3 @@ fn kopf");
        assert_eq!(diff.hunks[0].lines.len(), 4);
        assert_eq!(diff.hunks[0].lines[1].kind, DiffLineKind::Remove);
        assert_eq!(diff.hunks[0].lines[1].text, "zwei");
        assert_eq!(diff.hunks[0].lines[2].kind, DiffLineKind::Add);
        assert_eq!(diff.hunks[1].lines.len(), 3);
    }

    #[test]
    fn binaerdateien_haben_keine_abschnitte() {
        let diff = parse(
            "b.bin",
            "diff --git a/b.bin b/b.bin\nBinary files a/b.bin and b/b.bin differ\n",
        );
        assert!(diff.hunks.is_empty());
    }

    #[test]
    fn kuerzt_sehr_lange_diffs() {
        let mut patch = String::from("@@ -0,0 +1,6000 @@\n");
        for n in 0..6000 {
            patch.push_str(&format!("+{n}\n"));
        }
        let diff = parse("x", &patch);
        assert!(diff.truncated);
        assert_eq!(diff.hunks[0].lines.len(), MAX_DIFF_LINES);
    }

    #[test]
    fn neue_datei_ist_ganz_hinzugefuegt() {
        let diff = all_added("n.txt", "a\nb\n", None);
        assert_eq!(diff.hunks[0].header, "@@ -0,0 +1,2 @@");
        assert!(diff.hunks[0]
            .lines
            .iter()
            .all(|line| line.kind == DiffLineKind::Add));
    }
}
