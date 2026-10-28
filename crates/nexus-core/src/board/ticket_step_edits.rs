//! Reine Listenoperationen für die Schritte eines Tickets (Nummern ab 1); prüft Text, Länge
//! und Nummern. Portiert aus `packages/core/src/board/TicketStepEdits.ts`.

use crate::board::basics::MAX_STEP_TEXT_LENGTH;
use crate::board::limits::BoardLimits;
use crate::board::types::TicketStep;
use crate::error::{HarnessError, Result};

/// Ergebnis einer Schritt-Änderung: neue Liste und die Angaben für den Verlauf
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepEdit {
    pub steps: Vec<TicketStep>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub note: String,
}

/// Änderung eines Schritts (`None` lässt das Feld unverändert, ein leeres Detail entfernt die
/// Anleitung)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepChange {
    pub text: Option<String>,
    pub detail: Option<Option<String>>,
}

/// Schutz vor gleichzeitigen Änderungen: erwartete Ticket-Version und/oder erwarteter Text
/// des Schritts
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepGuard {
    pub rev: Option<i32>,
    pub expect_text: Option<String>,
}

/// Wohin ein neuer Schritt kommt: Anleitung und Position (ab 1, `None` hängt an)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepPosition {
    pub detail: Option<String>,
    pub at: Option<i32>,
}

/// Schritte einer Liste anlegen, ändern, löschen und verschieben.
pub struct TicketStepEdits;

impl TicketStepEdits {
    /// Hängt einen Schritt an oder fügt ihn an Position `at` (1 bis Anzahl + 1) ein
    pub fn add(
        ticket_id: &str,
        steps: &[TicketStep],
        text: &str,
        options: &StepPosition,
    ) -> Result<StepEdit> {
        let count = steps.len() as i32;
        let at = options.at.unwrap_or(count + 1);
        if at < 1 || at > count + 1 {
            return Err(HarnessError::new(
                "step-not-found",
                format!(
                    "{ticket_id}: Position {at} gibt es nicht (1 bis {}).",
                    count + 1
                ),
            )
            .into());
        }
        let step = with_detail(
            TicketStep {
                text: step_text(text)?,
                done: false,
                detail: None,
            },
            options.detail.as_deref(),
        );
        let mut next = steps.to_vec();
        next.insert((at - 1) as usize, step.clone());
        Ok(StepEdit {
            steps: next,
            from: None,
            to: Some(format!("{at}: {}", step.text)),
            note: "Schritt hinzugefügt".into(),
        })
    }

    /// Ändert Text und/oder Anleitung eines Schritts; der Haken bleibt
    pub fn edit(
        ticket_id: &str,
        steps: &[TicketStep],
        number: i32,
        change: &StepChange,
    ) -> Result<StepEdit> {
        if change.text.is_none() && change.detail.is_none() {
            return Err(HarnessError::new(
                "nothing-changed",
                "Keine Änderung angegeben (Text oder Anleitung).",
            )
            .into());
        }
        let old = steps[Self::index(ticket_id, steps, number)?].clone();
        let step = with_detail(
            TicketStep {
                text: match &change.text {
                    Some(text) => step_text(text)?,
                    None => old.text.clone(),
                },
                done: old.done,
                detail: None,
            },
            match &change.detail {
                Some(detail) => detail.as_deref(),
                None => old.detail.as_deref(),
            },
        );
        let note = if step.text == old.text {
            "Anleitung geändert"
        } else {
            "Schritt bearbeitet"
        };
        let mut next = steps.to_vec();
        next[(number - 1) as usize] = step.clone();
        Ok(StepEdit {
            steps: next,
            from: Some(format!("{number}: {}", old.text)),
            to: Some(format!("{number}: {}", step.text)),
            note: note.into(),
        })
    }

    /// Entfernt einen Schritt; die folgenden rücken nach
    pub fn remove(ticket_id: &str, steps: &[TicketStep], number: i32) -> Result<StepEdit> {
        let index = Self::index(ticket_id, steps, number)?;
        let mut next = steps.to_vec();
        let removed = next.remove(index);
        Ok(StepEdit {
            steps: next,
            from: Some(format!("{number}: {}", removed.text)),
            to: None,
            note: "Schritt gelöscht".into(),
        })
    }

    /// Verschiebt einen Schritt von Nummer `from` auf Nummer `to`
    pub fn move_step(
        ticket_id: &str,
        steps: &[TicketStep],
        from: i32,
        to: i32,
    ) -> Result<StepEdit> {
        let source = Self::index(ticket_id, steps, from)?;
        let target = Self::index(ticket_id, steps, to)?;
        let mut next = steps.to_vec();
        let step = next.remove(source);
        next.insert(target, step.clone());
        Ok(StepEdit {
            steps: next,
            from: Some(format!("{from}: {}", step.text)),
            to: Some(format!("{to}: {}", step.text)),
            note: "Schritt verschoben".into(),
        })
    }

    /// Prüft, ob an Nummer `number` noch der erwartete Text steht (ohne `expect_text` keine
    /// Prüfung); sonst „conflict“
    pub fn expect(
        ticket_id: &str,
        steps: &[TicketStep],
        number: i32,
        expect_text: Option<&str>,
    ) -> Result<()> {
        let Some(expect_text) = expect_text else {
            return Ok(());
        };
        let step = if number >= 1 {
            steps.get((number - 1) as usize)
        } else {
            None
        };
        let Some(step) = step else {
            return Err(HarnessError::new(
                "conflict",
                format!("{ticket_id}: Schritt {number} gibt es inzwischen nicht mehr – die Schritte wurden geändert."),
            )
            .into());
        };
        if step.text.trim() != expect_text.trim() {
            return Err(HarnessError::new(
                "conflict",
                format!(
                    "{ticket_id}: Schritt {number} ist inzwischen „{}“ – die Schritte wurden geändert.",
                    step.text
                ),
            )
            .into());
        }
        Ok(())
    }

    /// Index einer Schrittnummer; der Fehler „step-not-found“ nennt den gültigen Bereich
    pub fn index(ticket_id: &str, steps: &[TicketStep], number: i32) -> Result<usize> {
        let count = steps.len() as i32;
        if number < 1 || number > count {
            let message = if count > 0 {
                format!("{ticket_id} hat keinen Schritt {number} (1 bis {count}).")
            } else {
                format!("{ticket_id} hat keine Schritte.")
            };
            return Err(HarnessError::new("step-not-found", message).into());
        }
        Ok((number - 1) as usize)
    }
}

/// Text ohne Leerraum am Rand; leer oder zu lang ist nicht erlaubt
fn step_text(value: &str) -> Result<String> {
    let text = value.trim();
    if text.is_empty() {
        return Err(HarnessError::new("step-empty", "Ein Schritt braucht einen Text.").into());
    }
    BoardLimits::text(text, MAX_STEP_TEXT_LENGTH, "Der Text eines Schritts")?;
    Ok(text.to_string())
}

/// Setzt die Anleitung; leer oder `None` entfernt sie
fn with_detail(step: TicketStep, detail: Option<&str>) -> TicketStep {
    let detail = detail.filter(|text| !text.trim().is_empty());
    TicketStep {
        detail: detail.map(str::to_string),
        ..step
    }
}
