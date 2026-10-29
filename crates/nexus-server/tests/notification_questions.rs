//! Meldungen zu neuen Fragen und Recherchen von Agenten; portiert aus
//! `packages/server/tests/unit/NotificationQuestions.test.ts`.

mod common;

use common::notify::{service, ServiceParts};
use nexus_core::actors::AuthorKind;
use nexus_core::board::question_types::{
    BoardQuestionOption, NotifiableBoardQuestion, NotifiableOrigin, NotifiableResearch,
};
use nexus_core::settings::{NotificationMode, UserSettings};
use nexus_server::notifications::model::{ToastKind, ToastOptions};
use nexus_server::notifications::question_changes::QuestionChangeKind;
use nexus_server::notifications::texts::{QuestionInput, ToastTexts};
use nexus_server::notifications::toast_xml::ToastXmlBuilder;
use nexus_server::notifications::QuestionChange;

/// Eine Option einer Recherche
fn option(id: &str, recommended: bool) -> BoardQuestionOption {
    BoardQuestionOption {
        id: id.to_string(),
        title: id.to_string(),
        text: "t".to_string(),
        pros: Vec::new(),
        cons: Vec::new(),
        recommended,
    }
}

/// Eine Recherche eines Agenten mit diesen Optionen
fn research(kind: AuthorKind, options: Vec<BoardQuestionOption>) -> NotifiableResearch {
    NotifiableResearch { kind, options }
}

/// Eine Frage, wie sie die Board-Datenbank für Meldungen liefert
fn question(
    id: &str,
    kind: AuthorKind,
    research: Vec<NotifiableResearch>,
) -> NotifiableBoardQuestion {
    NotifiableBoardQuestion {
        id: id.to_string(),
        title: "Welcher Speicher?".to_string(),
        occasion: Some("T-0042".to_string()),
        origin: NotifiableOrigin { kind },
        research,
        project_key: "ARC".to_string(),
        project_name: "Arches".to_string(),
        repo_paths: Vec::new(),
    }
}

/// Die Eingabe der Texte zu dieser Frage
fn input<'a>(question: &'a NotifiableBoardQuestion, open_url: &'a str) -> QuestionInput<'a> {
    QuestionInput {
        project_name: "webshop",
        question_id: &question.id,
        question_title: &question.title,
        occasion: question.occasion.as_deref(),
        research_count: question.research.len(),
        open_url,
    }
}

// ---------------------------------------------------------------- ToastTexts für Fragen

#[test]
fn formuliert_die_meldung_einer_neuen_frage() {
    let frage = question("F-0007", AuthorKind::Session, Vec::new());
    let model = ToastTexts.question_new(&input(&frage, "u"));
    assert_eq!(model.kind, ToastKind::FrageNeu);
    assert_eq!(model.title, "Neue Frage von einer Session");
    assert_eq!(model.line1, "webshop · F-0007 Welcher Speicher?");
    assert_eq!(model.line2, "Anlass: T-0042 · noch keine Recherche");
    assert_eq!(model.open_url, "u");
}

#[test]
fn nennt_bei_einer_neuen_frage_mit_vorhandener_recherche_dass_sie_vorliegt() {
    let frage = question(
        "F-0007",
        AuthorKind::Session,
        vec![research(AuthorKind::Session, vec![option("O1", false)])],
    );
    let model = ToastTexts.question_new(&input(&frage, "u"));
    assert_eq!(model.line2, "Anlass: T-0042 · Recherche liegt vor");
}

#[test]
fn formuliert_die_recherche_mit_bereich_und_empfehlung() {
    let frage = question("F-0007", AuthorKind::Session, Vec::new());
    let optionen = vec![option("O3", false), option("O4", true), option("O5", false)];
    let model = ToastTexts.research(&input(&frage, "u"), &optionen);
    assert_eq!(model.kind, ToastKind::Recherche);
    assert_eq!(model.title, "Recherche ist da");
    assert_eq!(model.line2, "3 Optionen (O3 bis O5), O4 empfohlen");
}

#[test]
fn nennt_ohne_optionen_nur_den_ergaenzten_kontext() {
    let frage = question("F-1", AuthorKind::Session, Vec::new());
    let model = ToastTexts.research(&input(&frage, "u"), &[]);
    assert_eq!(model.line2, "Kontext ergänzt");
}

#[test]
fn nennt_eine_einzelne_option_ohne_bereich() {
    let frage = question("F-1", AuthorKind::Session, Vec::new());
    let model = ToastTexts.research(&input(&frage, "u"), &[option("O1", false)]);
    assert_eq!(model.line2, "1 Option (O1)");
}

