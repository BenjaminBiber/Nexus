//! Text aus einer Option oder einer Datei („-“ = Standardeingabe); für CLI-Schalter wie
//! `--plan`/`--plan-file`. Portiert aus `packages/core/src/text/TextSource.ts`.

use std::fs;
use std::io::Read;

use crate::error::{HarnessError, Result};

/// Liest Text aus Option, Datei oder Standardeingabe.
#[derive(Debug, Clone, Copy, Default)]
pub struct TextSource;

impl TextSource {
    /// Die Datei hat Vorrang vor dem Text in der Option; fehlt beides, bleibt der Wert `None`
    pub fn read(inline: Option<&str>, file: Option<&str>) -> Result<Option<String>> {
        let Some(file) = file else {
            return Ok(inline.map(ToString::to_string));
        };
        let gelesen = if file == "-" {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .map(|_| text)
                .map_err(|_| ())
        } else {
            fs::read_to_string(file).map_err(|_| ())
        };
        gelesen.map(Some).map_err(|()| {
            HarnessError::new(
                "input-unreadable",
                format!(
                    "Die Eingabe {} lässt sich nicht lesen.",
                    if file == "-" {
                        "von der Standardeingabe".to_string()
                    } else {
                        file.to_string()
                    }
                ),
            )
            .into()
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn nimmt_ohne_datei_den_text_der_option() {
        assert_eq!(
            TextSource::read(Some("Plan"), None).unwrap().as_deref(),
            Some("Plan")
        );
        assert_eq!(TextSource::read(None, None).unwrap(), None);
    }

    #[test]
    fn liest_die_datei_und_ubergeht_dabei_den_text_der_option() {
        let dir = tempfile::tempdir().expect("ein temporärer Ordner geht immer");
        let file = dir.path().join("plan.md");
        fs::write(&file, "## Ziel\n").expect("schreiben geht immer");
        assert_eq!(
            TextSource::read(Some("ignoriert"), Some(file.to_str().unwrap()))
                .unwrap()
                .as_deref(),
            Some("## Ziel\n")
        );
    }

    #[test]
    fn meldet_eine_unlesbare_datei() {
        let dir = tempfile::tempdir().expect("ein temporärer Ordner geht immer");
        let fehlt = dir.path().join("fehlt.md");
        let fehler = TextSource::read(None, Some(fehlt.to_str().unwrap())).unwrap_err();
        assert_eq!(fehler.code(), Some("input-unreadable"));
    }
}
