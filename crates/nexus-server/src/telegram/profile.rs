//! Richtet das Profil des Bots bei Telegram ein: Profilbild, Name, Beschreibungen und
//! Befehlsliste.
//!
//! Der Abgleich läuft im Poller nach `getMe` und über `POST /api/telegram/profile`. Gesetzt
//! wird nur, was sich geändert hat: In `telegram.yaml` steht unter `profile.applied` die
//! Bot-ID samt Fingerabdruck des Bilds und der Texte. Stimmen alle drei, bleibt der Abgleich
//! still – sonst würde jeder Serverstart dieselben Bilder wieder hochladen und in die
//! Rate-Grenze von Telegram laufen.
//!
//! Ein Fehler (eine eigene Bot-API kennt `setMyProfilePhoto` womöglich nicht, Telegram
//! bremst) hält nichts auf: Er steht im Stand des Bots, und der nächste Start versucht es
//! wieder. Fehlertexte nennen nie eine Adresse, weil in jeder Telegram-URL das Bot-Token steht.

use nexus_core::telegram::{AppliedProfile, TelegramConfigStore};
use nexus_core::HarnessHome;

use crate::telegram::api::TelegramApi;
use crate::telegram::texts;

/// Das Profilbild: 640 × 640 JPEG, erzeugt von `assets/brand/render.mjs` aus der Bildmarke
pub const AVATAR: &[u8] = include_bytes!("../../assets/telegram-avatar.jpg");

/// Was ein Abgleich bewirkt hat
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileOutcome {
    /// true = das Profil wurde gerade gesetzt
    pub applied: bool,
    /// Der erste Fehler in Worten; null = keiner
    pub error: Option<String>,
}

impl ProfileOutcome {
    /// Nichts zu tun: abgeschaltet oder alles schon gesetzt
    fn skipped() -> Self {
        Self::default()
    }

    /// Gesetzt, ohne Fehler
    fn done() -> Self {
        Self {
            applied: true,
            error: None,
        }
    }

    /// Nicht gesetzt, weil etwas schiefging
    fn failed(error: String) -> Self {
        Self {
            applied: false,
            error: Some(error),
        }
    }
}

/// Gleicht das Profil des Bots mit dem ab, was Nexus einrichten will.
#[derive(Debug, Clone)]
pub struct BotProfileSync {
    config: TelegramConfigStore,
}

impl BotProfileSync {
    /// Abgleich auf der Einstellung in diesem Benutzerordner
    pub fn new(home: HarnessHome) -> Self {
        Self {
            config: TelegramConfigStore::new(home),
        }
    }

    /// Richtet das Profil ein, wenn nötig.
    ///
    /// `bot_id` ist die ID aus `getMe`, `commands` die Befehlsliste des Routers. `force`
    /// setzt auch dann, wenn sich nichts geändert hat (die Route erzwingt es so).
    pub async fn sync(
        &self,
        api: &dyn TelegramApi,
        bot_id: i64,
        commands: &[(String, String)],
        force: bool,
    ) -> ProfileOutcome {
        let mut config = match self.config.read() {
            Ok(config) => config,
            Err(error) => {
                return ProfileOutcome::failed(format!("Einstellung nicht lesbar: {error}"))
            }
        };
        let description = texts::description(commands);
        // Die Befehlsliste gehört zum Fingerabdruck der Texte: meldet ein späteres Ticket
        // einen Befehl an, setzt der nächste Start sie neu
        let mut parts: Vec<&[u8]> = vec![
            texts::NAME.as_bytes(),
            texts::SHORT_DESCRIPTION.as_bytes(),
            description.as_bytes(),
        ];
        for (name, description) in commands {
            parts.push(name.as_bytes());
            parts.push(description.as_bytes());
        }
        let applied = AppliedProfile {
            bot_id,
            photo: fingerprint([AVATAR]),
            texts: fingerprint(parts),
        };
        if !force && config.profile.applied.as_ref() == Some(&applied) {
            return ProfileOutcome::skipped();
        }
        // Bild zuerst: es ist der Aufruf, der am ehesten scheitert (eigene Bot-API, Rate-Grenze)
        if let Err(error) = api.set_my_profile_photo(AVATAR.to_vec()).await {
            return ProfileOutcome::failed(format!("Profilbild nicht gesetzt: {error}"));
        }
        if let Err(error) = api.set_my_name(texts::NAME).await {
            return ProfileOutcome::failed(format!("Name des Bots nicht gesetzt: {error}"));
        }
        if let Err(error) = api.set_my_short_description(texts::SHORT_DESCRIPTION).await {
            return ProfileOutcome::failed(format!("Kurzbeschreibung nicht gesetzt: {error}"));
        }
        if let Err(error) = api.set_my_description(&description).await {
            return ProfileOutcome::failed(format!("Beschreibung nicht gesetzt: {error}"));
        }
        if let Err(error) = api.set_my_commands(commands).await {
            return ProfileOutcome::failed(format!("Befehlsliste nicht gesetzt: {error}"));
        }
        // Erst merken, wenn alles durch ist: ein halb gesetztes Profil soll der nächste
        // Start wieder anfassen
        config.profile.applied = Some(applied);
        if let Err(error) = self.config.write(&config) {
            return ProfileOutcome::failed(format!(
                "Das eingerichtete Profil ließ sich nicht merken: {error}"
            ));
        }
        ProfileOutcome::done()
    }
}

