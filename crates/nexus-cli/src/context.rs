//! Gemeinsamer Zugriff der Befehle auf globale Optionen, Ausgabe und das geöffnete Projekt.
//! Portiert aus `packages/cli/src/CliContext.ts`.

use std::path::{Path, PathBuf};

use nexus_core::dispatch::config::JoinWorktrees;
use nexus_core::dispatch::config_store::DispatchConfigStore;
use nexus_core::git::{GitCommands, GitRunner};
use nexus_core::{
    Actor, AuthorKind, Error, HarnessError, HarnessHome, HarnessProject, QuestionActor, Result,
};
use nexus_db::{BoardRuntime, ConnectOptions};

use crate::output::Printer;

/// Name, der einspringt, wenn Git keinen Benutzernamen kennt
const UNKNOWN_USER: &str = "unbekannt";

/// Globale Optionen, Ausgabe und Projektzugriff für alle Befehle.
#[derive(Debug, Clone)]
pub struct CliContext {
    json: bool,
    dir: PathBuf,
    session: Option<String>,
}

impl CliContext {
    /// Kontext aus den globalen Optionen; die Session kommt aus `HARNESS_SESSION`
    pub fn new(json: bool, dir: Option<PathBuf>) -> Self {
        Self::with_session(
            json,
            dir,
            std::env::var("HARNESS_SESSION")
                .ok()
                .filter(|id| !id.is_empty()),
        )
    }

    /// Kontext mit ausdrücklich gesetzter Session (Tests)
    pub fn with_session(json: bool, dir: Option<PathBuf>, session: Option<String>) -> Self {
        Self {
            json,
            dir: dir
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            session,
        }
    }

    /// Ausgabe passend zu `--json`
    pub fn printer(&self) -> Printer {
        Printer::new(self.json)
    }

    /// Projektordner aus `--dir` oder dem aktuellen Verzeichnis
    pub fn project_dir(&self) -> &Path {
        &self.dir
    }

    /// Öffnet das eingerichtete Projekt (Fehler mit Hinweis auf `harness init`, wenn nicht eingerichtet)
    pub fn open_project(&self) -> Result<HarnessProject> {
        HarnessProject::open(&self.dir)
    }

    /// Benutzerweiter Harness-Ordner (`HARNESS_HOME` oder `~/.harness`)
    pub fn home(&self) -> HarnessHome {
        HarnessHome::from_process_env()
    }

    /// Worktrees des Dispatchers (`joinWorktrees` aus `~/.harness/dispatch.yaml`); ohne oder
    /// mit kaputter Datei: ein Worktree je Ticket
    pub fn join_worktrees(&self) -> JoinWorktrees {
        DispatchConfigStore::new(self.home())
            .read()
            .map(|read| read.config.join_worktrees)
            .unwrap_or(JoinWorktrees::None)
    }

