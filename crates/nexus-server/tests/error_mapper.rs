//! Abbildung der Fehler auf Status und JSON; portiert aus
//! `packages/server/tests/unit/ErrorMapper.test.ts`.

use nexus_core::{Error, HarnessError};
use nexus_server::error::{ApiError, ErrorMapper, UNAVAILABLE};

/// Status eines erwartbaren Fehlers mit diesem Code
fn status(code: &str) -> u16 {
    ErrorMapper::map(&HarnessError::new(code, "Meldung").into())
        .status
        .as_u16()
}

#[test]
fn meldet_eine_nicht_verfugbare_board_datenbank_mit_503_samt_code_und_meldung() {
    for code in UNAVAILABLE {
        assert_eq!(status(code), 503, "{code}");
    }
    let mapped =
        ErrorMapper::map(&HarnessError::new("board-db-unreachable", "nicht erreichbar").into());
    assert_eq!(
        mapped.body,
        serde_json::json!({ "code": "board-db-unreachable", "message": "nicht erreichbar" })
    );
}

#[test]
fn ordnet_die_docs_fehler_zu() {
    assert_eq!(status("doc-changed"), 409);
    assert_eq!(status("doc-range-invalid"), 400);
    assert_eq!(status("doc-not-writable"), 422);
    assert_eq!(status("doc-not-found"), 404);
    let mapped = ErrorMapper::map(
        &HarnessError::new("doc-changed", "geändert")
            .with_details(serde_json::json!({ "hash": "abc" }))
            .into(),
    );
    assert_eq!(
        mapped.body,
        serde_json::json!({
            "code": "doc-changed",
            "message": "geändert",
            "details": { "hash": "abc" },
        })
    );
}

#[test]
fn bleibt_bei_404_409_und_422() {
    assert_eq!(status("board-not-found"), 404);
    assert_eq!(status("not-found"), 404);
    assert_eq!(status("conflict"), 409);
    assert_eq!(status("export-target-not-empty"), 409);
    assert_eq!(status("token-invalid"), 403);
    assert_eq!(status("board-config-invalid"), 422);
    assert_eq!(status("title-empty"), 422);
}

#[test]
fn unerwartete_fehler_werden_zu_500_ohne_interna() {
    let mapped = ErrorMapper::map(&Error::unexpected_message("Spalte X fehlt"));
    assert_eq!(mapped.status, 500);
    assert_eq!(
        mapped.body,
        serde_json::json!({ "code": "internal", "message": "Interner Fehler im Server." })
    );
}

#[test]
fn eine_ungultige_anfrage_wird_zu_400_bad_request() {
    let mapped = ErrorMapper::map(&ApiError::bad_request().0);
    assert_eq!(mapped.status, 400);
    assert_eq!(
        mapped.body,
        serde_json::json!({ "code": "bad-request", "message": "Die Anfrage ist ungültig." })
    );
}
