//! Tests der Notizen; portiert aus `packages/core/tests/unit/board/NoteService.test.ts`.

mod locking;

use super::{AddNoteOptions, NewNoteText, NoteService, TakeNoteOptions};
use crate::board::basics::{BoardActor, NoteState, Priority};
use crate::board::ticket_service::{NewTicketInput, WriteOptions};
use crate::board::types::Note;
use crate::error::Result;
use crate::testing::BoardFixture;

/// Legt ein Ticket mit diesem Titel auf dem Board „Sprint“ an
async fn ticket(f: &BoardFixture, title: &str) {
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput::titled(title),
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .unwrap();
}

/// Legt eine Notiz mit diesem Titel an (ohne Herkunft und Labels)
async fn note(f: &BoardFixture, title: &str) -> Result<Note> {
    f.notes
        .add(
            f.board_id,
            &NewNoteText::titled(title),
            &f.actor,
            &AddNoteOptions::default(),
        )
        .await
}

/// Die Titel der offenen Notizen des Boards
async fn open_titles(f: &BoardFixture) -> Vec<String> {
    f.notes
        .list(f.board_id, None, None)
        .await
        .unwrap()
        .into_iter()
        .map(|note| note.title)
        .collect()
}

#[tokio::test]
async fn legt_notizen_an_und_listet_standardmassig_nur_offene() {
    let f = BoardFixture::create().await;
    ticket(&f, "A").await;

    let n = f
        .notes
        .add(
            f.board_id,
            &NewNoteText::titled("Cache läuft nie ab"),
            &f.actor,
            &AddNoteOptions {
                source: Some("arc-1".into()),
                labels: Some(vec!["cache".into()]),
                force: false,
            },
        )
        .await
        .unwrap();

    assert_eq!(n.state, NoteState::Open);
    assert_eq!(n.source.as_deref(), Some("ARC-0001"));
    assert_eq!(n.labels, ["cache"]);
    assert_eq!(n.author_name, "claude");

    note(&f, "Zweite").await.unwrap();
    let zweite = f.notes.list(f.board_id, None, None).await.unwrap()[1].id;
    f.notes.drop_note(zweite, "erledigt sich").await.unwrap();

    assert_eq!(open_titles(&f).await, ["Cache läuft nie ab"]);
    assert_eq!(
        f.notes
            .list(
                f.board_id,
                Some(&[NoteState::Open, NoteState::Taken, NoteState::Dropped]),
                None
            )
            .await
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn lehnt_eine_unbekannte_herkunft_einen_leeren_mehrzeiligen_oder_zu_langen_titel_ab() {
    let f = BoardFixture::create().await;

    assert_eq!(
        f.notes
            .add(
                f.board_id,
                &NewNoteText::titled("x"),
                &f.actor,
                &AddNoteOptions {
                    source: Some("ARC-0009".into()),
                    ..AddNoteOptions::default()
                }
            )
            .await
            .unwrap_err()
            .code(),
        Some("ticket-not-found")
    );
    assert_eq!(
        f.notes
            .add(
                f.board_id,
                &NewNoteText {
                    title: " ".into(),
                    body: Some("nur Details".into()),
                },
                &f.actor,
                &AddNoteOptions::default()
            )
            .await
            .unwrap_err()
            .code(),
        Some("note-empty")
    );
    assert_eq!(
        note(&f, "Zeile eins\nZeile zwei").await.unwrap_err().code(),
        Some("note-title-multiline")
    );
    let error = note(&f, &"t".repeat(201)).await.unwrap_err();
    assert_eq!(error.code(), Some("value-too-long"));
    assert!(error.to_string().contains("--details"));
}

#[tokio::test]
async fn speichert_titel_und_details_getrennt_getrimmt() {
    let f = BoardFixture::create().await;

    let n = f
        .notes
        .add(
            f.board_id,
            &NewNoteText {
                title: "  Cache läuft nie ab ".into(),
                body: Some("\nBoardViewService ohne TTL\n".into()),
            },
            &f.actor,
            &AddNoteOptions::default(),
        )
        .await
        .unwrap();

    assert_eq!(n.title, "Cache läuft nie ab");
    assert_eq!(n.body, "BoardViewService ohne TTL");
    assert_eq!(n.seen_count, 1);
    assert!(n.seen_in.is_empty());
    let stored = f.notes.get(n.id).await.unwrap();
    assert_eq!(stored.title, "Cache läuft nie ab");
    assert_eq!(stored.body, "BoardViewService ohne TTL");
}

#[tokio::test]
async fn lehnt_denselben_befund_ab_und_nennt_offene_und_ubernommene_kandidaten() {
    let f = BoardFixture::create().await;
    ticket(&f, "A").await;
    let first = f
        .notes
        .add(
            f.board_id,
            &NewNoteText {
                title: "Core-Unit-Tests scheitern am Windows-Kurzpfad im TEMP".into(),
                body: Some("GitPaths, LongPaths: BENJAM~1.BIB statt Langname".into()),
            },
            &f.actor,
            &AddNoteOptions::default(),
        )
        .await
        .unwrap();
    let again = NewNoteText {
        title: "npm run test:unit: GitPaths und LongPaths rot".into(),
        body: Some("Windows-Kurzpfad BENJAM~1.BIB im TEMP, unabhängig von ARC-0001".into()),
    };

    let error = f
        .notes
        .add(
            f.board_id,
            &again,
            &f.actor,
            &AddNoteOptions {
                source: Some("ARC-0001".into()),
                ..AddNoteOptions::default()
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("note-similar"));
    assert!(error.to_string().contains(&format!(
        "nexus board note seen {} --from ARC-0001",
        first.id
    )));
    assert_eq!(
        error.harness().unwrap().details,
        Some(serde_json::json!({
            "notes": [{
                "id": first.id,
                "title": first.title,
                "state": "open",
                "ticketId": null,
            }],
        }))
    );

    let taken = f
        .notes
        .take(first.id, &f.actor, &TakeNoteOptions::default())
        .await
        .unwrap();
    let error = f
        .notes
        .add(f.board_id, &again, &f.actor, &AddNoteOptions::default())
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("note-similar"));
    assert!(error
        .to_string()
        .contains(&format!("an {} kommentieren", taken.ticket.ticket.id)));

    let forced = f
        .notes
        .add(
            f.board_id,
            &again,
            &f.actor,
            &AddNoteOptions {
                force: true,
                ..AddNoteOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(forced.state, NoteState::Open);
    assert_eq!(forced.seen_count, 1);

    // Etwas anderes geht ohne force durch
    assert_eq!(
        note(&f, "Cache der Board-Ansicht läuft nie ab")
            .await
            .unwrap()
            .state,
        NoteState::Open
    );
}

#[tokio::test]
async fn zahlt_einen_erneut_gesehenen_befund_hoch_und_merkt_sich_jedes_ticket_einmal() {
    let f = BoardFixture::create().await;
    for title in ["A", "B", "C"] {
        ticket(&f, title).await;
    }
    let n = f
        .notes
        .add(
            f.board_id,
            &NewNoteText::titled("Befund"),
            &f.actor,
            &AddNoteOptions {
                source: Some("ARC-0001".into()),
                ..AddNoteOptions::default()
            },
        )
        .await
        .unwrap();

    for source in [Some("ARC-0002"), Some("arc-2"), Some("ARC-0001")] {
        f.notes.seen(n.id, source).await.unwrap();
    }
    let seen = f.notes.seen(n.id, None).await.unwrap();

    assert_eq!(seen.seen_count, 5);
    assert_eq!(seen.seen_in, ["ARC-0002"]);
    let stored = f.notes.get(n.id).await.unwrap();
    assert_eq!(stored.seen_count, 5);
    assert_eq!(stored.seen_in, ["ARC-0002"]);

    let taken = f
        .notes
        .take(n.id, &f.actor, &TakeNoteOptions::default())
        .await
        .unwrap();
    assert_eq!(
        taken.ticket.ticket.description,
        "(Notiz vom 2026-10-13T10:00:00.000Z, aufgefallen bei ARC-0001; insgesamt 5× aufgefallen, auch bei ARC-0002)"
    );

    let error = f.notes.seen(n.id, None).await.unwrap_err();
    assert_eq!(error.code(), Some("note-taken"));
    assert!(error.to_string().contains(&taken.ticket.ticket.id));
    assert_eq!(
        f.notes.seen(999, None).await.unwrap_err().code(),
        Some("note-not-found")
    );
}

#[tokio::test]
async fn sucht_ahnliche_offene_und_ubernommene_notizen_ahnlichste_zuerst() {
    let f = BoardFixture::create().await;
    let a = f
        .notes
        .add(
            f.board_id,
            &NewNoteText {
                title: "Dispatcher startet Worktree doppelt".into(),
                body: Some("WorktreeManager legt Branch zweimal an".into()),
            },
            &f.actor,
            &AddNoteOptions::default(),
        )
        .await
        .unwrap();
    note(&f, "Cache der Board-Ansicht läuft nie ab")
        .await
        .unwrap();
    let dropped = f
        .notes
        .add(
            f.board_id,
            &NewNoteText::titled("Worktree Branch doppelt angelegt vom Dispatcher"),
            &f.actor,
            &AddNoteOptions {
                force: true,
                ..AddNoteOptions::default()
            },
        )
        .await
        .unwrap();
    f.notes.drop_note(dropped.id, "doppelt").await.unwrap();

    assert_eq!(
        f.notes
            .similar(f.board_id, "WorktreeManager: Branch doppelt, Dispatcher")
            .await
            .unwrap()
            .into_iter()
            .map(|note| note.id)
            .collect::<Vec<_>>(),
        [a.id]
    );
}

#[tokio::test]
async fn halt_eine_kurze_notiz_nicht_fur_dublette_einer_langen() {
    let f = BoardFixture::create().await;
    f.notes
        .add(
            f.board_id,
            &NewNoteText {
                title: "Dispatcher räumt Worktree nach Abbruch nicht auf".into(),
                body: Some(
                    "Nach einem abgebrochenen Lauf bleibt der Branch samt Ordner liegen; \
                     WorktreeManager prüft beim nächsten Start nur den Pfad, nicht den Branch. \
                     Folge: zweiter Lauf scheitert mit „already exists“, Aufräumen per Hand nötig."
                        .into(),
                ),
            },
            &f.actor,
            &AddNoteOptions::default(),
        )
        .await
        .unwrap();

    assert!(f
        .notes
        .similar(f.board_id, "Dispatcher zeigt Branch des Worktree falsch an")
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        note(&f, "Dispatcher zeigt Branch des Worktree falsch an")
            .await
            .unwrap()
            .state,
        NoteState::Open
    );
}

#[tokio::test]
async fn lehnt_eine_herkunft_aus_einem_anderen_projekt_ab() {
    let f = BoardFixture::create().await;
    f.projects.create("AIH", "Harness", &f.actor).await.unwrap();
    let aih = f.boards.create("AIH", "Harness", &f.actor).await.unwrap();
    f.tickets
        .create(
            aih.board.id,
            &NewTicketInput::titled("A"),
            &f.actor,
            &WriteOptions::default(),
        )
        .await
        .unwrap();
    let von_aih = AddNoteOptions {
        source: Some("AIH-0001".into()),
        ..AddNoteOptions::default()
    };

    assert_eq!(
        f.notes
            .add(f.board_id, &NewNoteText::titled("x"), &f.actor, &von_aih)
            .await
            .unwrap_err()
            .code(),
        Some("note-source-other-project")
    );
    let n = f
        .notes
        .add(aih.board.id, &NewNoteText::titled("x"), &f.actor, &von_aih)
        .await
        .unwrap();
    assert_eq!(n.board_id, aih.board.id);
    assert_eq!(n.source.as_deref(), Some("AIH-0001"));
}

#[tokio::test]
async fn macht_aus_einer_notiz_ein_ticket_mit_herkunftsvermerk() {
    let f = BoardFixture::create().await;
    ticket(&f, "A").await;
    let n = f
        .notes
        .add(
            f.board_id,
            &NewNoteText {
                title: "Cache prüfen".into(),
                body: Some("Details".into()),
            },
            &f.actor,
            &AddNoteOptions {
                source: Some("ARC-0001".into()),
                labels: Some(vec!["cache".into()]),
                force: false,
            },
        )
        .await
        .unwrap();

    let taken = f
        .notes
        .take(
            n.id,
            &f.actor,
            &TakeNoteOptions {
                priority: Some(Priority::High),
                ..TakeNoteOptions::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(taken.ticket.ticket.id, "ARC-0002");
    assert_eq!(taken.ticket.ticket.title, "Cache prüfen");
    assert_eq!(taken.ticket.ticket.priority, Priority::High);
    assert_eq!(taken.ticket.ticket.labels, ["cache"]);
    assert_eq!(
        taken.ticket.ticket.description,
        "Details\n\n(Notiz vom 2026-10-13T10:00:00.000Z, aufgefallen bei ARC-0001)"
    );
    assert_eq!(taken.note.state, NoteState::Taken);
    assert_eq!(taken.note.ticket_id.as_deref(), Some("ARC-0002"));

    let error = f
        .notes
        .take(n.id, &f.actor, &TakeNoteOptions::default())
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("note-not-open"));
    assert!(error.to_string().contains("ARC-0002"));
}

#[tokio::test]
async fn ubernimmt_eine_notiz_auch_wenn_ein_offenes_ticket_denselben_titel_hat() {
    let f = BoardFixture::create().await;
    ticket(&f, "Cache prüfen").await;
    let n = note(&f, "Cache prüfen").await.unwrap();

    let taken = f
        .notes
        .take(n.id, &f.actor, &TakeNoteOptions::default())
        .await
        .unwrap();

    assert_eq!(taken.ticket.ticket.id, "ARC-0002");
    assert_eq!(taken.ticket.ticket.title, "Cache prüfen");
}

#[tokio::test]
async fn verwirft_nur_mit_grund_und_nicht_nach_der_ubernahme() {
    let f = BoardFixture::create().await;
    let n = note(&f, "x").await.unwrap();

    assert_eq!(
        f.notes.drop_note(n.id, " ").await.unwrap_err().code(),
        Some("reason-missing")
    );
    f.notes
        .take(n.id, &f.actor, &TakeNoteOptions::default())
        .await
        .unwrap();
    assert_eq!(
        f.notes.drop_note(n.id, "egal").await.unwrap_err().code(),
        Some("note-taken")
    );
    assert_eq!(
        f.notes.get(999).await.unwrap_err().code(),
        Some("note-not-found")
    );
}

#[tokio::test]
async fn entfernt_doppelte_labels_ohne_rucksicht_auf_schreibweise_und_pruft_langen() {
    let f = BoardFixture::create().await;

    let n = f
        .notes
        .add(
            f.board_id,
            &NewNoteText::titled("A"),
            &f.actor,
            &AddNoteOptions {
                labels: Some(
                    ["Cache", "cache, UI", "ui"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                ),
                ..AddNoteOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(n.labels, ["Cache", "UI"]);

    assert_eq!(
        f.notes
            .add(
                f.board_id,
                &NewNoteText::titled("B"),
                &f.actor,
                &AddNoteOptions {
                    labels: Some(vec!["l".repeat(101)]),
                    ..AddNoteOptions::default()
                }
            )
            .await
            .unwrap_err()
            .code(),
        Some("value-too-long")
    );

    // Zwölf Labels à 90 Zeichen sprengen die Spalte der Label-Liste
    let many: Vec<String> = (0..12).map(|i| format!("{i}{}", "x".repeat(89))).collect();
    let error = f
        .notes
        .add(
            f.board_id,
            &NewNoteText::titled("C"),
            &f.actor,
            &AddNoteOptions {
                labels: Some(many),
                ..AddNoteOptions::default()
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("value-too-long"));
    assert!(error.to_string().contains("Label-Liste"));

    let zu_langer_name = BoardActor {
        name: "n".repeat(101),
        ..f.actor.clone()
    };
    assert_eq!(
        f.notes
            .add(
                f.board_id,
                &NewNoteText::titled("D"),
                &zu_langer_name,
                &AddNoteOptions::default()
            )
            .await
            .unwrap_err()
            .code(),
        Some("value-too-long")
    );
    assert_eq!(
        f.notes
            .drop_note(n.id, &"g".repeat(1001))
            .await
            .unwrap_err()
            .code(),
        Some("value-too-long")
    );
}

/// Der Dienst nimmt jede Datenbank, nicht nur die der Umgebung
#[tokio::test]
async fn arbeitet_auf_einer_eigenen_datenbank() {
    let f = BoardFixture::create().await;
    let notes = NoteService::new(f.db.clone(), f.clock.clone());

    let n = notes
        .add(
            f.board_id,
            &NewNoteText::titled("Eigene Datenbank"),
            &f.actor,
            &AddNoteOptions::default(),
        )
        .await
        .unwrap();

    assert_eq!(f.notes.get(n.id).await.unwrap().title, "Eigene Datenbank");
}