/// Fingerabdruck über mehrere Teile: SHA-1 in Hexadezimalschreibweise.
///
/// Zwischen den Teilen steht ein Trennzeichen, das in keinem Text vorkommt; sonst ergäben
/// („ab“, „c“) und („a“, „bc“) denselben Abdruck.
fn fingerprint<'a>(parts: impl IntoIterator<Item = &'a [u8]>) -> String {
    let mut sha1 = sha1_smol::Sha1::new();
    for part in parts {
        sha1.update(part);
        sha1.update(&[0]);
    }
    sha1.digest().to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nexus_core::telegram::{BotProfile, TelegramConfig};

    use super::*;
    use crate::telegram::api::{TelegramError, User};
    use crate::telegram::fake::FakeTelegram;

    /// Ein Token, das dem Muster von @BotFather entspricht
    const TOKEN: &str = "123456:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw";

    /// Die Bot-ID der Attrappe
    const BOT_ID: i64 = 7012345678;

    /// Abgleich samt Attrappe auf einem eigenen Benutzerordner
    struct Harness {
        _dir: tempfile::TempDir,
        home: HarnessHome,
        fake: Arc<FakeTelegram>,
        sync: BotProfileSync,
    }

    impl Harness {
        /// Benutzerordner mit dieser Einstellung
        fn new(profile: BotProfile) -> Self {
            let dir = tempfile::Builder::new()
                .prefix("harness-telegram-profile-")
                .tempdir()
                .expect("Ordner");
            let home = HarnessHome::at(dir.path());
            TelegramConfigStore::new(home.clone())
                .write(&TelegramConfig {
                    enabled: true,
                    bot_token: TOKEN.to_string(),
                    profile,
                    ..TelegramConfig::default()
                })
                .expect("Einstellung");
            Self {
                _dir: dir,
                fake: Arc::new(FakeTelegram::new()),
                sync: BotProfileSync::new(home.clone()),
                home,
            }
        }

        /// Ein Abgleich für den Bot der Attrappe
        async fn sync(&self) -> ProfileOutcome {
            self.sync_of(BOT_ID, false).await
        }

        /// Ein Abgleich für diese Bot-ID, auf Wunsch erzwungen
        async fn sync_of(&self, bot_id: i64, force: bool) -> ProfileOutcome {
            self.sync
                .sync(self.fake.as_ref(), bot_id, &commands(), force)
                .await
        }

        /// Die gespeicherte Einstellung
        fn config(&self) -> TelegramConfig {
            TelegramConfigStore::new(self.home.clone())
                .read()
                .expect("Einstellung")
        }

        /// Die Namen der aufgezeichneten Aufrufe in ihrer Reihenfolge
        fn methods(&self) -> Vec<String> {
            self.fake
                .take_calls()
                .into_iter()
                .map(|call| call.method)
                .collect()
        }
    }

    /// Eine Befehlsliste wie die des Routers
    fn commands() -> Vec<(String, String)> {
        vec![
            ("start".to_string(), "Begrüßung und Kopplung".to_string()),
            ("hilfe".to_string(), "Diese Liste".to_string()),
        ]
    }

    /// Alle Aufrufe, die ein Abgleich macht, in ihrer Reihenfolge
    fn alle_aufrufe() -> Vec<String> {
        [
            "setMyProfilePhoto",
            "setMyName",
            "setMyShortDescription",
            "setMyDescription",
            "setMyCommands",
        ]
        .map(ToString::to_string)
        .to_vec()
    }

    #[tokio::test]
    async fn setzt_beim_ersten_mal_bild_name_beschreibungen_und_befehle() {
        let h = Harness::new(BotProfile::default());

        let outcome = h.sync().await;

        assert_eq!(outcome, ProfileOutcome::done());
        assert_eq!(h.methods(), alle_aufrufe());
        let applied = h.config().profile.applied.expect("gemerkt");
        assert_eq!(applied.bot_id, BOT_ID);
        assert_eq!(applied.photo.len(), 40, "SHA-1 in Hexadezimalschreibweise");
    }

    #[tokio::test]
    async fn schickt_das_bild_und_die_texte_wirklich_mit() {
        let h = Harness::new(BotProfile::default());

        h.sync().await;

        let photo = h.fake.calls_of("setMyProfilePhoto");
        assert_eq!(photo[0].params["bytes"], AVATAR.len());
        assert_eq!(h.fake.calls_of("setMyName")[0].params["name"], texts::NAME);
        let kurz = h.fake.calls_of("setMyShortDescription");
        assert_eq!(
            kurz[0].params["short_description"],
            texts::SHORT_DESCRIPTION
        );
        let lang = h.fake.calls_of("setMyDescription");
        let beschreibung = lang[0].params["description"].as_str().unwrap_or_default();
        assert!(
            beschreibung.contains("/hilfe – Diese Liste"),
            "{beschreibung}"
        );
    }

    #[tokio::test]
    async fn ruhrt_den_zweiten_start_nicht_an() {
        let h = Harness::new(BotProfile::default());
        h.sync().await;
        h.methods();

        let outcome = h.sync().await;

        assert_eq!(outcome, ProfileOutcome::skipped());
        assert!(h.methods().is_empty(), "nichts wird wieder gesetzt");
    }

    #[tokio::test]
    async fn setzt_fur_einen_anderen_bot_erneut() {
        let h = Harness::new(BotProfile::default());
        h.sync().await;
        h.methods();

        let outcome = h.sync_of(999, false).await;

        assert_eq!(outcome, ProfileOutcome::done());
        assert_eq!(h.methods(), alle_aufrufe());
        assert_eq!(h.config().profile.applied.unwrap().bot_id, 999);
    }

    #[tokio::test]
    async fn setzt_bei_einem_neuen_bild_erneut() {
        // Ein anderes Bild heißt: der gemerkte Fingerabdruck passt nicht mehr
        let h = Harness::new(BotProfile {
            applied: Some(AppliedProfile {
                bot_id: BOT_ID,
                photo: "ein alter Abdruck".to_string(),
                texts: fingerprint([&[] as &[u8]]),
            }),
        });

        let outcome = h.sync().await;

        assert_eq!(outcome, ProfileOutcome::done());
        assert_eq!(h.methods(), alle_aufrufe());
    }

    #[tokio::test]
    async fn setzt_bei_geanderten_befehlen_erneut() {
        let h = Harness::new(BotProfile::default());
        h.sync().await;
        h.methods();

        let outcome = h
            .sync
            .sync(
                h.fake.as_ref(),
                BOT_ID,
                &[("status".to_string(), "neu dabei".to_string())],
                false,
            )
            .await;

        assert_eq!(outcome, ProfileOutcome::done());
        assert_eq!(h.methods(), alle_aufrufe());
    }

    #[tokio::test]
    async fn setzt_erzwungen_auch_ohne_anderung() {
        let h = Harness::new(BotProfile::default());
        h.sync().await;
        h.methods();

        let outcome = h.sync_of(BOT_ID, true).await;

        assert_eq!(outcome, ProfileOutcome::done());
        assert_eq!(h.methods(), alle_aufrufe());
    }

    #[tokio::test]
    async fn meldet_einen_fehler_des_profilbilds_und_merkt_nichts() {
        let h = Harness::new(BotProfile::default());
        h.fake.fail(
            "setMyProfilePhoto",
            TelegramError::Api {
                code: 400,
                description: "method not found".to_string(),
            },
        );

        let outcome = h.sync().await;

        assert!(!outcome.applied);
        let error = outcome.error.expect("Fehler");
        assert!(error.starts_with("Profilbild nicht gesetzt: "), "{error}");
        assert!(!error.contains(TOKEN), "{error}");
        assert_eq!(
            h.config().profile.applied,
            None,
            "der nächste Start versucht es wieder"
        );
        // Nach dem ersten Fehler bleiben die weiteren Aufrufe aus
        assert_eq!(h.methods(), vec!["setMyProfilePhoto".to_string()]);
    }

    #[tokio::test]
    async fn meldet_einen_fehler_der_befehlsliste() {
        let h = Harness::new(BotProfile::default());
        h.fake.fail(
            "setMyCommands",
            TelegramError::TooManyRequests { retry_after: 7 },
        );

        let outcome = h.sync().await;

        let error = outcome.error.expect("Fehler");
        assert!(error.starts_with("Befehlsliste nicht gesetzt: "), "{error}");
        assert_eq!(h.config().profile.applied, None);
    }

    #[tokio::test]
    async fn versucht_es_nach_einem_fehler_beim_nachsten_mal_wieder() {
        let h = Harness::new(BotProfile::default());
        h.fake.fail("setMyName", TelegramError::Conflict);
        h.sync().await;
        h.methods();
        h.fake.clear_failure("setMyName");

        let outcome = h.sync().await;

        assert_eq!(outcome, ProfileOutcome::done());
        assert_eq!(h.methods(), alle_aufrufe());
    }

    #[test]
    fn das_eingebaute_bild_ist_ein_jpeg_mit_640_x_640() {
        let (width, height) = jpeg_size(AVATAR).expect("Maße im JPEG");

        assert_eq!(&AVATAR[0..2], b"\xff\xd8", "JPEG beginnt mit SOI");
        assert_eq!((width, height), (640, 640));
    }

    #[test]
    fn unterscheidet_dieselben_teile_in_anderer_aufteilung() {
        assert_ne!(
            fingerprint([b"ab".as_slice(), b"c".as_slice()]),
            fingerprint([b"a".as_slice(), b"bc".as_slice()])
        );
    }

    /// Breite und Höhe eines JPEG aus dem Segment SOF0/SOF2; `None`, wenn keines kommt.
    /// Von Hand gelesen, damit kein Bildpaket als Abhängigkeit dazukommt
    fn jpeg_size(bytes: &[u8]) -> Option<(u16, u16)> {
        let mut at = 2;
        while at + 9 < bytes.len() {
            if bytes[at] != 0xFF {
                return None;
            }
            let marker = bytes[at + 1];
            let length = u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]) as usize;
            // SOF0 (Baseline) und SOF2 (progressiv) nennen Höhe und Breite nach der Genauigkeit
            if marker == 0xC0 || marker == 0xC2 {
                let height = u16::from_be_bytes([bytes[at + 5], bytes[at + 6]]);
                let width = u16::from_be_bytes([bytes[at + 7], bytes[at + 8]]);
                return Some((width, height));
            }
            at += 2 + length;
        }
        None
    }

    #[test]
    fn liest_die_masse_eines_bildes_ohne_sof_nicht() {
        assert_eq!(jpeg_size(b"\xff\xd8kein Segment"), None);
    }

    /// Die Attrappe liefert diesen Bot; der Test hält die ID in [`BOT_ID`] fest
    #[tokio::test]
    async fn die_attrappe_hat_die_erwartete_bot_id() {
        let me: User = FakeTelegram::new().get_me().await.unwrap();
        assert_eq!(me.id, BOT_ID);
    }
}
