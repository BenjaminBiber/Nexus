//! HTTP/1.1 über eine einzelne Verbindung zu `127.0.0.1`: ein Aufruf, eine Verbindung,
//! `Connection: close`. Das reicht für `GET /api/health` und `POST /api/projects` und
//! ersetzt `fetch` aus dem TypeScript-CLI.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// Kopfzeile, mit der sich die eigenen Werkzeuge beim Server ausweisen (`RequestGuard`)
const HARNESS_HEADER: &str = "x-harness";

/// Antwort des Servers: Statuscode und Rumpf als Text
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalResponse {
    pub status: u16,
    pub body: String,
}

impl LocalResponse {
    /// Status im Bereich 200–299, wie `response.ok` in Node
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Rumpf als JSON, falls er welches ist
    pub fn json(&self) -> Option<serde_json::Value> {
        serde_json::from_str(&self.body).ok()
    }
}

/// Ruft den lokalen Server auf; `timeout` gilt für Verbindung, Senden und Lesen.
#[derive(Debug, Clone, Copy)]
pub struct LocalHttp {
    timeout: Duration,
}

impl LocalHttp {
    /// Client mit der angegebenen Wartezeit
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// `GET <path>` auf dem Port
    pub fn get(&self, port: u16, path: &str) -> io::Result<LocalResponse> {
        self.request(port, "GET", path, None, &[])
    }

    /// `POST <path>` mit JSON-Rumpf auf dem Port
    pub fn post_json(&self, port: u16, path: &str, body: &str) -> io::Result<LocalResponse> {
        self.request(port, "POST", path, Some(body), &[])
    }

    /// Wie [`LocalHttp::post_json`], dazu eigene Kopfzeilen (z. B. das Token eines Projekts)
    pub fn post_json_with(
        &self,
        port: u16,
        path: &str,
        body: &str,
        headers: &[(&str, &str)],
    ) -> io::Result<LocalResponse> {
        self.request(port, "POST", path, Some(body), headers)
    }

    /// Schickt die Anfrage und liest die ganze Antwort; der Server schließt danach
    fn request(
        &self,
        port: u16,
        method: &str,
        path: &str,
        body: Option<&str>,
        headers: &[(&str, &str)],
    ) -> io::Result<LocalResponse> {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let mut stream = TcpStream::connect_timeout(&address, self.timeout)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;
        stream.write_all(&Self::head(port, method, path, body, headers))?;
        if let Some(body) = body {
            stream.write_all(body.as_bytes())?;
        }
        stream.flush()?;
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw)?;
        Self::parse(&raw)
    }

    /// Anfragezeile und Kopfzeilen
    fn head(
        port: u16,
        method: &str,
        path: &str,
        body: Option<&str>,
        headers: &[(&str, &str)],
    ) -> Vec<u8> {
        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{HARNESS_HEADER}: 1\r\nAccept: application/json\r\nConnection: close\r\n"
        );
        for (name, value) in headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        if let Some(body) = body {
            head.push_str("content-type: application/json\r\n");
            head.push_str(&format!("content-length: {}\r\n", body.len()));
        }
        head.push_str("\r\n");
        head.into_bytes()
    }

    /// Trennt Statuszeile, Kopfzeilen und Rumpf; ein zerstückelter Rumpf wird zusammengesetzt
    fn parse(raw: &[u8]) -> io::Result<LocalResponse> {
        let text = String::from_utf8_lossy(raw);
        let (head, body) = text
            .split_once("\r\n\r\n")
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Antwort ohne Kopfende"))?;
        let mut lines = head.split("\r\n");
        let status = lines
            .next()
            .and_then(|line| line.split(' ').nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Antwort ohne Status"))?;
        let chunked = lines.any(|line| {
            let lower = line.to_ascii_lowercase();
            lower.starts_with("transfer-encoding:") && lower.contains("chunked")
        });
        let body = if chunked {
            Self::dechunk(body)
        } else {
            body.to_string()
        };
        Ok(LocalResponse { status, body })
    }

    /// Setzt einen in Stücken übertragenen Rumpf zusammen
    fn dechunk(body: &str) -> String {
        let mut out = String::new();
        let mut rest = body;
        while let Some((head, tail)) = rest.split_once("\r\n") {
            let size = usize::from_str_radix(head.split(';').next().unwrap_or("").trim(), 16);
            let Ok(size) = size else { break };
            if size == 0 || tail.len() < size {
                out.push_str(&tail[..size.min(tail.len())]);
                break;
            }
            out.push_str(&tail[..size]);
            rest = tail[size..].trim_start_matches("\r\n");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liest_status_und_rumpf_mit_laenge() {
        let raw = b"HTTP/1.1 200 OK\r\ncontent-length: 13\r\n\r\n{\"name\":\"ok\"}";
        let response = LocalHttp::parse(raw).unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, "{\"name\":\"ok\"}");
        assert!(response.ok());
    }

    #[test]
    fn setzt_einen_zerstueckelten_rumpf_zusammen() {
        let raw =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n7\r\n{\"a\":1}\r\n0\r\n\r\n";
        let response = LocalHttp::parse(raw).unwrap();
        assert_eq!(response.json(), Some(serde_json::json!({ "a": 1 })));
    }

    #[test]
    fn ein_fehlerstatus_ist_nicht_ok() {
        let raw = b"HTTP/1.1 409 Conflict\r\ncontent-length: 2\r\n\r\n{}";
        let response = LocalHttp::parse(raw).unwrap();
        assert_eq!(response.status, 409);
        assert!(!response.ok());
    }

    #[test]
    fn eine_antwort_ohne_kopfende_ist_ein_fehler() {
        assert!(LocalHttp::parse(b"kein HTTP").is_err());
    }

    #[test]
    fn die_anfrage_nennt_host_und_kopfzeile() {
        let head =
            String::from_utf8(LocalHttp::head(4710, "GET", "/api/health", None, &[])).unwrap();
        assert!(head.starts_with("GET /api/health HTTP/1.1\r\n"));
        assert!(head.contains("Host: 127.0.0.1:4710\r\n"));
        assert!(head.contains("x-harness: 1\r\n"));
        assert!(head.ends_with("\r\n\r\n"));
    }

    #[test]
    fn eine_anfrage_mit_rumpf_nennt_laenge_und_inhaltstyp() {
        let head = String::from_utf8(LocalHttp::head(
            4710,
            "POST",
            "/api/projects",
            Some("{\"a\":1}"),
            &[("x-harness-token", "geheim")],
        ))
        .unwrap();
        assert!(head.contains("content-type: application/json\r\n"));
        assert!(head.contains("content-length: 7\r\n"));
        assert!(head.contains("x-harness-token: geheim\r\n"));
    }
}
