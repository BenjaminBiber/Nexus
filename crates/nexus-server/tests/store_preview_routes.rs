//! Routengruppe `store_preview`: `GET /api/themes/store/preview?repoId=…&id=…` liefert das
//! Vorschaubild eines Store-Eintrags über den Server, weil die CSP der Oberfläche keine Bilder
//! fremder Hosts zulässt. Repo und Bild-Server sind eine Attrappe aus axum im eigenen Prozess –
//! nichts verlässt den Rechner.

mod common;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use common::{Reply, TestServer, TestServerOptions};
use serde_json::{json, Value};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR-vorschau";
const JPEG: &[u8] = b"\xff\xd8\xff\xe0\x00\x10JFIF-vorschau";
const WEBP: &[u8] = b"RIFF\x1a\x00\x00\x00WEBPVP8 vorschau";
const GIF: &[u8] = b"GIF89a-vorschau";
const SVG: &[u8] = b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>";

/// Die Attrappe: Adresse und Zahl der Abrufe von `/index.json`
struct Stub {
    addr: SocketAddr,
    index_loads: Arc<AtomicUsize>,
}

impl Stub {
    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.addr.port())
    }

    fn index_loads(&self) -> usize {
        self.index_loads.load(Ordering::SeqCst)
    }
}

/// Index-Eintrag `id`; `preview` ist der Pfad des Bilds auf der Attrappe
fn eintrag(addr: SocketAddr, id: &str, preview: Option<&str>) -> Value {
    let mut entry = json!({
        "id": id,
        "name": id,
        "version": "1.0.0",
        "sdkVersion": "^1.0",
        "downloadUrl": format!("http://127.0.0.1:{}/{id}.nexus", addr.port()),
        "sha256": "0".repeat(64),
    });
    if let Some(path) = preview {
        entry["preview"] = json!(format!("http://127.0.0.1:{}{path}", addr.port()));
    }
    entry
}

/// Startet die Attrappe mit dem Index aus `eintraege` und den Dateien `dateien` (Pfad → Inhalt).
/// Dazu immer `/riesig.png` (6 MiB) und `/langsam.png` (5 s)
async fn stub(
    eintraege: impl FnOnce(SocketAddr) -> Vec<Value>,
    dateien: Vec<(&'static str, &'static [u8])>,
) -> Stub {
    stub_mit_folge(|addr| vec![eintraege(addr)], dateien).await
}

/// Wie [`stub`], aber der n-te Abruf von `/index.json` liefert den n-ten Index; der letzte
/// gilt für alle weiteren
async fn stub_mit_folge(
    folge: impl FnOnce(SocketAddr) -> Vec<Vec<Value>>,
    dateien: Vec<(&'static str, &'static [u8])>,
) -> Stub {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("Port der Attrappe");
    let addr = listener.local_addr().expect("Adresse der Attrappe");
    let folge: Vec<String> = folge(addr)
        .into_iter()
        .map(|eintraege| Value::Array(eintraege).to_string())
        .collect();
    let index_loads = Arc::new(AtomicUsize::new(0));
    let zaehler = index_loads.clone();
    let mut app = Router::new()
        .route(
            "/index.json",
            get(move || {
                let abruf = zaehler.fetch_add(1, Ordering::SeqCst);
                let index = folge[abruf.min(folge.len() - 1)].clone();
                async move { index }
            }),
        )
        .route(
            "/riesig.png",
            get(|| async {
                let mut bytes = PNG.to_vec();
                bytes.resize(6 * 1024 * 1024, 0);
                bytes
            }),
        )
        .route(
            "/langsam.png",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                PNG
            }),
        );
    for (path, inhalt) in dateien {
        app = app.route(path, get(move || async move { inhalt }));
    }
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Stub { addr, index_loads }
}

fn server() -> TestServer {
    TestServer::with(TestServerOptions {
        store_index_timeout: Some(Duration::from_secs(5)),
        store_download_timeout: Some(Duration::from_millis(1000)),
        ..Default::default()
    })
}

/// Trägt das Repo der Attrappe ein und liefert seine ID
async fn add_repo(server: &TestServer, stub: &Stub) -> String {
    let body = json!({ "url": stub.url("/index.json") });
    let reply = server
        .post("/api/themes/store/repos", &body.to_string())
        .await;
    assert_eq!(reply.status, 201, "{}", reply.text);
    reply.body()["id"].as_str().unwrap().to_string()
}

async fn preview(server: &TestServer, repo: &str, id: &str) -> Reply {
    server
        .get(&format!(
            "/api/themes/store/preview?repoId={repo}&id={id}&v=1.0.0"
        ))
        .await
}

