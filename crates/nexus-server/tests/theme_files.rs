//! Auslieferung installierter Themes unter `/themes/{id}/{*path}`.

mod common;

use std::fs;

use common::TestServer;

const GEHEIM: &str = "GEHEIMER-INHALT";

/// Server mit dem installierten Theme `demo-theme` und einer Datei außerhalb des Theme-Ordners
fn server_mit_theme() -> TestServer {
    let server = TestServer::start();
    let themes = server.home.themes_dir();
    let theme = themes.join("demo-theme");
    fs::create_dir_all(theme.join("assets")).unwrap();
    fs::write(theme.join("manifest.json"), "{}").unwrap();
    fs::write(theme.join("theme.css"), "body { color: red; }").unwrap();
    fs::write(theme.join("main.js"), "export default {};").unwrap();
    fs::write(
        theme.join("assets").join("logo.png"),
        [0x89, b'P', b'N', b'G'],
    )
    .unwrap();
    fs::write(theme.join("assets").join("icon.svg"), "<svg/>").unwrap();
    fs::write(themes.join("secret.txt"), GEHEIM).unwrap();
    fs::write(server.home.dir().join("secret.txt"), GEHEIM).unwrap();
    server
}

#[tokio::test]
async fn liefert_theme_css_mit_typ_und_nosniff() {
    let server = server_mit_theme();
    let reply = server.get("/themes/demo-theme/theme.css").await;
    assert_eq!(reply.status, 200);
    assert!(reply
        .header("content-type")
        .unwrap()
        .starts_with("text/css"));
    assert_eq!(reply.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(reply.text, "body { color: red; }");
}

#[tokio::test]
async fn liefert_main_js_als_javascript() {
    let server = server_mit_theme();
    let reply = server.get("/themes/demo-theme/main.js").await;
    assert_eq!(reply.status, 200);
    assert!(reply
        .header("content-type")
        .unwrap()
        .starts_with("text/javascript"));
    assert_eq!(reply.header("x-content-type-options"), Some("nosniff"));
}

#[tokio::test]
async fn liefert_bilder_mit_bildtyp() {
    let server = server_mit_theme();
    for (url, typ) in [
        ("/themes/demo-theme/assets/logo.png", "image/png"),
        ("/themes/demo-theme/assets/icon.svg", "image/svg+xml"),
    ] {
        let reply = server.get(url).await;
        assert_eq!(reply.status, 200, "{url}");
        assert_eq!(reply.header("content-type"), Some(typ), "{url}");
        assert_eq!(reply.header("x-content-type-options"), Some("nosniff"));
    }
}

#[tokio::test]
async fn unbekannte_oder_ungueltige_id_ist_404() {
    let server = server_mit_theme();
    for url in ["/themes/gibt-es-nicht/theme.css", "/themes/A_B/theme.css"] {
        assert_eq!(server.get(url).await.status, 404, "{url}");
    }
}

#[tokio::test]
async fn fehlende_datei_und_ordner_sind_404() {
    let server = server_mit_theme();
    for url in [
        "/themes/demo-theme/fehlt.css",
        "/themes/demo-theme/assets",
        "/themes/demo-theme/assets/",
        "/themes/demo-theme/",
        "/themes/demo-theme",
    ] {
        let reply = server.get(url).await;
        assert_eq!(reply.status, 404, "{url}");
        assert!(!reply.text.contains("<html"), "{url}");
    }
}

#[tokio::test]
async fn pfad_traversal_ist_404() {
    let server = server_mit_theme();
    for url in [
        "/themes/demo-theme/../demo-theme/theme.css",
        "/themes/demo-theme/../secret.txt",
        "/themes/demo-theme/%2e%2e/secret.txt",
        "/themes/demo-theme/assets/..%2f..%2fsecret.txt",
        "/themes/demo-theme/..%5Csecret.txt",
        "/themes/demo-theme/..%5C..%5Csecret.txt",
        "/themes/demo-theme/%2e%2e%2f%2e%2e%2fsecret.txt",
        "/themes/demo-theme/C:%5Csecret.txt",
    ] {
        let reply = server.get(url).await;
        assert_eq!(reply.status, 404, "{url}");
        assert!(!reply.text.contains(GEHEIM), "{url}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_aus_dem_theme_heraus_ist_404() {
    let server = server_mit_theme();
    let theme = server.home.themes_dir().join("demo-theme");
    std::os::unix::fs::symlink(server.home.dir().join("secret.txt"), theme.join("link.txt"))
        .unwrap();
    let reply = server.get("/themes/demo-theme/link.txt").await;
    assert_eq!(reply.status, 404);
    assert!(!reply.text.contains(GEHEIM));
}
