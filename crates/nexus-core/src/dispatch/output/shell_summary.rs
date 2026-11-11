//! Macht aus einem Shell-Befehl des Agenten eine lesbare Zeile; portiert aus
//! `packages/core/src/dispatch/ShellCommandSummary.ts`.
//!
//! Bewusst grob – im Zweifel bleibt der gekürzte Befehl. Die Art landet als Kennung in
//! eckigen Klammern im Lauf-Protokoll und ist damit Vertrag.

use std::sync::LazyLock;

use regex::Regex;

use super::text::one_line;

/// Befehle, die eine Datei nur lesen
const READERS: [&str; 7] = ["cat", "head", "tail", "less", "nl", "wc", "type"];

static TEST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(vitest|jest|playwright test|npm (?:run )?test|npm t\b|dotnet test|pytest)\b")
        .expect("festes Muster")
});
static INSTALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:npm (?:ci|install|i)|pnpm install|yarn(?: install)?)(?:\s|$)")
        .expect("festes Muster")
});
static SCRIPT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:node|python3?|py|tsx|npx tsx|deno run)\b").expect("festes Muster")
});
static WRITES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"writeFileSync|writeFile\(|appendFileSync|open\([^)]*['"][wa]['"]|\.write_text\(|\.write\("#)
        .expect("festes Muster")
});
static FILE_LITERAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"['"`]([^'"`\s]+\.(?:tsx?|jsx?|mjs|cjs|css|scss|json|md|html|py|cs|ya?ml|sql|txt))['"`]"#,
    )
    .expect("festes Muster")
});

/// `cat <<EOF > datei` bzw. `cat > datei`
static CAT_HEREDOC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^cat\s+(?:<<-?\s*['"]?\w+['"]?\s+)?>\s*(\S+)"#).expect("festes Muster")
});
static CAT_WRITE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^cat\s+>\s*(\S+)").expect("festes Muster"));
static TEE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\btee\s+(?:-a\s+)?(\S+)").expect("festes Muster"));

static SED_INPLACE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^sed\s+-i").expect("festes Muster"));
static ECHO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^echo\b|^printf\b").expect("festes Muster"));
static SEGMENTS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*(?:&&|\|\||;)\s*").expect("festes Muster"));
static PIPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\|\s*").expect("festes Muster"));
static GREP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:grep|rg|findstr)\b(.*)$").expect("festes Muster"));
static FILE_TOOLS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:ls|dir|file|stat|test|find)\b").expect("festes Muster"));
static GIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^git\s+(\S+)(.*)$").expect("festes Muster"));
/// `git commit -m "…"` bzw. `-m '…'`; in JavaScript ein Rückverweis auf das Anführungszeichen
static COMMIT_MESSAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"-m\s+(?:"([^"\n]*)"|'([^'\n]*)')"#).expect("festes Muster"));
static HARNESS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:nexus|harness)\s+board\s+(\S+)\s*(\S+)?(.*)$").expect("festes Muster")
});
static LEADING_CD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^cd\s+\S+\s*(?:&&|;)\s*").expect("festes Muster"));
static TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"'[^']*'|"[^"]*"|\S+"#).expect("festes Muster"));
/// Testdateien im Befehl einer Testausführung
static TEST_FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.(?:test|spec)\.[cm]?[tj]sx?$").expect("festes Muster"));
static TEST_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|/)tests?/").expect("festes Muster"));
/// Erste Zahl im Rest von `nexus board step …`
static STEP_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\d+)").expect("festes Muster"));

/// Art eines Shell-Befehls im Lauf-Protokoll
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Read,
    Edit,
    Search,
    Test,
    Run,
}

/// Kurzfassung eines Shell-Befehls
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSummary {
    pub kind: ShellKind,
    pub text: String,
}

/// Macht aus einem Shell-Befehl des Agenten eine lesbare Zeile: was er tut (lesen, ändern,
/// suchen, testen, ausführen) und woran. `strip` kürzt Pfade (z. B. auf den Worktree).
pub struct ShellCommandSummary {
    strip: Box<dyn Fn(&str) -> String + Send + Sync>,
}

impl std::fmt::Debug for ShellCommandSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ShellCommandSummary")
    }
}

impl Default for ShellCommandSummary {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellCommandSummary {
    /// Kurzfassung ohne Kürzen der Pfade
    pub fn new() -> Self {
        Self {
            strip: Box::new(|text| text.to_string()),
        }
    }

    /// Kurzfassung, die Pfade mit `strip` kürzt
    pub fn with_strip(strip: Box<dyn Fn(&str) -> String + Send + Sync>) -> Self {
        Self { strip }
    }

