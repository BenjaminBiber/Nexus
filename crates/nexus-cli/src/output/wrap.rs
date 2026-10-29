//! Zeilenumbruch wie in commander (`Help.wrap`): gleiche Breite, gleiche Trennstellen,
//! gleiche Einrückung der Folgezeilen. Nur so sind die Hilfetexte Zeichen für Zeichen gleich.

/// Schmaler als so viele Spalten wird nicht umbrochen (sonst stünde fast jedes Wort allein)
const MIN_COLUMN_WIDTH: usize = 40;

/// Bricht `text` auf `width` Spalten um; die ersten `indent` Zeichen sind der Kopf (Begriff samt
/// Auffüllung), Folgezeilen werden um `indent` eingerückt. Von Hand umbrochene und eingerückte
/// Texte bleiben unverändert.
pub fn wrap(text: &str, width: usize, indent: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if manually_indented(&chars) {
        return text.to_string();
    }
    let column = width.saturating_sub(indent);
    if column < MIN_COLUMN_WIDTH {
        return text.to_string();
    }
    let head: String = chars.iter().take(indent).collect();
    let body: Vec<char> = chars.iter().skip(indent).copied().collect();
    let padding = " ".repeat(indent);
    let lines: Vec<String> = chunks(&body, column - 1)
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| match chunk.as_str() {
            "\n" => String::new(),
            line if index > 0 => format!("{padding}{}", line.trim_end()),
            line => line.trim_end().to_string(),
        })
        .collect();
    format!("{head}{}", lines.join("\n"))
}

/// Enthält der Text einen Zeilenumbruch, dem Leerraum folgt? Dann ist er schon von Hand gesetzt.
fn manually_indented(chars: &[char]) -> bool {
    chars
        .windows(2)
        .any(|pair| pair[0] == '\n' && is_indent_char(pair[1]))
}

/// Leerraum, der eine Einrückung sein kann (alles aus `\s` außer dem Zeilenumbruch)
fn is_indent_char(c: char) -> bool {
    matches!(
        c,
        ' ' | '\t'
            | '\u{0B}'
            | '\u{0C}'
            | '\u{A0}'
            | '\u{1680}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    ) || ('\u{2000}'..='\u{200A}').contains(&c)
}

/// Trennstelle: Leerraum oder das Zeichen für einen Umbruch ohne Breite
fn is_break(c: char) -> bool {
    c.is_whitespace() || c == '\u{200B}'
}

/// Zerlegt den Text in Abschnitte von höchstens `max` Zeichen, die an einer Trennstelle enden;
/// ein einzelner Zeilenumbruch wird selbst zu einem Abschnitt (er bleibt als Leerzeile erhalten).
fn chunks(chars: &[char], max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        if chars[start] == '\n' {
            out.push("\n".to_string());
            start += 1;
            continue;
        }
        let limit = stop_at_newline(chars, start, max);
        let end = longest_fitting(chars, start, limit).unwrap_or_else(|| next_break(chars, start));
        out.push(chars[start..end].iter().collect());
        start = if end < chars.len() { end + 1 } else { end };
    }
    out
}

/// Ende des längsten Abschnitts, der noch in `max` Zeichen passt und keinen Umbruch enthält
fn stop_at_newline(chars: &[char], start: usize, max: usize) -> usize {
    let limit = (start + max).min(chars.len());
    (start..limit)
        .find(|index| chars[*index] == '\n')
        .unwrap_or(limit)
}

/// Größtes Ende bis `limit`, das an einer Trennstelle oder am Textende liegt
fn longest_fitting(chars: &[char], start: usize, limit: usize) -> Option<usize> {
    (start + 1..=limit)
        .rev()
        .find(|end| *end == chars.len() || is_break(chars[*end]))
}

/// Ende eines Wortes, das länger ist als die Spalte: bis zur nächsten Trennstelle
fn next_break(chars: &[char], start: usize) -> usize {
    (start + 1..chars.len())
        .find(|index| is_break(chars[*index]))
        .unwrap_or(chars.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bricht_an_der_letzten_trennstelle_vor_der_breite_um() {
        let text = "--step <text>              Arbeitsschritt (wiederholbar, in dieser Reihenfolge; Kommas bleiben erhalten)";
        assert_eq!(
            wrap(text, 78, 27),
            "--step <text>              Arbeitsschritt (wiederholbar, in dieser\n                           Reihenfolge; Kommas bleiben erhalten)"
        );
    }

    #[test]
    fn lasst_von_hand_eingeruckte_texte_unverandert() {
        let text = "Beispiele:\n  nexus board\n  nexus board --port 4800";
        assert_eq!(wrap(text, 80, 0), text);
    }

    #[test]
    fn bricht_nicht_um_wenn_die_spalte_zu_schmal_ware() {
        let text = "--sehr-lange-option <wert>  eine Beschreibung, die eigentlich zu lang ist";
        assert_eq!(wrap(text, 78, 40), text);
    }

    #[test]
    fn trennt_ein_zu_langes_wort_erst_an_der_nachsten_trennstelle() {
        let lang = "a".repeat(60);
        assert_eq!(
            wrap(&format!("{lang} danach"), 50, 0),
            format!("{lang}\ndanach")
        );
    }

    #[test]
    fn behalt_leerzeilen() {
        assert_eq!(wrap("eins\n\nzwei", 80, 0), "eins\n\nzwei");
    }
}
