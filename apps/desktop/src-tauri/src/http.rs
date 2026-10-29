//! HTTP-Clients der App. Alle Aufrufe gehen an den eigenen Server auf localhost.
use std::sync::Once;
use std::time::Duration;

static ANBIETER: Once = Once::new();

// Die App teilt sich reqwest mit dem Updater-Plugin, und das bindet es ohne festen
// Kryptoanbieter ein. Ohne gesetzten Anbieter bricht jeder Bau eines Clients ab,
// deshalb wird ring einmalig als Vorgabe eingetragen (bereits ein Fehlschlag, weil ein
// anderer Teil schneller war, ist in Ordnung).
fn anbieter_setzen() {
    ANBIETER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

// Client mit Zeitgrenze
pub fn client(frist: Duration) -> Result<reqwest::Client, reqwest::Error> {
    anbieter_setzen();
    reqwest::Client::builder().timeout(frist).build()
}