    /// Kurzfassung; eine Beschreibung des Agenten (Feld `description` des Bash-Tools) ersetzt
    /// den Text, nicht die Art
    pub fn summarize(&self, command: &str, description: Option<&str>) -> ShellSummary {
        let own = self.guess(command);
        let text = match description {
            Some(text) if !text.trim().is_empty() => one_line(text, 160),
            _ => own.text,
        };
        ShellSummary {
            kind: own.kind,
            text,
        }
    }

    fn guess(&self, command: &str) -> ShellSummary {
        let full = (self.strip)(command);
        let first = without_cd(full.split('\n').next().unwrap_or(""));
        if TEST.is_match(&first) {
            return ShellSummary {
                kind: ShellKind::Test,
                text: test_text(&first),
            };
        }
        if INSTALL.is_match(&first) {
            return ShellSummary {
                kind: ShellKind::Run,
                text: "installiert Abhängigkeiten".to_string(),
            };
        }
        if SCRIPT.is_match(&first)
            && (first.contains("<<") || contains_flag(&first, "-e") || contains_flag(&first, "-c"))
        {
            let mut files: Vec<String> = Vec::new();
            for found in FILE_LITERAL.captures_iter(&full) {
                let name = found[1].to_string();
                if !files.contains(&name) {
                    files.push(name);
                }
            }
            let list = if files.is_empty() {
                String::new()
            } else {
                format!(": {}", names(&files))
            };
            return if WRITES.is_match(&full) {
                ShellSummary {
                    kind: ShellKind::Edit,
                    text: format!("ändert per Skript{list}"),
                }
            } else {
                ShellSummary {
                    kind: ShellKind::Run,
                    text: format!("führt Skript aus{list}"),
                }
            };
        }
        if let Some(target) = CAT_HEREDOC
            .captures(&first)
            .or_else(|| CAT_WRITE.captures(&first))
            .or_else(|| TEE.captures(&first))
        {
            return ShellSummary {
                kind: ShellKind::Edit,
                text: format!("schreibt {}", names(&[target[1].to_string()])),
            };
        }
        let segments: Vec<String> = SEGMENTS
            .split(&first)
            .map(without_cd)
            .filter(|segment| !segment.is_empty() && !ECHO.is_match(segment))
            .collect();
        if !segments.is_empty() && segments.iter().all(|s| SED_INPLACE.is_match(s)) {
            // `sed -i "s/a/b/" datei`: die Datei steht am Ende
            let files = pick_files(&segments, |tokens| {
                tokens.last().map(|t| vec![t.clone()]).unwrap_or_default()
            });
            return ShellSummary {
                kind: ShellKind::Edit,
                text: format!("ändert {}", names(&files)),
            };
        }
        let piped: Vec<String> = segments
            .iter()
            .map(|s| PIPE.split(s).next().unwrap_or("").to_string())
            .collect();
        if !piped.is_empty() && piped.iter().all(|s| is_read(s)) {
            // Bei sed ist das erste Wort ohne Bindestrich das Skript, danach kommen die Dateien
            let files = pick_files(&piped, |tokens| {
                let skip = usize::from(tokens.first().is_some_and(|first| first == "sed"));
                tokens
                    .iter()
                    .skip(1)
                    .filter(|token| !token.starts_with('-'))
                    .skip(skip)
                    .cloned()
                    .collect()
            });
            if !files.is_empty() {
                return ShellSummary {
                    kind: ShellKind::Read,
                    text: format!("liest {}", names(&files)),
                };
            }
        }
        let lead = piped.first().cloned().unwrap_or_else(|| first.clone());
        if let Some(found) = GREP.captures(&lead) {
            let args: Vec<String> = tokens(&found[1])
                .into_iter()
                .filter(|token| !token.starts_with('-'))
                .collect();
            let where_ = if args.len() > 1 {
                format!(" in {}", names(&args[1..]))
            } else {
                String::new()
            };
            let pattern = one_line(args.first().map(String::as_str).unwrap_or(""), 60);
            return ShellSummary {
                kind: ShellKind::Search,
                text: format!("sucht „{pattern}“{where_}"),
            };
        }
        if FILE_TOOLS.is_match(&lead) {
            return ShellSummary {
                kind: ShellKind::Run,
                text: "prüft Dateien".to_string(),
            };
        }
        if let Some(found) = GIT.captures(&lead) {
            let verb = &found[1];
            let message = if verb == "commit" {
                COMMIT_MESSAGE.captures(&found[2]).and_then(|m| {
                    m.get(1)
                        .or_else(|| m.get(2))
                        .map(|group| group.as_str().to_string())
                })
            } else {
                None
            };
            return ShellSummary {
                kind: ShellKind::Run,
                text: match message {
                    Some(message) => format!("git commit „{}“", one_line(&message, 80)),
                    None => format!("git {verb}"),
                },
            };
        }
        if let Some(found) = HARNESS.captures(&lead) {
            return ShellSummary {
                kind: ShellKind::Run,
                text: harness_text(
                    &found[1],
                    found.get(2).map(|group| group.as_str()),
                    &found[3],
                ),
            };
        }
        ShellSummary {
            kind: ShellKind::Run,
            text: one_line(&first, 120),
        }
    }
}

/// „Tests“ mit den genannten Testdateien, sonst „(alle)“
fn test_text(command: &str) -> String {
    let files: Vec<String> = tokens(command)
        .into_iter()
        .filter(|token| TEST_FILE.is_match(token) || TEST_DIR.is_match(token))
        .collect();
    if files.is_empty() {
        "Tests (alle)".to_string()
    } else {
        format!("Tests: {}", names(&files))
    }
}

/// Was ein `nexus board …` (auch über den Alias `harness`) tut
fn harness_text(verb: &str, id: Option<&str>, rest: &str) -> String {
    let id = id.unwrap_or("");
    match verb {
        "show" => format!("liest Ticket {id}").trim_end().to_string(),
        "comment" => format!("kommentiert {id}").trim_end().to_string(),
        "step" => {
            let step = STEP_NUMBER
                .captures(rest)
                .map_or_else(|| "?".to_string(), |found| found[1].to_string());
            format!("hakt Schritt {step} ab")
        }
        "done" => {
            if id.is_empty() {
                "meldet erledigt".to_string()
            } else {
                format!("meldet {id} als erledigt")
            }
        }
        other => one_line(&format!("nexus board {other} {id}{rest}"), 120),
    }
}

/// Liest das Segment nur (cat, head, sed -n …, ohne Umleitung in eine Datei)?
fn is_read(segment: &str) -> bool {
    if segment.contains('>') {
        return false;
    }
    let command = segment.split_whitespace().next().unwrap_or("");
    READERS.contains(&command)
        || (command == "sed" && format!(" {segment} ").contains(" -n ") && !segment.contains(" -i"))
}

/// Dateien aus den Segmenten (je Segment `pick` auf die Wörter), ohne Doppelte
fn pick_files(segments: &[String], pick: impl Fn(&[String]) -> Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for segment in segments {
        for file in pick(&tokens(segment)) {
            if !out.contains(&file) {
                out.push(file);
            }
        }
    }
    out
}

/// Höchstens drei Namen (Dateinamen ohne Ordner), Rest als „+N“
fn names(items: &[String]) -> String {
    let names: Vec<String> = items
        .iter()
        .map(|item| {
            let plain = unquote(item);
            let plain = plain.strip_suffix('*').unwrap_or(&plain);
            let last = plain.rsplit('/').next().unwrap_or("");
            if last.is_empty() {
                item.clone()
            } else {
                last.to_string()
            }
        })
        .collect();
    if names.len() > 3 {
        format!("{} +{}", names[..3].join(", "), names.len() - 3)
    } else {
        names.join(", ")
    }
}

/// Führende „cd … &&“ weg
fn without_cd(segment: &str) -> String {
    let mut text = segment.trim().to_string();
    while let Some(found) = LEADING_CD.find(&text) {
        text = text[found.end()..].to_string();
    }
    text
}

/// Wörter mit einfachen und doppelten Anführungszeichen
fn tokens(text: &str) -> Vec<String> {
    TOKEN
        .find_iter(text)
        .map(|found| unquote(found.as_str()))
        .collect()
}

/// Ein Wort ohne umschließende Anführungszeichen; in JavaScript ein Rückverweis
fn unquote(text: &str) -> String {
    let mut chars = text.chars();
    match (chars.next(), text.chars().last()) {
        (Some(first), Some(last))
            if (first == '\'' || first == '"') && first == last && text.len() >= 2 =>
        {
            text[first.len_utf8()..text.len() - last.len_utf8()].to_string()
        }
        _ => text.to_string(),
    }
}

/// Steht das Wort (`-e`, `-c`) mit Leerraum darum im Befehl? (`/\s-e\s/` in JavaScript)
fn contains_flag(text: &str, flag: &str) -> bool {
    let needle = format!("{flag} ");
    text.match_indices(&needle).any(|(at, _)| {
        at > 0
            && text[..at]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(command: &str) -> ShellSummary {
        ShellCommandSummary::new().summarize(command, None)
    }

    #[test]
    fn erkennt_lesen_schreiben_andern_per_skript_suchen_und_tests() {
        assert_eq!(
            summary("cat src/a.tsx; echo ===; sed -n '1,130p' src/b.tsx"),
            ShellSummary {
                kind: ShellKind::Read,
                text: "liest a.tsx, b.tsx".to_string()
            }
        );
        assert_eq!(
            summary("cat > src/c.tsx <<'EOF'\nexport {}\nEOF"),
            ShellSummary {
                kind: ShellKind::Edit,
                text: "schreibt c.tsx".to_string()
            }
        );
        assert_eq!(
            summary("node - <<'EOF'\nconst fs=require('fs');let p='src/d.tsx';fs.writeFileSync(p,'x');\nEOF"),
            ShellSummary {
                kind: ShellKind::Edit,
                text: "ändert per Skript: d.tsx".to_string()
            }
        );
        assert_eq!(
            summary(r#"sed -i "s/a/b/" src/e.ts"#),
            ShellSummary {
                kind: ShellKind::Edit,
                text: "ändert e.ts".to_string()
            }
        );
        assert_eq!(
            summary(r#"grep -n "db-composer" src/styles/x.css | head -40"#),
            ShellSummary {
                kind: ShellKind::Search,
                text: "sucht „db-composer“ in x.css".to_string()
            }
        );
        assert_eq!(
            summary("cd ../.. && npx vitest run packages/web/tests/A.test.tsx 2>&1 | tail -60"),
            ShellSummary {
                kind: ShellKind::Test,
                text: "Tests: A.test.tsx".to_string()
            }
        );
        assert_eq!(
            summary("npm test 2>&1 | tail -25"),
            ShellSummary {
                kind: ShellKind::Test,
                text: "Tests (alle)".to_string()
            }
        );
    }

    #[test]
    fn kennt_installation_git_und_harness_sonst_bleibt_der_gekurzte_befehl() {
        assert_eq!(
            summary("npm ci 2>&1 | tail -5"),
            ShellSummary {
                kind: ShellKind::Run,
                text: "installiert Abhängigkeiten".to_string()
            }
        );
        assert_eq!(
            summary(r#"git commit -m "feat: Dialog""#),
            ShellSummary {
                kind: ShellKind::Run,
                text: "git commit „feat: Dialog“".to_string()
            }
        );
        assert_eq!(
            summary("harness board show AIH-0043 2>&1 | head -200"),
            ShellSummary {
                kind: ShellKind::Run,
                text: "liest Ticket AIH-0043".to_string()
            }
        );
        assert_eq!(
            summary("docker ps"),
            ShellSummary {
                kind: ShellKind::Run,
                text: "docker ps".to_string()
            }
        );
    }

    #[test]
    fn nimmt_die_beschreibung_des_agenten_als_text_behalt_aber_die_art() {
        assert_eq!(
            ShellCommandSummary::new().summarize("npm test", Some("Alle Tests")),
            ShellSummary {
                kind: ShellKind::Test,
                text: "Alle Tests".to_string()
            }
        );
    }

    #[test]
    fn harness_befehle_nennen_schritt_kommentar_und_abschluss() {
        assert_eq!(
            summary("nexus board step ARC-0007 3").text,
            "hakt Schritt 3 ab"
        );
        assert_eq!(
            summary("nexus board comment ARC-0007 \"x\"").text,
            "kommentiert ARC-0007"
        );
        assert_eq!(
            summary("nexus board done ARC-0007").text,
            "meldet ARC-0007 als erledigt"
        );
        assert_eq!(
            summary("nexus board note list").text,
            "nexus board note list"
        );
        // Der alte Alias `harness` wird weiter erkannt und als `nexus` angezeigt
        assert_eq!(
            summary("harness board note list").text,
            "nexus board note list"
        );
    }

    #[test]
    fn kurzt_lange_listen_auf_drei_namen() {
        assert_eq!(
            summary("cat a/1.ts b/2.ts c/3.ts d/4.ts").text,
            "liest 1.ts, 2.ts, 3.ts +1"
        );
    }

    #[test]
    fn strip_kurzt_pfade_vor_der_auswertung() {
        let shell = ShellCommandSummary::with_strip(Box::new(|text| text.replace("/wt/A/", "")));
        assert_eq!(
            shell.summarize("cat /wt/A/src/a.ts", None).text,
            "liest a.ts"
        );
    }
}