/// Der Körper einer Antwort, so wie ihn `Reply::text` aus den Bytes liest
fn als_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[tokio::test]
async fn liefert_das_vorschaubild_eines_store_eintrags() {
    let stub = stub(
        |addr| vec![eintrag(addr, "ozean", Some("/ozean.png"))],
        vec![("/ozean.png", PNG)],
    )
    .await;
    let server = server();
    let repo = add_repo(&server, &stub).await;

    let reply = preview(&server, &repo, "ozean").await;

    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.header("content-type"), Some("image/png"));
    assert_eq!(reply.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(
        reply.header("cache-control"),
        Some("private, max-age=86400")
    );
    assert_eq!(reply.text, als_text(PNG));
}

/// Die Adresse sagt nichts über den Inhalt: der Typ kommt aus den ersten Bytes
#[tokio::test]
async fn erkennt_den_bildtyp_am_inhalt() {
    let stub = stub(
        |addr| {
            vec![
                eintrag(addr, "jpeg", Some("/bild-a")),
                eintrag(addr, "webp", Some("/bild-b")),
                eintrag(addr, "gif", Some("/bild-c.png")),
            ]
        },
        vec![("/bild-a", JPEG), ("/bild-b", WEBP), ("/bild-c.png", GIF)],
    )
    .await;
    let server = server();
    let repo = add_repo(&server, &stub).await;

    for (id, typ, bytes) in [
        ("jpeg", "image/jpeg", JPEG),
        ("webp", "image/webp", WEBP),
        ("gif", "image/gif", GIF),
    ] {
        let reply = preview(&server, &repo, id).await;
        assert_eq!(reply.status, StatusCode::OK, "{id}: {}", reply.text);
        assert_eq!(reply.header("content-type"), Some(typ), "{id}");
        assert_eq!(reply.text, als_text(bytes), "{id}");
    }
}

/// Der Server reicht nur Bilder durch: HTML und SVG (kann Skripte tragen) kämen sonst unter
/// seiner Herkunft an
#[tokio::test]
async fn lehnt_inhalte_ab_die_kein_bild_sind() {
    let stub = stub(
        |addr| {
            vec![
                eintrag(addr, "html", Some("/seite.png")),
                eintrag(addr, "svg", Some("/bild.svg")),
                eintrag(addr, "leer", Some("/leer.png")),
            ]
        },
        vec![
            ("/seite.png", b"<!doctype html><script>alert(1)</script>"),
            ("/bild.svg", SVG),
            ("/leer.png", b""),
        ],
    )
    .await;
    let server = server();
    let repo = add_repo(&server, &stub).await;

    for id in ["html", "svg", "leer"] {
        let reply = preview(&server, &repo, id).await;
        assert_eq!(
            reply.status,
            StatusCode::BAD_GATEWAY,
            "{id}: {}",
            reply.text
        );
        assert_eq!(
            reply.code().as_deref(),
            Some("store-preview-invalid"),
            "{id}"
        );
    }
}

#[tokio::test]
async fn meldet_404_ohne_vorschau_ohne_eintrag_und_ohne_repo() {
    let stub = stub(|addr| vec![eintrag(addr, "ohne", None)], vec![]).await;
    let server = server();
    let repo = add_repo(&server, &stub).await;

    let ohne_vorschau = preview(&server, &repo, "ohne").await;
    assert_eq!(ohne_vorschau.status, StatusCode::NOT_FOUND);
    assert_eq!(
        ohne_vorschau.code().as_deref(),
        Some("store-preview-not-found")
    );

    let ohne_eintrag = preview(&server, &repo, "fehlt").await;
    assert_eq!(ohne_eintrag.status, StatusCode::NOT_FOUND);
    assert_eq!(
        ohne_eintrag.code().as_deref(),
        Some("store-entry-not-found")
    );

    let ohne_repo = preview(&server, "gibt-es-nicht", "ohne").await;
    assert_eq!(ohne_repo.status, StatusCode::NOT_FOUND);
    assert_eq!(ohne_repo.code().as_deref(), Some("store-repo-not-found"));
}

#[tokio::test]
async fn verlangt_repo_und_id() {
    let server = server();
    for url in [
        "/api/themes/store/preview",
        "/api/themes/store/preview?repoId=r1",
        "/api/themes/store/preview?id=ozean",
        "/api/themes/store/preview?repoId=&id=ozean",
    ] {
        let reply = server.get(url).await;
        assert_eq!(
            reply.status,
            StatusCode::BAD_REQUEST,
            "{url}: {}",
            reply.text
        );
        assert_eq!(reply.code().as_deref(), Some("bad-request"), "{url}");
    }
}

