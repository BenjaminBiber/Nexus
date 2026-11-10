//! Einzige Entscheidungsstelle, ob der Autopilot für ein Ticket gilt

use crate::dispatch::config::DispatchConfig;

/// Gilt der Autopilot für ein Ticket mit diesen Labels? Schalter an und kein Ausnahme-Label
/// (Vergleich getrimmt, ohne Groß-/Kleinschreibung)
pub fn autopilot_for(cfg: &DispatchConfig, labels: &[String]) -> bool {
    if !cfg.autopilot.enabled {
        return false;
    }
    match cfg.autopilot.except_label.as_deref().map(str::trim) {
        Some(except) if !except.is_empty() => !labels
            .iter()
            .any(|label| label.trim().to_lowercase() == except.to_lowercase()),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::config_normalizer::DispatchConfigNormalizer;
    use serde_json::json;

    fn cfg(enabled: bool) -> DispatchConfig {
        DispatchConfigNormalizer::new()
            .normalize(&json!({ "autopilot": { "enabled": enabled } }))
            .unwrap()
    }

    fn labels(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn aus_gilt_nie() {
        assert!(!autopilot_for(&cfg(false), &labels(&["auto"])));
    }

    #[test]
    fn an_ohne_ausnahme_label_gilt() {
        assert!(autopilot_for(&cfg(true), &labels(&[])));
        assert!(autopilot_for(&cfg(true), &labels(&["auto", "backend"])));
    }

    #[test]
    fn ausnahme_label_in_anderer_schreibweise_gilt_nicht() {
        assert!(!autopilot_for(
            &cfg(true),
            &labels(&["auto", " Interaktiv "])
        ));
    }

    #[test]
    fn ohne_ausnahme_label_gilt_immer() {
        let mut cfg = cfg(true);
        cfg.autopilot.except_label = None;
        assert!(autopilot_for(&cfg, &labels(&["interaktiv"])));
    }
}
