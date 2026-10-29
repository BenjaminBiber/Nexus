//! Portiert aus `packages/core/tests/unit/docs/DocCommentStore.test.ts`.

use super::*;

const SPEC: &str = "docs/superpowers/specs/2026-10-22-login-design.md";
const PLAN: &str = "docs/superpowers/plans/2026-10-22-login.md";

/// Die Angaben, aus denen die Tests ihre Kommentare bauen (`NEW` in TypeScript)
fn neu() -> NewDocComment {
    NewDocComment {
        text: "Bitte präzisieren".into(),
        quote: "Das Ticket".into(),
        heading: Some("Ziel".into()),
        lines: Some([3, 3]),
    }
}

/// Projektordner mit den Ordnern der Specs, Pläne und ADRs
fn projekt() -> (tempfile::TempDir, DocCommentStore) {
    let dir = tempfile::Builder::new()
        .prefix("harness-comments-")
        .tempdir()
        .unwrap();
    for sub in [
        "docs/superpowers/specs",
        "docs/superpowers/plans",
        "docs/adr",
    ] {
        std::fs::create_dir_all(dir.path().join(sub)).unwrap();
    }
    let store = DocCommentStore::new(dir.path());
    (dir, store)
}

#[test]
fn leitet_den_sidecar_namen_ab() {
    assert_eq!(
        DocCommentStore::sidecar_of(SPEC).unwrap(),
        "docs/superpowers/specs/2026-10-22-login-design.comments.json"
    );
    assert_eq!(
        DocCommentStore::sidecar_of("GLOSSARY.md").unwrap(),
        "GLOSSARY.comments.json"
    );
}

#[test]
fn liefert_ohne_datei_eine_leere_liste_und_legt_beim_hinzufuegen_die_datei_an() {
    let (dir, store) = projekt();
    assert_eq!(store.list(SPEC).unwrap(), Vec::new());

    let added = store.add(SPEC, &neu()).unwrap();
    assert_eq!(added.status, DocCommentStatus::Open);
    assert_eq!(added.resolution, None);
    assert_eq!(added.text, neu().text);
    assert_eq!(added.lines, Some([3, 3]));
    assert!(
        added.id.starts_with("K-")
            && added.id.len() == 10
            && added.id[2..].chars().all(|c| c.is_ascii_hexdigit()),
        "{}",
        added.id
    );

    let raw = std::fs::read_to_string(
        dir.path()
            .join("docs/superpowers/specs/2026-10-22-login-design.comments.json"),
    )
    .unwrap();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(json["comments"].as_array().unwrap().len(), 1);
    // Dateiformat wie in TypeScript: zwei Leerzeichen Einrückung und ein `\n` am Ende
    assert!(raw.contains("\n  \"comments\""), "{raw}");
    assert!(raw.ends_with("}\n"), "{raw}");
}

#[test]
fn aendert_text_status_und_antwort() {
    let (_dir, store) = projekt();
    let id = store.add("GLOSSARY.md", &neu()).unwrap().id;
    let changed = store
        .update(
            "GLOSSARY.md",
            &id,
            &DocCommentPatch {
                status: Some(DocCommentStatus::Resolved),
                resolution: Some(Some("Ergänzt in Abschnitt Ziel".into())),
                ..DocCommentPatch::default()
            },
        )
        .unwrap();
    assert_eq!(changed.status, DocCommentStatus::Resolved);
    assert_eq!(
        changed.resolution.as_deref(),
        Some("Ergänzt in Abschnitt Ziel")
    );
    assert_eq!(changed.text, neu().text);
}

#[test]
fn entfernt_einen_kommentar_und_meldet_unbekannte_ids() {
    let (_dir, store) = projekt();
    let id = store.add("GLOSSARY.md", &neu()).unwrap().id;
    store.remove("GLOSSARY.md", &id).unwrap();
    assert_eq!(store.list("GLOSSARY.md").unwrap(), Vec::new());

    let error = store
        .update(
            "GLOSSARY.md",
            "K-00000000",
            &DocCommentPatch {
                text: Some("x".into()),
                ..DocCommentPatch::default()
            },
        )
        .unwrap_err();
    assert_eq!(error.code(), Some("comment-not-found"));
}

#[test]
fn listet_ohne_scope_dokumente_glossar_und_adrs_und_laesst_erledigte_weg() {
    let (_dir, store) = projekt();
    store.add(SPEC, &neu()).unwrap();
    store.add("GLOSSARY.md", &neu()).unwrap();
    store.add("docs/adr/0001-eins.md", &neu()).unwrap();
    let done = store.add(PLAN, &neu()).unwrap();
    store
        .update(
            PLAN,
            &done.id,
            &DocCommentPatch {
                status: Some(DocCommentStatus::Resolved),
                ..DocCommentPatch::default()
            },
        )
        .unwrap();

    let mut pfade: Vec<String> = store
        .list_open(None)
        .unwrap()
        .into_iter()
        .map(|o| o.path)
        .collect();
    pfade.sort();
    assert_eq!(
        pfade,
        vec![
            "GLOSSARY.md".to_string(),
            "docs/adr/0001-eins.md".to_string(),
            SPEC.to_string()
        ]
    );
}

