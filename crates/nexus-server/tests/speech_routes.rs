//! Spracherkennung: `POST /api/speech/test` und das Backend mit der Schnittstelle von OpenAI.
//!
//! Der Erkennungsdienst ist eine Attrappe aus axum im eigenen Prozess – nichts verlässt den
//! Rechner, und kein Test ruft OpenAI. Die Attrappe schreibt mit, welche Multipart-Felder und
//! welche Köpfe ankamen.

mod common;

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::Router;
use common::{TestServer, TestServerOptions};
use nexus_core::board::ticket_service::{NewTicketInput, WriteOptions};
use nexus_core::telegram::{SpeechBackend, SpeechConfig, TelegramConfig, TelegramConfigStore};
use nexus_server::speech::{self, AudioClip, OpenAiCompatibleSpeech, SpeechHint, SpeechToText};

/// Eine empfangene Anfrage der Attrappe: die Köpfe und die Felder des Multipart-Körpers
#[derive(Debug, Clone, Default)]
struct Received {
    /// Alle Köpfe der Anfrage, klein geschrieben
    headers: BTreeMap<String, String>,
    /// Die Textfelder des Multipart-Körpers
    fields: BTreeMap<String, String>,
    /// Name der Datei im Feld `file`
    file_name: Option<String>,
    /// Inhaltstyp der Datei im Feld `file`
    file_mime: Option<String>,
    /// Größe der Datei in Bytes
    file_bytes: usize,
}

