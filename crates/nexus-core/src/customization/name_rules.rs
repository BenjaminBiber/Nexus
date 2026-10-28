//! Portiert `packages/core/src/customization/NameRules.ts`.

/// Regeln für Namen von Skills und Output-Styles: Kleinbuchstaben, Ziffern, einzelne
/// Bindestriche, höchstens 64 Zeichen.
#[derive(Debug, Clone, Copy, Default)]
pub struct NameRules;

impl NameRules {
    /// Neue Regeln (halten keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// true, wenn der Name als Skill- oder Output-Style-Name zulässig ist
    pub fn is_valid(&self, name: &str) -> bool {
        if name.len() > 64 || name.is_empty() {
            return false;
        }
        // Dasselbe wie /^[a-z0-9]+(-[a-z0-9]+)*$/: Gruppen aus Kleinbuchstaben und Ziffern,
        // dazwischen genau ein Bindestrich
        name.split('-').all(|teil| {
            !teil.is_empty()
                && teil
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erlaubt_kleinbuchstaben_ziffern_und_einzelne_bindestriche() {
        let rules = NameRules::new();
        assert!(rules.is_valid("mein-review"));
        assert!(rules.is_valid("harness-implement2"));
    }

    #[test]
    fn lehnt_grossbuchstaben_doppelte_und_randstandige_bindestriche_und_uberlange_ab() {
        let rules = NameRules::new();
        let lang = "a".repeat(65);
        for name in ["Mein", "a--b", "-a", "a-", "mit leer", lang.as_str(), ""] {
            assert!(!rules.is_valid(name), "{name}");
        }
        assert!(rules.is_valid(&"a".repeat(64)));
    }
}
