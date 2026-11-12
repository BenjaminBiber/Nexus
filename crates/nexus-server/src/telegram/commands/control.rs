//! `/pause [min]` und `/weiter`: dieselben Dienste wie `POST /api/dispatch/pause|resume`.

use async_trait::async_trait;
use nexus_core::Result;

use crate::telegram::commands::{report, Deps};
use crate::telegram::router::{BotContext, CommandHandler};
use crate::telegram::views::{paused, BAD_MINUTES, RESUMED};

/// Pause ohne Angabe, in Minuten (wie `DEFAULT_PAUSE_MINUTES` der Route)
pub const DEFAULT_PAUSE_MINUTES: i64 = 60;

/// Erlaubte Länge einer Pause in Minuten (wie in der Route)
const RANGE: std::ops::RangeInclusive<i64> = 1..=1440;

/// Der Befehl `/pause`
#[derive(Debug)]
pub(crate) struct PauseCommand(pub(crate) Deps);

#[async_trait]
impl CommandHandler for PauseCommand {
    async fn run(&self, ctx: &BotContext, args: &str) -> Result<()> {
        let Some(minutes) = minutes(args) else {
            return ctx.reply(BAD_MINUTES).await;
        };
        let state = self.0.state()?;
        match state.dispatch.pause(minutes).await {
            Ok(()) => ctx.reply(&paused(minutes)).await,
            Err(error) => report(ctx, &error).await,
        }
    }
}

/// Der Befehl `/weiter`
#[derive(Debug)]
pub(crate) struct ResumeCommand(pub(crate) Deps);

#[async_trait]
impl CommandHandler for ResumeCommand {
    async fn run(&self, ctx: &BotContext, _args: &str) -> Result<()> {
        let state = self.0.state()?;
        match state.dispatch.resume().await {
            Ok(()) => ctx.reply(RESUMED).await,
            Err(error) => report(ctx, &error).await,
        }
    }
}

/// Die Minuten aus dem Argument; ohne Angabe die Vorgabe, bei allem anderen `None`
fn minutes(args: &str) -> Option<i64> {
    let wanted = args.split_whitespace().next().unwrap_or_default();
    if wanted.is_empty() {
        return Some(DEFAULT_PAUSE_MINUTES);
    }
    wanted
        .parse::<i64>()
        .ok()
        .filter(|minutes| RANGE.contains(minutes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nimmt_ohne_angabe_eine_stunde() {
        assert_eq!(minutes(""), Some(DEFAULT_PAUSE_MINUTES));
        assert_eq!(minutes("   "), Some(DEFAULT_PAUSE_MINUTES));
    }

    #[test]
    fn nimmt_eine_zahl_im_erlaubten_bereich() {
        assert_eq!(minutes("30"), Some(30));
        assert_eq!(minutes("1"), Some(1));
        assert_eq!(minutes("1440"), Some(1440));
        assert_eq!(minutes("30 Minuten"), Some(30));
    }

    #[test]
    fn weist_alles_andere_ab() {
        for kaputt in ["0", "-5", "1441", "halbe Stunde", "30.5"] {
            assert_eq!(minutes(kaputt), None, "{kaputt}");
        }
    }
}