    /// Die Kennung der Session, in der der Befehl läuft (`HARNESS_SESSION`)
    pub fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    /// Öffnet das Board aus `<HARNESS_HOME>/board.yaml`, führt die Arbeit aus und schließt die
    /// Verbindung. Die Board-Dienste sind asynchron, die Befehle der CLI nicht: deshalb läuft
    /// die Arbeit in einer eigenen, einfädigen Laufzeit.
    pub fn with_board<T, W>(&self, work: W) -> Result<T>
    where
        W: AsyncFnOnce(&BoardRuntime) -> Result<T>,
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(Error::unexpected)?;
        runtime.block_on(async {
            let board = BoardRuntime::open(&self.home(), ConnectOptions::default()).await?;
            let result = work(&board).await;
            let closed = board.close().await;
            // Der Fehler der Arbeit zählt; erst wenn sie geklappt hat, zählt das Schließen
            result.and_then(|value| closed.map(|()| value))
        })
    }

    /// Wer handelt: Art (Standard: „session“ innerhalb einer Session, sonst Mensch) und Name
    /// (Standard: Git-Benutzername)
    pub fn actor(&self, kind: Option<&str>, author: Option<&str>) -> Result<Actor> {
        let kind: AuthorKind = kind.unwrap_or(self.default_kind()).parse()?;
        Ok(Actor {
            kind,
            author: author
                .map(ToString::to_string)
                .unwrap_or_else(|| self.git_user_name()),
        })
    }

    /// Wer handelt bei Fragen: wie `actor`, bei Sessions und Subagenten zusätzlich mit der Kennung
    /// der Session. Ohne `--kind` gilt „session“ innerhalb einer Session (`HARNESS_SESSION` oder
    /// `--session`), sonst „human“.
    pub fn question_actor(
        &self,
        kind: Option<&str>,
        author: Option<&str>,
        session: Option<&str>,
    ) -> Result<QuestionActor> {
        let id = session
            .map(ToString::to_string)
            .or_else(|| self.session.clone());
        let actor = self.actor(
            Some(kind.unwrap_or(if id.is_some() { "session" } else { "human" })),
            author,
        )?;
        let session = if actor.kind == AuthorKind::Human {
            None
        } else {
            id
        };
        Ok(QuestionActor {
            author: actor.author,
            kind: actor.kind,
            session,
        })
    }

    /// Wer entscheidet: nur ein Mensch, also weder `--kind session/subagent` noch ein Aufruf aus
    /// einer Session
    pub fn human_actor(&self, kind: Option<&str>, author: Option<&str>) -> Result<Actor> {
        if self.session.is_some() {
            return Err(HarnessError::new(
                "human-only",
                "Diese Entscheidung trifft ein Mensch. Bitte in der Oberfläche oder in einem eigenen Terminal ausführen.",
            )
            .into());
        }
        self.actor(kind, author)
    }

    /// Art, die ohne `--kind` gilt
    fn default_kind(&self) -> &'static str {
        if self.session.is_some() {
            "session"
        } else {
            "human"
        }
    }

    /// Git-Benutzername oder „unbekannt“
    fn git_user_name(&self) -> String {
        match GitRunner::new().run(&self.dir, &["config", "user.name"]) {
            Ok(name) if !name.is_empty() => name,
            _ => UNKNOWN_USER.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kontext(session: Option<&str>) -> CliContext {
        CliContext::with_session(
            false,
            Some(PathBuf::from("/kein/projekt")),
            session.map(ToString::to_string),
        )
    }

    #[test]
    fn ohne_session_handelt_ein_mensch() {
        let actor = kontext(None).actor(None, Some("Benjamin")).unwrap();
        assert_eq!(actor.kind, AuthorKind::Human);
        assert_eq!(actor.author, "Benjamin");
    }

    #[test]
    fn innerhalb_einer_session_handelt_die_session() {
        let actor = kontext(Some("s-1")).actor(None, Some("claude")).unwrap();
        assert_eq!(actor.kind, AuthorKind::Session);
    }

    #[test]
    fn unbekannte_art_wird_mit_kind_invalid_abgelehnt() {
        let error = kontext(None).actor(Some("mensch"), None).unwrap_err();
        assert_eq!(error.code(), Some("kind-invalid"));
        assert_eq!(
            error.to_string(),
            "Art „mensch“ ist ungültig. Erlaubt: human, session, subagent."
        );
    }

    #[test]
    fn ohne_git_benutzernamen_heisst_der_autor_unbekannt() {
        let actor = kontext(None).actor(Some("human"), None).unwrap();
        assert_eq!(actor.author, UNKNOWN_USER);
    }

    #[test]
    fn frage_akteur_tragt_die_session_nach() {
        let actor = kontext(Some("s-7"))
            .question_actor(None, Some("claude"), None)
            .unwrap();
        assert_eq!(actor.kind, AuthorKind::Session);
        assert_eq!(actor.session.as_deref(), Some("s-7"));
    }

    #[test]
    fn frage_akteur_eines_menschen_hat_keine_session() {
        let actor = kontext(Some("s-7"))
            .question_actor(Some("human"), Some("Benjamin"), None)
            .unwrap();
        assert_eq!(actor.kind, AuthorKind::Human);
        assert_eq!(actor.session, None);
    }

    #[test]
    fn frage_akteur_nimmt_die_session_auch_aus_der_option() {
        let actor = kontext(None)
            .question_actor(None, Some("claude"), Some("s-9"))
            .unwrap();
        assert_eq!(actor.kind, AuthorKind::Session);
        assert_eq!(actor.session.as_deref(), Some("s-9"));
    }

    #[test]
    fn entscheidungen_trifft_nur_ein_mensch() {
        let error = kontext(Some("s-1")).human_actor(None, None).unwrap_err();
        assert_eq!(error.code(), Some("human-only"));
        assert_eq!(
            error.to_string(),
            "Diese Entscheidung trifft ein Mensch. Bitte in der Oberfläche oder in einem eigenen Terminal ausführen."
        );
    }

    #[test]
    fn ausserhalb_einer_session_darf_ein_mensch_entscheiden() {
        let actor = kontext(None).human_actor(None, Some("Benjamin")).unwrap();
        assert_eq!(actor.kind, AuthorKind::Human);
    }

    #[test]
    fn der_projektordner_kommt_aus_der_option() {
        assert_eq!(kontext(None).project_dir(), Path::new("/kein/projekt"));
    }
}
