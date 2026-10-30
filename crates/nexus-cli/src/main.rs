//! Binary `nexus`: startet das Programm und übersetzt Fehler in Rückgabecodes
//! (1 erwartbar, 2 unerwartet). Portiert aus `packages/cli/src/main.ts`.

use std::process::ExitCode;

use nexus_cli::{cli, CliErrorText, Printer};
use nexus_core::Error;

/// Startet das Programm; `--json` wird an den rohen Argumenten erkannt, damit auch Fehler
/// vor dem Parsen im richtigen Format erscheinen.
fn main() -> ExitCode {
    // Wie Node beim Start: Kindprozesse erben die Standard-Handles nur ausdrücklich. Sonst hielte
    // ein abgelöst gestarteter Server unter Windows die Pipes des Aufrufers offen.
    nexus_platform::stdio::disable_inheritance();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let printer = Printer::from_args(&args);
    match cli::run(&args) {
        Ok(code) => ExitCode::from(code),
        Err(Error::Harness(error)) => {
            let text = CliErrorText;
            printer.failure(&error.code, &text.of(&error), &text.line(&error));
            ExitCode::from(1)
        }
        Err(error) => {
            let message = format!("Unerwarteter Fehler: {error}");
            printer.failure("unexpected", &message, &message);
            ExitCode::from(2)
        }
    }
}