#[test]
fn nimmt_als_scope_ein_thema_oder_genau_einen_dokumentpfad() {
    let (_dir, store) = projekt();
    store.add(SPEC, &neu()).unwrap();
    store.add(PLAN, &neu()).unwrap();
    store
        .add("docs/superpowers/specs/2026-10-20-export-design.md", &neu())
        .unwrap();
    store.add("GLOSSARY.md", &neu()).unwrap();

    let mut thema: Vec<String> = store
        .list_open(Some("login"))
        .unwrap()
        .into_iter()
        .map(|o| o.path)
        .collect();
    thema.sort();
    assert_eq!(thema, vec![PLAN.to_string(), SPEC.to_string()]);

    let dokument: Vec<String> = store
        .list_open(Some(SPEC))
        .unwrap()
        .into_iter()
        .map(|o| o.path)
        .collect();
    assert_eq!(dokument, vec![SPEC.to_string()]);
    assert!(store.list_open(Some("gibt-es-nicht")).unwrap().is_empty());
}

#[test]
fn erledigt_einen_kommentar_ueber_die_id_ohne_das_dokument_zu_kennen() {
    let (_dir, store) = projekt();
    let id = store.add(PLAN, &neu()).unwrap().id;
    let resolved = store.resolve(&id, "Schritt 3 ergänzt").unwrap();
    assert_eq!(resolved.path, PLAN);
    assert_eq!(resolved.comment.status, DocCommentStatus::Resolved);
    assert_eq!(
        resolved.comment.resolution.as_deref(),
        Some("Schritt 3 ergänzt")
    );
}

#[test]
fn meldet_eine_beschaedigte_sidecar_datei_verstaendlich() {
    let (dir, store) = projekt();
    std::fs::write(dir.path().join("GLOSSARY.comments.json"), "{kaputt").unwrap();
    let error = store.list("GLOSSARY.md").unwrap_err();
    assert_eq!(error.code(), Some("comments-corrupt"));
    assert_eq!(
        error.to_string(),
        "Die Kommentardatei GLOSSARY.comments.json ist beschädigt."
    );
}

#[test]
fn lehnt_pfade_ohne_md_oder_ausserhalb_des_projekts_ab_und_legt_nichts_an() {
    let (dir, store) = projekt();
    assert_eq!(
        store.list("README.txt").unwrap_err().code(),
        Some("doc-not-found")
    );
    for bad in [
        "../x.md",
        "/abs.md",
        "docs/superpowers/specs/2026-10-22-login-design.txt",
        "C:/x.md",
    ] {
        assert_eq!(
            store.add(bad, &neu()).unwrap_err().code(),
            Some("doc-not-found"),
            "{bad}"
        );
    }
    assert!(!dir
        .path()
        .join("docs/superpowers/specs/2026-10-22-login-design.txt")
        .exists());
    assert!(!dir.path().join("x.comments.json").exists());
}

#[test]
fn lehnt_pfade_mit_backslash_ab_und_schreibt_nie_ausserhalb_des_projekts() {
    let (dir, store) = projekt();
    for bad in ["docs\\..\\..\\x.md", "..\\x.md", "\\\\server\\share\\x.md"] {
        assert_eq!(
            store.add(bad, &neu()).unwrap_err().code(),
            Some("doc-not-found"),
            "{bad}"
        );
    }
    assert!(!dir
        .path()
        .parent()
        .unwrap()
        .join("x.comments.json")
        .exists());
    assert!(!dir.path().join("x.comments.json").exists());
}

#[test]
fn ignoriert_fremde_comments_dateien_im_projektstamm() {
    let (_dir, store) = projekt();
    store.add("GLOSSARY.md", &neu()).unwrap();
    store.add("README.md", &neu()).unwrap();
    let pfade: Vec<String> = store
        .list_open(None)
        .unwrap()
        .into_iter()
        .map(|o| o.path)
        .collect();
    assert_eq!(pfade, vec!["GLOSSARY.md".to_string()]);
}

#[test]
fn meldet_beim_erledigen_einer_unbekannten_id_comment_not_found() {
    let (_dir, store) = projekt();
    let error = store.resolve("K-00000000", "x").unwrap_err();
    assert_eq!(error.code(), Some("comment-not-found"));
    assert_eq!(error.to_string(), "Kommentar K-00000000 gibt es nicht.");
}

#[test]
fn behandelt_json_ohne_comments_array_als_leer() {
    let (dir, store) = projekt();
    std::fs::write(dir.path().join("GLOSSARY.comments.json"), "{}").unwrap();
    assert_eq!(store.list("GLOSSARY.md").unwrap(), Vec::new());
}

#[test]
fn ignoriert_nicht_gesetzte_werte_im_patch() {
    let (_dir, store) = projekt();
    let id = store.add("GLOSSARY.md", &neu()).unwrap().id;
    let changed = store
        .update(
            "GLOSSARY.md",
            &id,
            &DocCommentPatch {
                status: Some(DocCommentStatus::Resolved),
                ..DocCommentPatch::default()
            },
        )
        .unwrap();
    assert_eq!(changed.text, neu().text);
    assert_eq!(changed.status, DocCommentStatus::Resolved);
}

#[test]
fn ein_thema_entsteht_aus_dem_dateinamen_ohne_datum_und_ohne_design() {
    assert_eq!(topic_of(SPEC), "login");
    assert_eq!(topic_of(PLAN), "login");
    assert_eq!(topic_of("GLOSSARY.md"), "GLOSSARY");
    assert_eq!(topic_of("docs/adr/0001-speicher.md"), "0001-speicher");
}