/// Auch ein Eintrag, der nicht zur SDK-Version passt, steht mit Bild im Store
#[tokio::test]
async fn liefert_das_bild_auch_fuer_einen_nicht_kompatiblen_eintrag() {
    let stub = stub(
        |addr| {
            let mut entry = eintrag(addr, "morgen", Some("/morgen.png"));
            entry["sdkVersion"] = json!("^99.0");
            vec![entry]
        },
        vec![("/morgen.png", PNG)],
    )
    .await;
    let server = server();
    let repo = add_repo(&server, &stub).await;

    let reply = preview(&server, &repo, "morgen").await;

    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.header("content-type"), Some("image/png"));
}

#[tokio::test]
async fn lehnt_zu_grosse_und_zu_langsame_bilder_ab() {
    let stub = stub(
        |addr| {
            vec![
                eintrag(addr, "riesig", Some("/riesig.png")),
                eintrag(addr, "langsam", Some("/langsam.png")),
                eintrag(addr, "weg", Some("/gibt-es-nicht.png")),
            ]
        },
        vec![],
    )
    .await;
    let server = server();
    let repo = add_repo(&server, &stub).await;

    let riesig = preview(&server, &repo, "riesig").await;
    assert_eq!(
        riesig.status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "{}",
        riesig.text
    );
    assert_eq!(riesig.code().as_deref(), Some("store-download-too-large"));

    let langsam = preview(&server, &repo, "langsam").await;
    assert_eq!(
        langsam.status,
        StatusCode::GATEWAY_TIMEOUT,
        "{}",
        langsam.text
    );
    assert_eq!(langsam.code().as_deref(), Some("store-download-timeout"));

    let weg = preview(&server, &repo, "weg").await;
    assert_eq!(weg.status, StatusCode::BAD_GATEWAY, "{}", weg.text);
    assert_eq!(weg.code().as_deref(), Some("store-download-http"));
}

/// Der Store zeigt viele Bilder auf einmal: jedes fragt den Server, aber der Index des Repos
/// wird dafür nicht jedes Mal neu geladen
#[tokio::test]
async fn laedt_den_index_nicht_fuer_jedes_bild_neu() {
    let stub = stub(
        |addr| {
            vec![
                eintrag(addr, "ozean", Some("/ozean.png")),
                eintrag(addr, "wald", Some("/wald.png")),
            ]
        },
        vec![("/ozean.png", PNG), ("/wald.png", PNG)],
    )
    .await;
    let server = server();
    let repo = add_repo(&server, &stub).await;

    // Die Oberfläche lädt erst den Index, dann die Bilder
    let index = server.get("/api/themes/store/index").await;
    assert_eq!(index.status, StatusCode::OK, "{}", index.text);
    assert_eq!(stub.index_loads(), 1);

    for id in ["ozean", "wald", "ozean"] {
        let reply = preview(&server, &repo, id).await;
        assert_eq!(reply.status, StatusCode::OK, "{id}: {}", reply.text);
    }
    assert_eq!(stub.index_loads(), 1);
}

/// Ohne vorher geladenen Index (Server frisch gestartet, Bild noch im Fenster) holt die Route
/// ihn selbst – einmal
#[tokio::test]
async fn holt_den_index_selbst_wenn_er_noch_nicht_geladen_war() {
    let stub = stub(
        |addr| vec![eintrag(addr, "ozean", Some("/ozean.png"))],
        vec![("/ozean.png", PNG)],
    )
    .await;
    let server = server();
    let repo = add_repo(&server, &stub).await;
    assert_eq!(stub.index_loads(), 0);

    for _ in 0..2 {
        let reply = preview(&server, &repo, "ozean").await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    }
    assert_eq!(stub.index_loads(), 1);
}

/// Ein neu geladener Index ersetzt die gemerkten Adressen: eine entfernte Vorschau bleibt
/// nicht hängen
#[tokio::test]
async fn ein_neu_geladener_index_ersetzt_die_gemerkten_adressen() {
    // Erster Abruf des Index mit Vorschau, danach ohne
    let stub = stub_mit_folge(
        |addr| {
            vec![
                vec![eintrag(addr, "ozean", Some("/ozean.png"))],
                vec![eintrag(addr, "ozean", None)],
            ]
        },
        vec![("/ozean.png", PNG)],
    )
    .await;
    let server = server();
    let repo = add_repo(&server, &stub).await;
    assert_eq!(
        preview(&server, &repo, "ozean").await.status,
        StatusCode::OK
    );

    let index = server.get("/api/themes/store/index").await;
    assert_eq!(index.status, StatusCode::OK, "{}", index.text);
    assert_eq!(stub.index_loads(), 2);
    let reply = preview(&server, &repo, "ozean").await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND, "{}", reply.text);
    assert_eq!(reply.code().as_deref(), Some("store-preview-not-found"));
}
