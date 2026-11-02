//! Portiert aus `packages/cli/tests/unit/DispatchStatusFormatter.test.ts`. Die Sichten entstehen
//! wie im Betrieb aus dem JSON des Servers.

use super::*;

/// Die Antwort, auf der alle Fälle aufbauen (`BASE` in TypeScript)
fn basis() -> Value {
    serde_json::json!({
        "enabled": true,
        "state": "running",
        "message": "läuft · 1/1",
        "pausedUntil": null,
        "wip": 1,
        "active": 1,
        "runs": [{
            "id": "ARC-0009-20261015-090000",
            "kind": "work",
            "ticketId": "ARC-0009",
            "title": "Rabatte",
            "profile": "standard",
            "model": "opus",
            "effort": "high",
            "startedAt": "2026-10-15T07:00:00.000Z",
            "log": "ARC-0009-20261015-090000.log"
        }],
        "merges": [{ "ticketId": "ARC-0003", "phase": "waiting", "branch": "ticket/ARC-0003-x" }],
        "boardId": 1,
        "boardInScope": true,
        "tickets": [
            { "id": "ARC-0010", "title": "Neu", "reason": "bereit – wartet auf freien Platz" },
            { "id": "ARC-0011", "title": "Ohne", "reason": "Kein Plan" }
        ],
        "ticketsError": null,
        "lastTick": "2026-10-15T07:01:00.000Z",
        "lastErrors": ["ARC-0012: claude fehlt"],
        "configFile": "C:/h/dispatch.yaml",
        "configError": null,
        "agentPrefix": "junior-"
    })
}

/// Der Recherche-Lauf aus dem TypeScript-Test
fn recherche_lauf() -> Value {
    serde_json::json!({
        "id": "ARC-F0007-20261015-090000-research",
        "kind": "research",
        "ticketId": "ARC-F0007",
        "questionId": "ARC-F0007",
        "title": "Brutto?",
        "profile": "standard",
        "model": "opus",
        "effort": null,
        "startedAt": "2026-10-15T07:00:00.000Z",
        "log": "ARC-F0007-20261015-090000-research.log"
    })
}

/// Die Basis mit zusätzlichen bzw. ersetzten Feldern
fn mit(felder: Value) -> DispatchStatusView {
    let mut json = basis();
    for (name, value) in felder.as_object().unwrap() {
        json[name] = value.clone();
    }
    DispatchStatusFormatter::view(&json)
}

const F: DispatchStatusFormatter = DispatchStatusFormatter;

#[test]
fn fasst_zustand_laeufe_merges_tickets_und_fehler_zusammen() {
    let lines = F.lines(&mit(serde_json::json!({})));
    assert_eq!(lines[0], "Dispatcher: läuft · 1/1");
    assert!(lines.contains(&"  ARC-0009  Umsetzung, Profil standard (opus/high), Protokoll ARC-0009-20261015-090000.log".to_string()));
    assert!(
        lines.contains(&"  ARC-0003  wartet auf weitere Tickets (ticket/ARC-0003-x)".to_string())
    );
    assert!(lines.contains(&"  ARC-0010  bereit – wartet auf freien Platz".to_string()));
    assert!(lines.contains(&"  ARC-0011  Kein Plan".to_string()));
    assert!(lines.contains(&"  ARC-0012: claude fehlt".to_string()));
    assert_eq!(lines.last().unwrap(), "Einstellung: C:/h/dispatch.yaml");
}

#[test]
fn nennt_die_einstellung_mit_einheitlichen_trennzeichen() {
    let lines = F.lines(&mit(
        serde_json::json!({ "configFile": "C:/h\\dispatch.yaml" }),
    ));
    assert_eq!(lines.last().unwrap(), "Einstellung: C:/h/dispatch.yaml");
}

#[test]
fn merge_nennt_das_ergebnis_fuer_das_ticket_statt_der_uebersicht() {
    let gemergt = mit(serde_json::json!({
        "result": { "outcome": "merged", "text": "ARC-0012 gemergt in main (abc1234)." }
    }));
    assert_eq!(
        F.merge_lines(&gemergt),
        vec!["ARC-0012 gemergt in main (abc1234).".to_string()]
    );
    let blockiert = mit(serde_json::json!({
        "result": { "outcome": "blocked", "text": "ARC-0012: Merge blockiert – lokale Änderungen im Hauptordner: a.txt" }
    }));
    assert_eq!(
        F.merge_lines(&blockiert),
        vec!["ARC-0012: Merge blockiert – lokale Änderungen im Hauptordner: a.txt".to_string()]
    );
    // Ohne Ergebnis (älterer Server) kommt die Übersicht
    assert_eq!(
        F.merge_lines(&mit(serde_json::json!({})))[0],
        "Dispatcher: läuft · 1/1"
    );
}

