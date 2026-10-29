//! Baut das XML einer Windows-Meldung aus dem Modell (Aufbau laut
//! `docs/design/toast/toast-spec.md`); portiert aus
//! `packages/server/src/notifications/ToastXmlBuilder.ts`.

use std::path::Path;

use crate::notifications::model::{ToastKind, ToastModel, ToastOptions};

/// Baut das Toast-XML
#[derive(Debug, Clone, Copy, Default)]
pub struct ToastXmlBuilder;

impl ToastXmlBuilder {
    /// Vollständiges Toast-XML
    pub fn build(&self, model: &ToastModel, options: &ToastOptions) -> String {
        let urgent = matches!(model.kind, ToastKind::Wartet | ToastKind::Gesammelt);
        let sound = if urgent {
            "Notification.Reminder"
        } else {
            "Notification.Default"
        };
        format!(
            "<toast launch=\"{launch}\" activationType=\"protocol\">\
<visual><binding template=\"ToastGeneric\">{images}\
<text>{title}</text><text>{line1}</text><text>{line2}</text>\
</binding></visual><actions>{actions}</actions>\
<audio src=\"ms-winsoundevent:{sound}\"/></toast>",
            launch = attr(&model.open_url),
            images = self.images(model, options),
            title = text(&model.title),
            line1 = text(&model.line1),
            line2 = text(&model.line2),
            actions = self.actions(model),
        )
    }

    /// Logo je Zustand, Hero nur bei „wartet“ und eingeschalteter Einstellung
    fn images(&self, model: &ToastModel, options: &ToastOptions) -> String {
        let logo = asset(&options.assets_dir, self.logo(model));
        let mut xml = format!("<image placement=\"appLogoOverride\" src=\"{logo}\"/>");
        if model.kind == ToastKind::Wartet && options.hero_images {
            let hero = asset(&options.assets_dir, "hero-wartet@2x.png");
            xml.push_str(&format!("<image placement=\"hero\" src=\"{hero}\"/>"));
        }
        xml
    }

    /// Dateiname des Logos je Art der Meldung
    fn logo(&self, model: &ToastModel) -> &'static str {
        match model.kind {
            ToastKind::Bereit => "logo-bereit.png",
            ToastKind::FrageNeu | ToastKind::Recherche => "logo-frage.png",
            _ => "logo-wartet.png",
        }
    }

    /// Einzige Schaltfläche: Board öffnen (bei Fragen „Frage öffnen“, ohne Ticket „Sessions
    /// öffnen“); die Meldung schließt sich selbst
    fn actions(&self, model: &ToastModel) -> String {
        let is_question = matches!(model.kind, ToastKind::FrageNeu | ToastKind::Recherche);
        let opens_sessions = opens_sessions(&model.open_url);
        let open_text = match (is_question, opens_sessions) {
            (true, _) => "Frage öffnen",
            (false, true) => "Sessions öffnen",
            (false, false) => "Board öffnen",
        };
        format!(
            "<action content=\"{open_text}\" arguments=\"{}\" activationType=\"protocol\"/>",
            attr(&model.open_url)
        )
    }
}

/// Endet die Adresse auf `/sessions` bzw. `/sessions/`? (wortgleich zu `/\/sessions\/?$/`)
fn opens_sessions(url: &str) -> bool {
    let trimmed = url.strip_suffix('/').unwrap_or(url);
    trimmed.ends_with("/sessions")
}

/// Pfad eines Bildes als Attributwert
fn asset(dir: &Path, name: &str) -> String {
    attr(&dir.join(name).to_string_lossy())
}

/// Text für Elementinhalt
fn text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Text für Attributwerte
fn attr(value: &str) -> String {
    text(value).replace('"', "&quot;")
}
