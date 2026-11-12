//! Sicheres Telegram-HTML: Escaping der drei Zeichen, die Telegram als Markup liest
//! (`<`, `>`, `&`), und kleine Bausteine für die üblichen Tags. Nur `parse_mode=HTML`
//! kennt diese Tags; alles, was in den Text selbst geht, muss escapet sein, sonst zerreißt
//! fremder Text (Ticket-Titel, Kommentare) das Markup oder liefert einen Fehler von Telegram.

/// Escapet `&`, `<`, `>` für Telegram-HTML; Reihenfolge wichtig, sonst escaped `&` erneut
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escapet zusätzlich `"`, für Text innerhalb eines Attributwerts (z. B. `href`)
fn escape_attr(value: &str) -> String {
    escape(value).replace('"', "&quot;")
}

/// Fett: `<b>…</b>`
pub fn bold(text: &str) -> String {
    format!("<b>{}</b>", escape(text))
}

/// Kursiv: `<i>…</i>`
pub fn italic(text: &str) -> String {
    format!("<i>{}</i>", escape(text))
}

/// Feste Breite: `<code>…</code>`
pub fn code(text: &str) -> String {
    format!("<code>{}</code>", escape(text))
}

/// Verweis: `<a href="…">…</a>`
pub fn link(text: &str, url: &str) -> String {
    format!("<a href=\"{}\">{}</a>", escape_attr(url), escape(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapt_die_drei_zeichen() {
        assert_eq!(escape("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn escapt_amp_vor_den_anderen_damit_nichts_doppelt_escapet_wird() {
        assert_eq!(escape("&lt;"), "&amp;lt;");
    }

    #[test]
    fn laesst_unbedenklichen_text_unveraendert() {
        assert_eq!(escape("Ticket AIH-0102: fertig"), "Ticket AIH-0102: fertig");
    }

    #[test]
    fn baut_die_bausteine_und_escapt_ihren_inhalt() {
        assert_eq!(bold("a < b"), "<b>a &lt; b</b>");
        assert_eq!(italic("a & b"), "<i>a &amp; b</i>");
        assert_eq!(code("<script>"), "<code>&lt;script&gt;</code>");
    }

    #[test]
    fn baut_einen_verweis_und_escapt_text_und_adresse() {
        assert_eq!(
            link("Board & Co", "https://example.com/?a=1&b=\"x\""),
            "<a href=\"https://example.com/?a=1&amp;b=&quot;x&quot;\">Board &amp; Co</a>"
        );
    }
}