/// Was die Attrappe antwortet
#[derive(Debug, Clone)]
enum StubReply {
    /// Erfolg mit diesem Körper
    Ok(&'static str),
    /// Fehlerstatus mit diesem Körper
    Error(StatusCode, &'static str),
    /// Antwortet erst nach dieser Zeit (für das Zeitlimit)
    Slow(Duration),
}

/// Attrappe eines Dienstes mit der Schnittstelle von OpenAI
struct SpeechStub {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<Received>>>,
}

impl SpeechStub {
    /// Startet die Attrappe auf einem freien Port
    async fn start(reply: StubReply) -> SpeechStub {
        let received: Arc<Mutex<Vec<Received>>> = Arc::new(Mutex::new(Vec::new()));
        let seen = received.clone();
        let app = Router::new()
            .route(
                "/v1/audio/transcriptions",
                post(move |headers: HeaderMap, body: axum::body::Bytes| {
                    let seen = seen.clone();
                    let reply = reply.clone();
                    async move {
                        seen.lock()
                            .expect("Sperre")
                            .push(parse_multipart(&headers, &body));
                        match reply {
                            StubReply::Ok(body) => (StatusCode::OK, body.to_string()),
                            StubReply::Error(status, body) => (status, body.to_string()),
                            StubReply::Slow(wait) => {
                                tokio::time::sleep(wait).await;
                                (StatusCode::OK, "{\"text\":\"zu spät\"}".to_string())
                            }
                        }
                    }
                }),
            )
            .layer(DefaultBodyLimit::max(32 * 1024 * 1024));
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("Port der Attrappe");
        let addr = listener.local_addr().expect("Adresse der Attrappe");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        SpeechStub { addr, received }
    }

    /// Die Adresse, die in die Einstellung gehört
    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Die erste empfangene Anfrage
    fn first(&self) -> Received {
        self.received
            .lock()
            .expect("Sperre")
            .first()
            .cloned()
            .expect("keine Anfrage an der Attrappe")
    }

    /// Zahl der empfangenen Anfragen
    fn count(&self) -> usize {
        self.received.lock().expect("Sperre").len()
    }
}

/// Liest einen Multipart-Körper so weit, wie die Tests ihn brauchen: Textfelder, Dateiname,
/// Inhaltstyp und Größe der Datei. Eigene Zerlegung, damit kein weiteres Paket nötig ist.
fn parse_multipart(headers: &HeaderMap, body: &[u8]) -> Received {
    let mut received = Received::default();
    for (name, value) in headers {
        if let Ok(value) = value.to_str() {
            received
                .headers
                .insert(name.as_str().to_lowercase(), value.to_string());
        }
    }
    let Some(boundary) = received
        .headers
        .get("content-type")
        .and_then(|value| value.split("boundary=").nth(1))
        .map(|value| format!("--{}", value.trim_matches('"')))
    else {
        return received;
    };
    for part in split_on(body, boundary.as_bytes()) {
        // Kopf und Rumpf des Teils trennt eine Leerzeile
        let Some(split) = find(&part, b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&part[..split]).to_string();
        let mut content = &part[split + 4..];
        // Der Rumpf endet vor dem Zeilenumbruch, der zur nächsten Grenze gehört
        if content.ends_with(b"\r\n") {
            content = &content[..content.len() - 2];
        }
        let Some(name) = between(&head, "name=\"", "\"") else {
            continue;
        };
        match between(&head, "filename=\"", "\"") {
            Some(file_name) => {
                received.file_name = Some(file_name);
                received.file_mime = head
                    .lines()
                    .find_map(|line| {
                        line.to_lowercase().starts_with("content-type:").then(|| {
                            line.split_once(':')
                                .map(|(_, value)| value.trim().to_string())
                        })
                    })
                    .flatten();
                received.file_bytes = content.len();
            }
            None => {
                received
                    .fields
                    .insert(name, String::from_utf8_lossy(content).to_string());
            }
        }
    }
    received
}

/// Teilt einen Körper an der Multipart-Grenze; die Teile vor der ersten und nach der letzten
/// Grenze fallen weg
fn split_on(body: &[u8], boundary: &[u8]) -> Vec<Vec<u8>> {
    let mut parts = Vec::new();
    let mut rest = body;
    while let Some(start) = find(rest, boundary) {
        rest = &rest[start + boundary.len()..];
        let Some(end) = find(rest, boundary) else {
            break;
        };
        parts.push(rest[..end].to_vec());
    }
    parts
}

/// Die erste Stelle, an der `needle` in `haystack` steht
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Der Text zwischen zwei Markierungen
fn between(text: &str, start: &str, end: &str) -> Option<String> {
    let rest = text.split_once(start)?.1;
    Some(rest.split_once(end)?.0.to_string())
}

// ---------------------------------------------------------------------------------------------
// Das Backend selbst
// ---------------------------------------------------------------------------------------------

/// Erkennung gegen die Attrappe, mit oder ohne Schlüssel
fn speech(stub: &SpeechStub, api_key: &str) -> OpenAiCompatibleSpeech {
    OpenAiCompatibleSpeech::from_config(&SpeechConfig {
        base_url: stub.url(),
        api_key: api_key.to_string(),
        model: "whisper-1".to_string(),
        ..SpeechConfig::default()
    })
}

/// Eine kurze Aufnahme mit bekannter Länge
fn clip() -> AudioClip {
    AudioClip::new(b"OggS-Attrappe".to_vec(), "audio/ogg").with_seconds(Some(3.0))
}

#[tokio::test]
async fn schickt_file_model_language_und_prompt_als_multipart() {
    let stub = SpeechStub::start(StubReply::Ok(
        "{\"text\":\" Hallo Nexus. \",\"language\":\"german\"}",
    ))
    .await;
    let hint = SpeechHint::from_terms(Some("de".to_string()), ["NXS", "Dispatcher"]);

    let transcript = speech(&stub, "sk-geheim-1234")
        .transcribe(clip(), &hint)
        .await
        .expect("Erkennung");

    assert_eq!(transcript.text, "Hallo Nexus.");
    assert_eq!(transcript.language.as_deref(), Some("german"));
    let received = stub.first();
    assert_eq!(
        received.fields.get("model").map(String::as_str),
        Some("whisper-1")
    );
    assert_eq!(
        received.fields.get("language").map(String::as_str),
        Some("de")
    );
    assert_eq!(
        received.fields.get("prompt").map(String::as_str),
        Some("NXS, Dispatcher")
    );
    assert_eq!(
        received.fields.get("response_format").map(String::as_str),
        Some("json")
    );
    assert_eq!(received.file_name.as_deref(), Some("aufnahme.ogg"));
    assert_eq!(received.file_mime.as_deref(), Some("audio/ogg"));
    assert_eq!(received.file_bytes, b"OggS-Attrappe".len());
    assert_eq!(
        received.headers.get("authorization").map(String::as_str),
        Some("Bearer sk-geheim-1234")
    );
}

#[tokio::test]
async fn ohne_schlussel_geht_kein_authorization_kopf_mit() {
    let stub = SpeechStub::start(StubReply::Ok("{\"text\":\"ohne Schlüssel\"}")).await;

    let transcript = speech(&stub, "")
        .transcribe(clip(), &SpeechHint::language_only(None))
        .await
        .expect("Erkennung");

    assert_eq!(transcript.text, "ohne Schlüssel");
    // Ohne gemeldete Sprache bleibt das Feld leer
    assert_eq!(transcript.language, None);
    let received = stub.first();
    assert_eq!(received.headers.get("authorization"), None);
    // Ohne Sprache und ohne Begriffe fehlen beide Felder ganz
    assert_eq!(received.fields.get("language"), None);
    assert_eq!(received.fields.get("prompt"), None);
}

#[tokio::test]
async fn lehnt_401_mit_status_und_auszug_ohne_schlussel_im_text_ab() {
    let stub = SpeechStub::start(StubReply::Error(
        StatusCode::UNAUTHORIZED,
        "{\"error\":{\"message\":\"Incorrect API key sk-geheim-1234\"}}",
    ))
    .await;

    let error = speech(&stub, "sk-geheim-1234")
        .transcribe(clip(), &SpeechHint::default())
        .await
        .expect_err("kein Fehler");

    assert_eq!(error.code(), Some("speech-failed"));
    let message = error.to_string();
    assert!(message.contains("401"), "{message}");
    assert!(message.contains("Incorrect API key"), "{message}");
    assert!(!message.contains("sk-geheim-1234"), "{message}");
}

#[tokio::test]
async fn lehnt_413_mit_status_ab() {
    let stub = SpeechStub::start(StubReply::Error(
        StatusCode::PAYLOAD_TOO_LARGE,
        "Maximum content size limit exceeded",
    ))
    .await;

    let error = speech(&stub, "sk-geheim-1234")
        .transcribe(clip(), &SpeechHint::default())
        .await
        .expect_err("kein Fehler");

    assert_eq!(error.code(), Some("speech-failed"));
    let message = error.to_string();
    assert!(message.contains("413"), "{message}");
    assert!(message.contains("Maximum content size"), "{message}");
    assert!(!message.contains("sk-geheim"), "{message}");
}

#[tokio::test]
async fn lehnt_500_mit_status_ab() {
    let stub = SpeechStub::start(StubReply::Error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal server error",
    ))
    .await;

    let error = speech(&stub, "sk-geheim-1234")
        .transcribe(clip(), &SpeechHint::default())
        .await
        .expect_err("kein Fehler");

    assert_eq!(error.code(), Some("speech-failed"));
    let message = error.to_string();
    assert!(message.contains("500"), "{message}");
    assert!(message.contains("internal server error"), "{message}");
    assert!(!message.contains("sk-geheim"), "{message}");
}

#[tokio::test]
async fn nennt_das_zeitlimit_ohne_schlussel_im_text() {
    let stub = SpeechStub::start(StubReply::Slow(Duration::from_secs(30))).await;

    let error = speech(&stub, "sk-geheim-1234")
        .with_timeout(Duration::from_millis(200))
        .transcribe(clip(), &SpeechHint::default())
        .await
        .expect_err("kein Fehler");

    assert_eq!(error.code(), Some("speech-failed"));
    let message = error.to_string();
    assert!(message.contains("nicht innerhalb von 0 s"), "{message}");
    assert!(!message.contains("sk-geheim"), "{message}");
}

#[tokio::test]
async fn lehnt_eine_antwort_ohne_text_ab() {
    let stub = SpeechStub::start(StubReply::Ok("{\"ergebnis\":\"ohne text\"}")).await;

    let error = speech(&stub, "")
        .transcribe(clip(), &SpeechHint::default())
        .await
        .expect_err("kein Fehler");

    assert_eq!(error.code(), Some("speech-failed"));
    assert!(error.to_string().contains("passt nicht"), "{error}");
}

#[tokio::test]
async fn ladt_eine_zu_grosse_aufnahme_nicht_hoch() {
    let stub = SpeechStub::start(StubReply::Ok("{\"text\":\"nie gesendet\"}")).await;
    let clip = AudioClip::new(vec![0; speech::MAX_BYTES + 1], "audio/ogg");

    let error = speech(&stub, "")
        .transcribe(clip, &SpeechHint::default())
        .await
        .expect_err("kein Fehler");

    assert_eq!(error.code(), Some("speech-too-large"));
    assert_eq!(stub.count(), 0, "die Grenze gilt vor dem Upload");
}

#[tokio::test]
async fn ladt_eine_zu_lange_aufnahme_nicht_hoch() {
    let stub = SpeechStub::start(StubReply::Ok("{\"text\":\"nie gesendet\"}")).await;
    let clip = AudioClip::new(b"OggS".to_vec(), "audio/ogg").with_seconds(Some(300.5));

    let error = speech(&stub, "")
        .transcribe(clip, &SpeechHint::default())
        .await
        .expect_err("kein Fehler");

    assert_eq!(error.code(), Some("speech-too-large"));
    assert!(error.to_string().contains("300 s"), "{error}");
    assert_eq!(stub.count(), 0, "die Grenze gilt vor dem Upload");
}

#[tokio::test]
async fn meldet_einen_dienst_ohne_antwort_als_nicht_erreichbar() {
    // Port 9 nimmt nichts an (Discard); die Erkennung kommt nicht zustande
    let error = OpenAiCompatibleSpeech::from_config(&SpeechConfig {
        base_url: "http://127.0.0.1:9".to_string(),
        api_key: String::new(),
        ..SpeechConfig::default()
    })
    .with_timeout(Duration::from_millis(500))
    .transcribe(clip(), &SpeechHint::default())
    .await
    .expect_err("kein Fehler");

    assert_eq!(error.code(), Some("speech-failed"));
}

// ---------------------------------------------------------------------------------------------
// Die Route
// ---------------------------------------------------------------------------------------------

/// Testserver, dessen Einstellung auf die Attrappe zeigt
async fn server_für(stub: Option<&SpeechStub>, backend: SpeechBackend) -> TestServer {
    let server = TestServer::with(TestServerOptions::default());
    let config = TelegramConfig {
        speech: SpeechConfig {
            backend,
            base_url: stub.map(SpeechStub::url).unwrap_or_default(),
            api_key: "sk-geheim-1234".to_string(),
            model: "whisper-1".to_string(),
            language: Some("de".to_string()),
            ..SpeechConfig::default()
        },
        ..TelegramConfig::default()
    };
    TelegramConfigStore::new(server.home.clone())
        .write(&config)
        .expect("telegram.yaml schreiben");
    server
}

#[tokio::test]
async fn die_testroute_erkennt_ohne_korper_das_beispiel_audio() {
    let stub = SpeechStub::start(StubReply::Ok(
        "{\"text\":\"Hallo Nexus, das ist ein Test der Spracherkennung.\",\"language\":\"german\"}",
    ))
    .await;
    let server = server_für(Some(&stub), SpeechBackend::OpenaiCompatible).await;

    let reply = server.post("/api/speech/test", "").await;

    assert_eq!(reply.status, 200);
    let body = reply.body();
    assert_eq!(
        body["text"],
        serde_json::json!("Hallo Nexus, das ist ein Test der Spracherkennung.")
    );
    assert_eq!(body["language"], serde_json::json!("german"));
    assert_eq!(body["backend"], serde_json::json!("openai-compatible"));
    assert!(body["millis"].is_u64(), "{body}");
    // Hochgeladen wurde das mitgelieferte Beispiel
    let received = stub.first();
    assert_eq!(received.file_name.as_deref(), Some("aufnahme.ogg"));
    assert!(received.file_bytes > 1000, "{} Bytes", received.file_bytes);
    assert_eq!(
        received.fields.get("language").map(String::as_str),
        Some("de")
    );
}

#[tokio::test]
async fn die_testroute_erkennt_eine_mitgeschickte_aufnahme() {
    let stub = SpeechStub::start(StubReply::Ok("{\"text\":\"eigene Aufnahme\"}")).await;
    let server = server_für(Some(&stub), SpeechBackend::OpenaiCompatible).await;

    let reply = server
        .post_bytes("/api/speech/test", "audio/ogg", b"OggS-eigene-Aufnahme")
        .await;

    assert_eq!(reply.status, 200);
    assert_eq!(reply.body()["text"], serde_json::json!("eigene Aufnahme"));
    assert_eq!(stub.first().file_bytes, b"OggS-eigene-Aufnahme".len());
}

#[tokio::test]
async fn die_testroute_meldet_ein_abgeschaltetes_backend_mit_400() {
    let server = server_für(None, SpeechBackend::Off).await;

    let reply = server.post("/api/speech/test", "").await;

    assert_eq!(reply.status, 400);
    assert_eq!(reply.code().as_deref(), Some("speech-off"));
    assert_eq!(
        reply.body()["message"],
        serde_json::json!("Spracherkennung ist ausgeschaltet.")
    );
}

#[tokio::test]
async fn die_testroute_meldet_whisper_cli_als_noch_nicht_verfugbar() {
    let server = server_für(None, SpeechBackend::WhisperCli).await;

    let reply = server.post("/api/speech/test", "").await;

    assert_eq!(reply.status, 400);
    assert_eq!(reply.code().as_deref(), Some("speech-off"));
}

#[tokio::test]
async fn die_testroute_meldet_einen_fehler_des_dienstes_mit_502_und_ohne_schlussel() {
    let stub = SpeechStub::start(StubReply::Error(
        StatusCode::UNAUTHORIZED,
        "{\"error\":\"sk-geheim-1234 ist falsch\"}",
    ))
    .await;
    let server = server_für(Some(&stub), SpeechBackend::OpenaiCompatible).await;

    let reply = server.post("/api/speech/test", "").await;

    assert_eq!(reply.status, 502);
    assert_eq!(reply.code().as_deref(), Some("speech-failed"));
    assert!(reply.text.contains("401"), "{}", reply.text);
    assert!(!reply.text.contains("sk-geheim-1234"), "{}", reply.text);
}

#[tokio::test]
async fn die_testroute_lehnt_einen_json_korper_als_aufnahme_ab() {
    let stub = SpeechStub::start(StubReply::Ok("{\"text\":\"nie gesendet\"}")).await;
    let server = server_für(Some(&stub), SpeechBackend::OpenaiCompatible).await;

    let reply = server
        .post("/api/speech/test", "{\"audio\":\"nein\"}")
        .await;

    assert_eq!(reply.status, 400);
    assert_eq!(reply.code().as_deref(), Some("bad-request"));
    assert_eq!(stub.count(), 0);
}

#[tokio::test]
async fn die_testroute_lehnt_eine_aufnahme_uber_zwanzig_megabyte_ab() {
    let stub = SpeechStub::start(StubReply::Ok("{\"text\":\"nie gesendet\"}")).await;
    let server = server_für(Some(&stub), SpeechBackend::OpenaiCompatible).await;

    let reply = server
        .post_bytes(
            "/api/speech/test",
            "audio/ogg",
            &vec![0; speech::MAX_BYTES + 1],
        )
        .await;

    // Der Körper sprengt die Grenze der Route; axum lehnt ihn vor dem Handler ab
    assert_eq!(reply.status, 413);
    assert_eq!(stub.count(), 0);
}

#[tokio::test]
async fn die_testroute_braucht_den_pflichtkopf() {
    let stub = SpeechStub::start(StubReply::Ok("{\"text\":\"nie gesendet\"}")).await;
    let server = server_für(Some(&stub), SpeechBackend::OpenaiCompatible).await;

    let reply = server
        .request("POST", "/api/speech/test", Some(""), &[("x-harness", None)])
        .await;

    assert_eq!(reply.status, 403);
    assert_eq!(reply.code().as_deref(), Some("header-missing"));
    assert_eq!(stub.count(), 0);
}

#[tokio::test]
async fn die_erkennungshilfe_nennt_projekt_board_und_ticketnummern() {
    let stub = SpeechStub::start(StubReply::Ok("{\"text\":\"mit Hilfe\"}")).await;
    let (server, backend) = TestServer::start_with_board_and(TestServerOptions::default()).await;
    TelegramConfigStore::new(server.home.clone())
        .write(&TelegramConfig {
            speech: SpeechConfig {
                base_url: stub.url(),
                api_key: String::new(),
                language: Some("de".to_string()),
                ..SpeechConfig::default()
            },
            ..TelegramConfig::default()
        })
        .expect("telegram.yaml schreiben");
    let services = &backend.services;
    let actor = backend.actor();
    services
        .projects
        .create("NXS", "Nexus", &actor)
        .await
        .expect("Projekt");
    let board = services
        .boards
        .create("NXS", "Kernboard", &actor)
        .await
        .expect("Board");
    services
        .tickets
        .create(
            board.board.id,
            &NewTicketInput::titled("Spracherkennung einbauen"),
            &actor,
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket");

    let reply = server.post("/api/speech/test", "").await;

    assert_eq!(reply.status, 200);
    let prompt = stub
        .first()
        .fields
        .get("prompt")
        .cloned()
        .expect("prompt fehlt");
    assert!(prompt.contains("NXS"), "{prompt}");
    assert!(prompt.contains("Kernboard"), "{prompt}");
    assert!(prompt.contains("NXS-0001"), "{prompt}");
    assert!(prompt.chars().count() <= speech::MAX_HINT_CHARS, "{prompt}");
}