// ------------------------------------------------------- ToastXmlBuilder für Fragen

#[test]
fn nutzt_das_frage_logo_frage_oeffnen_und_kein_hero() {
    let model = nexus_server::notifications::model::ToastModel {
        kind: ToastKind::FrageNeu,
        title: "t".into(),
        line1: "a".into(),
        line2: "b".into(),
        open_url: "http://x/p/p1/questions/F-0007".into(),
    };
    let out = ToastXmlBuilder.build(
        &model,
        &ToastOptions {
            assets_dir: std::path::PathBuf::from("/a/toast"),
            hero_images: true,
        },
    );
    assert!(out.contains("logo-frage.png"), "{out}");
    assert!(out.contains("content=\"Frage öffnen\""), "{out}");
    assert!(!out.contains("gnorieren"), "{out}");
    assert!(!out.contains("placement=\"hero\""), "{out}");
}

// --------------------------------------- NotificationService::on_question_change

/// Eine erkannte neue Frage im angegebenen Harness-Projekt
fn change(
    project_id: &str,
    project_name: &str,
    question: NotifiableBoardQuestion,
) -> QuestionChange {
    QuestionChange {
        project_id: project_id.to_string(),
        project_name: project_name.to_string(),
        kind: QuestionChangeKind::FrageNeu,
        question,
        route: None,
    }
}

/// Dienst im Modus „windows“
fn windows_service() -> ServiceParts {
    service(UserSettings {
        notifications: NotificationMode::Windows,
        ..UserSettings::default()
    })
}

#[tokio::test]
async fn zeigt_die_meldung_mit_der_adresse_der_frage() {
    let parts = windows_service();
    parts
        .service
        .on_question_change(&change(
            "p1",
            "webshop",
            question("F-0007", AuthorKind::Session, Vec::new()),
        ))
        .await;
    let shown = parts.shown();
    assert_eq!(shown.len(), 1);
    assert!(
        shown[0].contains("http://127.0.0.1:7777/p/p1/questions/F-0007"),
        "{}",
        shown[0]
    );
    assert!(
        shown[0].contains("Neue Frage von einer Session"),
        "{}",
        shown[0]
    );
}

#[tokio::test]
async fn zeigt_im_modus_off_nichts() {
    let parts = service(UserSettings {
        notifications: NotificationMode::Off,
        ..UserSettings::default()
    });
    parts
        .service
        .on_question_change(&change(
            "p1",
            "webshop",
            question("F-0007", AuthorKind::Session, Vec::new()),
        ))
        .await;
    assert!(parts.shown().is_empty());
}

#[tokio::test]
async fn oeffnet_fragen_der_board_datenbank_unter_questions_und_ohne_projekt_die_startseite() {
    let parts = windows_service();
    for (pid, id) in [("p1", "ARC-F0001"), ("", "ARC-F0002")] {
        parts
            .service
            .on_question_change(&QuestionChange {
                route: Some("questions".to_string()),
                ..change(
                    pid,
                    "ARC · Arches",
                    question(id, AuthorKind::Session, Vec::new()),
                )
            })
            .await;
    }
    let shown = parts.shown();
    assert!(
        shown[0].contains("http://127.0.0.1:7777/p/p1/questions/ARC-F0001"),
        "{}",
        shown[0]
    );
    assert!(
        shown[0].contains("ARC · Arches · ARC-F0001 Welcher Speicher?"),
        "{}",
        shown[0]
    );
    assert!(shown[1].contains("http://127.0.0.1:7777"), "{}", shown[1]);
    assert!(!shown[1].contains("/p/"), "{}", shown[1]);
}

#[tokio::test]
async fn ersetzt_meldungen_derselben_frage_ueber_ihre_kennung() {
    let parts = windows_service();
    parts
        .service
        .on_question_change(&change(
            "p1",
            "webshop",
            question("F-0007", AuthorKind::Session, Vec::new()),
        ))
        .await;
    parts
        .service
        .on_question_change(&change(
            "",
            "ARC · Arches",
            question("ARC-F0001", AuthorKind::Session, Vec::new()),
        ))
        .await;
    let tags: Vec<String> = parts
        .notifier
        .shown()
        .into_iter()
        .map(|toast| toast.tag().to_string())
        .collect();
    assert_eq!(tags, ["question-p1-F-0007", "question-db-ARC-F0001"]);
}
