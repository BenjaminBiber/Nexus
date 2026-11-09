//! Prüft CSS aus Themes und Schnipseln: kein `@import` und keine Adresse mit Schema
//! (`http:`, `https:`, `//`, `file:` …) in `url(...)` oder `image-set(...)`. So lädt ein
//! Stylesheet nichts am Server-Proxy vorbei.
//!
//! Vor der Prüfung werden Kommentare entfernt, Escapes (`\69`, `\i`) aufgelöst und alles
//! kleingeschrieben, damit sich nichts hinter anderer Schreibweise verstecken kann.

use std::fmt;

/// Ein Verstoß gegen die CSS-Regeln
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssViolation {
    /// `@import` in beliebiger Schreibweise
    Import,
    /// Adresse mit Schema oder `//` (lädt von außen)
    ExternalUrl(String),
    /// Relativer Pfad, wo nur `data:` erlaubt ist (Schnipsel haben keine Assets)
    RelativeUrl(String),
}

impl fmt::Display for CssViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CssViolation::Import => f.write_str("@import ist nicht erlaubt."),
            CssViolation::ExternalUrl(url) => {
                write!(f, "Externe Adressen sind nicht erlaubt: {url}")
            }
            CssViolation::RelativeUrl(url) => {
                write!(f, "Nur data:-Adressen sind erlaubt: {url}")
            }
        }
    }
}

/// Regeln für `theme.css`: `data:` und relative Pfade (Assets des Pakets) sind erlaubt
pub fn check_css(text: &str) -> Result<(), CssViolation> {
    check(text, true)
}

/// Strengere Regeln für Schnipsel: nur `data:`-Adressen
pub fn check_snippet_css(text: &str) -> Result<(), CssViolation> {
    check(text, false)
}

fn check(text: &str, allow_relative: bool) -> Result<(), CssViolation> {
    let css = normalize(text);
    if css.contains("@import") {
        return Err(CssViolation::Import);
    }
    for target in targets(&css) {
        let clean: String = target
            .chars()
            .filter(|c| !c.is_whitespace() && !c.is_control())
            .map(|c| if c == '\\' { '/' } else { c })
            .collect();
        if clean.starts_with("data:") {
            continue;
        }
        if clean.starts_with("//") || has_scheme(&clean) {
            return Err(CssViolation::ExternalUrl(target));
        }
        if !allow_relative {
            return Err(CssViolation::RelativeUrl(target));
        }
    }
    Ok(())
}

/// Entfernt Kommentare, löst Escapes auf und schreibt klein
fn normalize(text: &str) -> String {
    let without_comments = strip_comments(text);
    unescape(&without_comments).to_lowercase()
}

/// Entfernt `/* … */`; ein offener Kommentar reicht bis zum Ende
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Löst CSS-Escapes auf: `\` mit 1–6 Hex-Ziffern (plus ein Leerzeichen) oder ein Zeichen
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let mut hex = String::new();
        while hex.len() < 6 {
            match chars.peek() {
                Some(d) if d.is_ascii_hexdigit() => {
                    hex.push(*d);
                    chars.next();
                }
                _ => break,
            }
        }
        if hex.is_empty() {
            match chars.next() {
                // Zeilenfortsetzung in Zeichenketten
                Some('\n') | None => {}
                Some(other) => out.push(other),
            }
            continue;
        }
        if matches!(chars.peek(), Some(w) if w.is_whitespace()) {
            chars.next();
        }
        let code = u32::from_str_radix(&hex, 16).unwrap_or(0xFFFD);
        out.push(char::from_u32(code).filter(|c| *c != '\0').unwrap_or('\u{FFFD}'));
    }
    out
}

/// Die Ziele aller `url(...)` und der Zeichenketten in `image-set(...)`
fn targets(css: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (start, _) in css.match_indices("url(") {
        let inner = &css[start + 4..];
        let end = inner.find(')').unwrap_or(inner.len());
        found.push(unquote(inner[..end].trim()).to_string());
    }
    for (start, _) in css.match_indices("image-set(") {
        let inner = &css[start + "image-set(".len()..];
        found.extend(strings(balanced(inner)));
    }
    found
}

/// Der Inhalt bis zur passenden schließenden Klammer
fn balanced(text: &str) -> &str {
    let mut depth = 1;
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return &text[..i];
                }
            }
            _ => {}
        }
    }
    text
}

/// Alle Zeichenketten in `"…"` oder `'…'`
fn strings(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '"' || c == '\'' {
            found.push(chars.by_ref().take_while(|d| *d != c).collect());
        }
    }
    found
}

fn unquote(text: &str) -> &str {
    let text = text.trim();
    for quote in ['"', '\''] {
        if let Some(rest) = text.strip_prefix(quote) {
            return rest.strip_suffix(quote).unwrap_or(rest);
        }
    }
    text
}

/// Beginnt mit einem Schema wie `http:` oder `file:`
fn has_scheme(url: &str) -> bool {
    let Some(colon) = url.find(':') else {
        return false;
    };
    let scheme = &url[..colon];
    let mut chars = scheme.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erlaubt_normales_css() {
        assert_eq!(check_snippet_css("body { color: red; }"), Ok(()));
        assert_eq!(check_css(".a { background: url(assets/bg.png) }"), Ok(()));
    }

    #[test]
    fn erlaubt_data_urls() {
        let css = r#".a { background: url("data:image/png;base64,AAAA") }"#;
        assert_eq!(check_snippet_css(css), Ok(()));
        assert_eq!(check_css(css), Ok(()));
    }

    #[test]
    fn lehnt_import_in_jeder_schreibweise_ab() {
        for css in [
            "@import 'a.css';",
            "@IMPORT url(a.css);",
            "@ImPoRt 'x';",
            r"@\69mport 'x';",
            r"@\49 mport 'x';",
            r"@i\mport 'x';",
            "@im/* versteckt */port 'x';",
        ] {
            assert_eq!(check_css(css), Err(CssViolation::Import), "{css}");
        }
    }

    #[test]
    fn import_im_kommentar_ist_harmlos() {
        assert_eq!(check_snippet_css("/* @import 'x'; */ a { color: red }"), Ok(()));
    }

    #[test]
    fn lehnt_externe_urls_ab() {
        for css in [
            "a { background: url(http://x.de/a.png) }",
            "a { background: url('https://x.de/a.png') }",
            "a { background: URL(\"//x.de/a.png\") }",
            "a { background: url(file:///c:/a.png) }",
            // Escapter doppelter Backslash: der Browser liest `\\` als `//`
            r"a { background: url(\\\\x.de/a.png) }",
            r"a { background: u\72l(https://x.de) }",
            "a { background: url( h\tttps://x.de ) }",
            "a { background: image-set(\"https://x.de/a.png\" 1x) }",
            "a { background: -webkit-image-set('//x.de/a.png' 1x) }",
        ] {
            assert!(
                matches!(check_css(css), Err(CssViolation::ExternalUrl(_))),
                "{css}"
            );
        }
    }

    #[test]
    fn url_im_kommentar_ist_harmlos() {
        assert_eq!(
            check_snippet_css("/* url(https://x.de) */ a { color: red }"),
            Ok(())
        );
    }

    #[test]
    fn schnipsel_erlauben_keine_relativen_pfade() {
        assert!(matches!(
            check_snippet_css("a { background: url(bg.png) }"),
            Err(CssViolation::RelativeUrl(_))
        ));
    }

    #[test]
    fn verstoss_hat_eine_meldung() {
        assert_eq!(CssViolation::Import.to_string(), "@import ist nicht erlaubt.");
    }
}