#[test]
fn zeigt_recherche_laeufe_und_meldet_den_start_einer_recherche() {
    let mut runs = basis()["runs"].as_array().unwrap().clone();
    runs.push(recherche_lauf());
    let lines = F.lines(&mit(serde_json::json!({
        "runs": runs,
        "research": { "auto": true, "wip": 1, "active": 1 }
    })));
    assert!(lines.contains(&"  Recherche ARC-F0007, Profil standard (opus/Standard), Protokoll ARC-F0007-20261015-090000-research.log".to_string()));
    assert!(lines.contains(&"Recherchen: 1/1 laufen, Automatik an".to_string()));

    let failing = F.lines(&mit(serde_json::json!({
        "research": { "auto": true, "wip": 1, "active": 0, "maxPerDay": 10, "today": 3, "retryAt": "2026-10-15T10:30:00.000Z", "error": "Git Bash nicht gefunden" }
    })));
    assert!(failing
        .contains(&"Recherchen: 0/1 laufen, Automatik an, heute 3/10 automatisch".to_string()));
    assert!(failing.contains(
        &"  Automatik-Fehler (Neuversuch ab 2026-10-15T10:30:00.000Z): Git Bash nicht gefunden"
            .to_string()
    ));

    let gestartet = mit(serde_json::json!({ "started": recherche_lauf() }));
    assert_eq!(
        F.research_lines(&gestartet)[0],
        "Recherche ARC-F0007 gestartet – Profil standard (opus/Standard), Protokoll ARC-F0007-20261015-090000-research.log."
    );
}

#[test]
fn zeigt_den_zustand_der_planungs_automatik_samt_fehler() {
    let aus = F.lines(&mit(serde_json::json!({
        "planningAuto": { "auto": false, "wip": 1, "active": 0 }
    })));
    assert!(aus.contains(&"Planungen: 0/1 laufen, Automatik aus".to_string()));

    let failing = F.lines(&mit(serde_json::json!({
        "planningAuto": { "auto": true, "wip": 2, "active": 1, "retryAt": "2026-10-15T10:30:00.000Z", "error": "Festplatte voll" }
    })));
    assert!(failing.contains(&"Planungen: 1/2 laufen, Automatik an".to_string()));
    assert!(failing.contains(
        &"  Automatik-Fehler (Neuversuch ab 2026-10-15T10:30:00.000Z): Festplatte voll".to_string()
    ));

    // Ohne das Feld (älterer Server) bleibt die Zeile weg
    assert!(!F
        .lines(&mit(serde_json::json!({})))
        .iter()
        .any(|line| line.starts_with("Planungen:")));
}

#[test]
fn nennt_bei_merges_den_zielbranch_nach_dem_branch() {
    let lines = F.lines(&mit(serde_json::json!({
        "merges": [
            { "ticketId": "ARC-0003", "phase": "pending", "branch": "ticket/ARC-0003-x", "target": "release/1" },
            { "ticketId": "ARC-0004", "phase": "session", "branch": "ticket/ARC-0004-y", "target": null }
        ]
    })));
    assert!(
        lines.contains(&"  ARC-0003  Merge steht an (ticket/ARC-0003-x → release/1)".to_string())
    );
    assert!(lines.contains(&"  ARC-0004  Merge-Session läuft (ticket/ARC-0004-y)".to_string()));
}

#[test]
fn nennt_ein_board_ausserhalb_der_auswahl_und_leere_listen_knapp() {
    let lines = F.lines(&mit(serde_json::json!({
        "runs": [], "merges": [], "tickets": [], "lastErrors": [], "boardInScope": false
    })));
    assert!(lines.contains(&"Board 1 gehört nicht zu den Boards des Dispatchers.".to_string()));
    assert!(lines.contains(&"Keine Tickets auf Geplant.".to_string()));
    assert!(lines.contains(&"  keine".to_string()));
}

#[test]
fn meldet_den_start_einer_planung() {
    let gestartet = mit(serde_json::json!({
        "started": {
            "id": "ARC-0003-plan", "kind": "plan", "ticketId": "ARC-0003", "title": "Suche",
            "profile": "standard", "model": "opus", "effort": "high",
            "startedAt": "2026-10-15T07:00:00.000Z", "log": "ARC-0003-plan.log"
        }
    }));
    let lines = F.plan_lines(&gestartet);
    assert_eq!(
        lines[0],
        "Planung ARC-0003 gestartet – Profil standard (opus/high), Protokoll ARC-0003-plan.log."
    );
    assert!(lines[1].starts_with("Der Plan erscheint am Ticket"));
}

#[test]
fn eine_fehlerhafte_einstellung_steht_direkt_unter_dem_zustand() {
    let lines = F.lines(&mit(
        serde_json::json!({ "configError": "Zeile 3: kein YAML" }),
    ));
    assert_eq!(lines[1], "Einstellung fehlerhaft: Zeile 3: kein YAML");
}
