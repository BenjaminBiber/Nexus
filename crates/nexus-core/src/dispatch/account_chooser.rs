//! Wählt den Claude-Account eines Laufs; portiert aus
//! `packages/core/src/dispatch/AccountChooser.ts`.
//!
//! Kandidaten sind `allowed_accounts` (sonst alle), ohne abgeschaltete, abgemeldete, pausierte
//! und volle; Fallback-Accounts nur, wenn sonst keiner frei ist; dann meiste freie Kapazität
//! oder erster der Liste. Reine Logik – Auslastung und Pausen kommen von außen.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Local, Utc};

use crate::accounts::ClaudeAccount;
use crate::dispatch::config::AccountStrategy;
use crate::dispatch::state::UsageSnapshot;

/// Erwarteter Verbrauch eines Laufs in Prozentpunkten je Fenster
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AccountReserve {
    /// Prozentpunkte im 5-Stunden-Fenster
    pub five_hour: i64,
    /// Prozentpunkte in der Woche
    pub week: i64,
}

impl AccountReserve {
    /// Summe zweier Reserven
    pub fn plus(self, other: AccountReserve) -> AccountReserve {
        AccountReserve {
            five_hour: self.five_hour + other.five_hour,
            week: self.week + other.week,
        }
    }
}

/// Was über einen Account beim Start bekannt ist; `None` = unbekannt (schließt nicht aus)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountState {
    pub logged_in: Option<bool>,
    pub plugin: Option<bool>,
    /// Auslastung in Prozent im 5-Stunden-Fenster; `None` = nicht abgefragt oder nicht erreichbar
    pub five_hour: Option<i64>,
    /// Auslastung in Prozent in der Woche
    pub seven_day: Option<i64>,
    /// Rücksetzung des 5-Stunden-Fensters laut Anthropic (ISO); `None` = unbekannt
    pub five_hour_resets_at: Option<String>,
    /// Rücksetzung der Woche laut Anthropic (ISO)
    pub seven_day_resets_at: Option<String>,
    /// Pause des Dispatchers für diesen Account (ISO)
    pub paused_until: Option<String>,
}

/// Was in die Wahl eingeht
#[derive(Debug, Clone)]
pub struct AccountChoiceInput<'a> {
    /// Erlaubte Accounts des Laufs; leer = alle
    pub allowed_accounts: Option<Vec<String>>,
    /// Herkunft der erlaubten Accounts („Profil schwer“); `None` = „Einstellung“
    pub accounts_from: Option<String>,
    pub accounts: &'a [ClaudeAccount],
    pub states: BTreeMap<String, AccountState>,
    pub strategy: AccountStrategy,
    pub limit_percent: i64,
    pub now: DateTime<Utc>,
    /// Erwarteter Verbrauch dieses Laufs
    pub reserve: AccountReserve,
    /// Noch ausstehender Verbrauch laufender Läufe je Account-ID
    pub pending: BTreeMap<String, AccountReserve>,
    /// Rücksetzung des 5-Stunden-Fensters innerhalb dieser Minuten: Reserve zählt dort nicht
    pub reset_grace_minutes: i64,
}

/// Ergebnis der Wahl
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountChoice {
    /// Dieser Account übernimmt den Lauf, samt Begründung für den Start-Kommentar und seiner
    /// Auslastung in diesem Moment
    Picked {
        account: ClaudeAccount,
        why: String,
        snapshot: UsageSnapshot,
    },
    /// Kein Account frei: Grund fürs Warten und frühester Zeitpunkt, ab dem wieder einer frei
    /// ist (Pause oder Rücksetzung eines vollen Fensters); `None`, wenn einer der Kandidaten
    /// ohne bekanntes Ende ausfällt
    Waiting {
        reason: String,
        resume_at: Option<String>,
    },
}

/// Warum ein Account nicht in Frage kommt und bis wann
struct Excluded {
    reason: String,
    /// Bis wann; `None` = kein bekanntes Ende
    until: Option<String>,
}

/// Ein freier Account samt bekannter Auslastung
struct Free<'a> {
    account: &'a ClaudeAccount,
    load: Option<i64>,
}

/// Wählt den Account eines Laufs.
#[derive(Debug, Clone, Copy, Default)]
pub struct AccountChooser;

impl AccountChooser {
    /// Neuer Chooser (hält keinen Zustand)
    pub fn new() -> Self {
        Self
    }

    /// Der Account dieses Laufs oder der Grund fürs Warten
    pub fn choose(&self, input: &AccountChoiceInput<'_>) -> AccountChoice {
        let allowed = input
            .allowed_accounts
            .as_deref()
            .filter(|ids| !ids.is_empty());
        let ids: Vec<String> = match allowed {
            Some(ids) => ids.to_vec(),
            None => input
                .accounts
                .iter()
                .map(|account| account.id.clone())
                .collect(),
        };
        let from = match allowed {
            Some(_) => input
                .accounts_from
                .clone()
                .unwrap_or_else(|| "Einstellung".to_string()),
            None => "alle Accounts".to_string(),
        };
        let mut free: Vec<Free<'_>> = Vec::new();
        let mut excluded: Vec<String> = Vec::new();
        let mut resumes: Vec<String> = Vec::new();
        let mut all_timed = true;
        let unknown = AccountState::default();
        for id in &ids {
            let Some(account) = input.accounts.iter().find(|account| &account.id == id) else {
                excluded.push(format!("{id}: gibt es nicht"));
                all_timed = false;
                continue;
            };
            let state = input.states.get(id).unwrap_or(&unknown);
            match excluded_reason(account, state, input) {
                Some(out) => {
                    excluded.push(format!("{id}: {}", out.reason));
                    match out.until {
                        Some(until) => resumes.push(until),
                        None => all_timed = false,
                    }
                }
                None => free.push(Free {
                    account,
                    load: load(state),
                }),
            }
        }
        if free.is_empty() {
            resumes.sort();
            let resume_at = if all_timed {
                resumes.first().cloned()
            } else {
                None
            };
            let resume = match &resume_at {
                Some(at) => format!(" – wieder frei ab {}", hhmm(at)),
                None => String::new(),
            };
            let why = if excluded.is_empty() {
                "keiner erlaubt".to_string()
            } else {
                excluded.join("; ")
            };
            return AccountChoice::Waiting {
                reason: format!("kein Account frei ({why}){resume}"),
                resume_at,
            };
        }
        // Fallback-Accounts zählen nur, wenn kein regulärer frei ist
        let regular = free.iter().filter(|f| !f.account.fallback).count();
        let pool: Vec<&Free<'_>> = if regular > 0 {
            free.iter().filter(|f| !f.account.fallback).collect()
        } else {
            free.iter().collect()
        };
        let pick = match input.strategy {
            AccountStrategy::Order => pool[0],
            AccountStrategy::MostFree => least_loaded(&pool),
        };
        let loads = if pool.len() > 1 {
            let parts: Vec<String> = pool
                .iter()
                .map(|f| {
                    let load = match f.load {
                        Some(load) => format!("{load} %"),
                        None => "?".to_string(),
                    };
                    format!("{} ({load})", f.account.id)
                })
                .collect();
            format!(": {}", parts.join(", "))
        } else {
            String::new()
        };
        let strategy = if pool.len() > 1 {
            match input.strategy {
                AccountStrategy::Order => ", erster freier",
                AccountStrategy::MostFree => ", meiste freie Kapazität",
            }
        } else {
            ""
        };
        let fallback = if regular == 0 && pick.account.fallback {
            ", Fallback (kein anderer frei)"
        } else {
            ""
        };
        let skipped = if excluded.is_empty() {
            String::new()
        } else {
            format!("; übersprungen {}", excluded.join("; "))
        };
        let snapshot = input
            .states
            .get(&pick.account.id)
            .map(|state| UsageSnapshot {
                five_hour: state.five_hour,
                seven_day: state.seven_day,
                five_hour_resets_at: state.five_hour_resets_at.clone(),
                seven_day_resets_at: state.seven_day_resets_at.clone(),
            })
            .unwrap_or_default();
        AccountChoice::Picked {
            account: pick.account.clone(),
            why: format!(
                "{from} → {}{fallback}{strategy}{loads}{skipped}",
                pick.account.id
            ),
            snapshot,
        }
    }
}

/// Warum der Account nicht in Frage kommt und bis wann – oder `None`
fn excluded_reason(
    account: &ClaudeAccount,
    state: &AccountState,
    input: &AccountChoiceInput<'_>,
) -> Option<Excluded> {
    if !account.enabled {
        return Some(Excluded {
            reason: "abgeschaltet".to_string(),
            until: None,
        });
    }
    if let Some(paused_until) = &state.paused_until {
        if after(paused_until, input.now) {
            return Some(Excluded {
                reason: format!("pausiert bis {}", hhmm(paused_until)),
                until: Some(paused_until.clone()),
            });
        }
    }
    if state.logged_in == Some(false) {
        return Some(Excluded {
            reason: "nicht angemeldet".to_string(),
            until: None,
        });
    }
    if state.plugin == Some(false) {
        return Some(Excluded {
            reason: "Nexus-Plugin fehlt".to_string(),
            until: None,
        });
    }
    // Eigene Obergrenze des Accounts vor der Vorgabe des Dispatchers
    let five = account.limit_five_hour.unwrap_or(input.limit_percent);
    let week = account.limit_week.unwrap_or(input.limit_percent);
    let mut full: Vec<(String, Option<String>)> = Vec::new();
    if let Some(used) = state.five_hour.filter(|used| *used >= five) {
        let own = match account.limit_five_hour {
            Some(_) => format!(", Grenze {five} %"),
            None => String::new(),
        };
        full.push((
            format!("5 h {used} %{own}"),
            state.five_hour_resets_at.clone(),
        ));
    }
    if let Some(used) = state.seven_day.filter(|used| *used >= week) {
        let own = match account.limit_week {
            Some(_) => format!(", Grenze {week} %"),
            None => String::new(),
        };
        full.push((
            format!("Woche {used} %{own}"),
            state.seven_day_resets_at.clone(),
        ));
    }
    if full.is_empty() {
        return tight(account, state, input, five, week);
    }
    let until = latest_reset(&full, input.now);
    let bis = match &until {
        Some(at) => format!(" bis {}", hhmm(at)),
        None => String::new(),
    };
    Some(Excluded {
        reason: format!("voll ({}){bis}", full[0].0),
        until,
    })
}

/// Frei erst, wenn jedes der Fenster zurückgesetzt ist: die späteste Rücksetzung; fehlt eine
/// Rücksetzzeit (oder liegt sie zurück), ist das Ende unbekannt
fn latest_reset(windows: &[(String, Option<String>)], now: DateTime<Utc>) -> Option<String> {
    let known = windows
        .iter()
        .all(|(_, resets_at)| resets_at.as_deref().is_some_and(|at| after(at, now)));
    if !known {
        return None;
    }
    let mut times: Vec<String> = windows
        .iter()
        .filter_map(|(_, resets_at)| resets_at.clone())
        .collect();
    times.sort();
    times.last().cloned()
}

/// Knapp: kein Fenster ist voll, aber Auslastung plus erwarteter Verbrauch (Reserve des Laufs
/// und ausstehende Reserve laufender Läufe) überschreitet die Grenze. Das 5-Stunden-Fenster
/// zählt nicht, wenn es sich innerhalb der Kulanz zurücksetzt; die Woche kennt keine Ausnahme.
/// Unbekannte Auslastung schließt nie aus.
fn tight(
    account: &ClaudeAccount,
    state: &AccountState,
    input: &AccountChoiceInput<'_>,
    five: i64,
    week: i64,
) -> Option<Excluded> {
    let need = input
        .reserve
        .plus(input.pending.get(&account.id).copied().unwrap_or_default());
    let grace_end = input.now + Duration::minutes(input.reset_grace_minutes);
    let resets_soon = state
        .five_hour_resets_at
        .as_deref()
        .is_some_and(|at| after(at, input.now) && !after(at, grace_end));
    let mut tight: Vec<(String, Option<String>)> = Vec::new();
    if let Some(used) = state.five_hour {
        if need.five_hour > 0 && used + need.five_hour > five && !resets_soon {
            tight.push((
                format!(
                    "5 h {used} % + {} % erwartet, Grenze {five} %",
                    need.five_hour
                ),
                state.five_hour_resets_at.clone(),
            ));
        }
    }
    if let Some(used) = state.seven_day {
        if need.week > 0 && used + need.week > week {
            tight.push((
                format!("Woche {used} % + {} % erwartet, Grenze {week} %", need.week),
                state.seven_day_resets_at.clone(),
            ));
        }
    }
    let first = tight.first()?;
    Some(Excluded {
        reason: format!("knapp ({})", first.0),
        until: latest_reset(&tight, input.now),
    })
}

/// Auslastung: das knappere Fenster; `None` = unbekannt
fn load(state: &AccountState) -> Option<i64> {
    match (state.five_hour, state.seven_day) {
        (Some(five), Some(week)) => Some(five.max(week)),
        (Some(five), None) => Some(five),
        (None, Some(week)) => Some(week),
        (None, None) => None,
    }
}

/// Niedrigste bekannte Auslastung; unbekannte zählen dahinter, bei Gleichstand gilt die
/// Reihenfolge
fn least_loaded<'a, 'b>(pool: &[&'a Free<'b>]) -> &'a Free<'b> {
    let mut best = pool[0];
    for next in &pool[1..] {
        let Some(load) = next.load else { continue };
        if best.load.is_none_or(|current| load < current) {
            best = next;
        }
    }
    best
}

/// Liegt der ISO-Zeitpunkt nach `now`? Ein unlesbarer Zeitpunkt zählt wie `NaN` in
/// JavaScript: der Vergleich ist falsch
fn after(at: &str, now: DateTime<Utc>) -> bool {
    at.parse::<DateTime<Utc>>().is_ok_and(|at| at > now)
}

/// Uhrzeit HH:MM in Ortszeit, wie `Date.getHours()` in JavaScript
pub fn hhmm(at: &str) -> String {
    match at.parse::<DateTime<Utc>>() {
        Ok(at) => at.with_timezone(&Local).format("%H:%M").to_string(),
        Err(_) => at.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-10-28T10:00:00Z";

    /// Der Zeitpunkt, mit dem die Tests rechnen
    fn now() -> DateTime<Utc> {
        NOW.parse().unwrap()
    }

    /// `h` Stunden nach `NOW` als ISO-Text
    fn at(h: i64) -> String {
        crate::clock::iso(now() + chrono::Duration::hours(h))
    }

    /// Ein Account mit den Standardwerten
    fn account(id: &str) -> ClaudeAccount {
        ClaudeAccount {
            id: id.to_string(),
            name: id.to_string(),
            config_dir: if id == "standard" {
                None
            } else {
                Some(format!("/a/{id}"))
            },
            enabled: true,
            fallback: false,
            limit_five_hour: None,
            limit_week: None,
        }
    }

    /// Ein abgeschalteter Account
    fn disabled(id: &str) -> ClaudeAccount {
        ClaudeAccount {
            enabled: false,
            ..account(id)
        }
    }

    /// Ein Fallback-Account
    fn fallback(id: &str) -> ClaudeAccount {
        ClaudeAccount {
            fallback: true,
            ..account(id)
        }
    }

    /// Angemeldet, mit Plugin, ohne bekannte Auslastung
    fn state() -> AccountState {
        AccountState {
            logged_in: Some(true),
            plugin: Some(true),
            ..AccountState::default()
        }
    }

    /// Die Zustände als Karte
    fn states(pairs: Vec<(&str, AccountState)>) -> BTreeMap<String, AccountState> {
        pairs
            .into_iter()
            .map(|(id, state)| (id.to_string(), state))
            .collect()
    }

    /// Die Vorgabe-Accounts der Tests
    fn standard_accounts() -> Vec<ClaudeAccount> {
        vec![
            account("standard"),
            account("pro-1"),
            account("pro-2"),
            account("max"),
        ]
    }

    /// Wahl mit den Vorgaben der TypeScript-Tests
    fn choose(
        accounts: &[ClaudeAccount],
        allowed: Option<&[&str]>,
        from: Option<&str>,
        states: BTreeMap<String, AccountState>,
        strategy: AccountStrategy,
    ) -> AccountChoice {
        AccountChooser::new().choose(&AccountChoiceInput {
            allowed_accounts: allowed
                .map(|ids| ids.iter().map(ToString::to_string).collect::<Vec<String>>()),
            accounts_from: from.map(ToString::to_string),
            accounts,
            states,
            strategy,
            limit_percent: 90,
            now: now(),
            reserve: AccountReserve::default(),
            pending: BTreeMap::new(),
            reset_grace_minutes: 30,
        })
    }

    /// Wahl mit Reserve des Laufs, ausstehender Reserve und Ausnahme von 30 Minuten
    fn choose_with_reserve(
        accounts: &[ClaudeAccount],
        states: BTreeMap<String, AccountState>,
        reserve: AccountReserve,
        pending: BTreeMap<String, AccountReserve>,
    ) -> AccountChoice {
        AccountChooser::new().choose(&AccountChoiceInput {
            allowed_accounts: None,
            accounts_from: None,
            accounts,
            states,
            strategy: AccountStrategy::Order,
            limit_percent: 90,
            now: now(),
            reserve,
            pending,
            reset_grace_minutes: 30,
        })
    }

    /// `m` Minuten nach `NOW` als ISO-Text
    fn in_minutes(m: i64) -> String {
        crate::clock::iso(now() + Duration::minutes(m))
    }

    /// Die ID des gewählten Accounts
    fn picked(choice: &AccountChoice) -> &str {
        match choice {
            AccountChoice::Picked { account, .. } => &account.id,
            AccountChoice::Waiting { reason, .. } => panic!("es wartet: {reason}"),
        }
    }

    /// Die Begründung der Wahl
    fn why(choice: &AccountChoice) -> &str {
        match choice {
            AccountChoice::Picked { why, .. } => why,
            AccountChoice::Waiting { reason, .. } => panic!("es wartet: {reason}"),
        }
    }

    #[test]
    fn ohne_erlaubte_accounts_darf_jeder_aktive_und_ohne_werte_gewinnt_der_erste() {
        let accounts = vec![disabled("standard"), account("pro-1")];
        let choice = choose(
            &accounts,
            None,
            None,
            states(vec![]),
            AccountStrategy::MostFree,
        );
        assert_eq!(picked(&choice), "pro-1");
    }

    #[test]
    fn most_free_nimmt_die_niedrigste_auslastung_unbekannte_zaehlen_dahinter() {
        let accounts = standard_accounts();
        let choice = choose(
            &accounts,
            Some(&["pro-1", "pro-2", "max"]),
            Some("Profil klein"),
            states(vec![
                (
                    "pro-1",
                    AccountState {
                        five_hour: Some(20),
                        seven_day: Some(70),
                        ..state()
                    },
                ),
                (
                    "pro-2",
                    AccountState {
                        five_hour: Some(40),
                        seven_day: Some(41),
                        ..state()
                    },
                ),
                ("max", state()),
            ]),
            AccountStrategy::MostFree,
        );
        assert_eq!(
            choice,
            AccountChoice::Picked {
                account: account("pro-2"),
                why: "Profil klein → pro-2, meiste freie Kapazität: pro-1 (70 %), pro-2 (41 %), max (?)"
                    .to_string(),
                snapshot: UsageSnapshot {
                    five_hour: Some(40),
                    seven_day: Some(41),
                    five_hour_resets_at: None,
                    seven_day_resets_at: None,
                },
            }
        );
    }

    #[test]
    fn order_nimmt_den_ersten_freien_der_liste() {
        let accounts = standard_accounts();
        let choice = choose(
            &accounts,
            Some(&["pro-1", "pro-2"]),
            None,
            states(vec![
                (
                    "pro-1",
                    AccountState {
                        five_hour: Some(95),
                        ..state()
                    },
                ),
                (
                    "pro-2",
                    AccountState {
                        five_hour: Some(60),
                        ..state()
                    },
                ),
            ]),
            AccountStrategy::Order,
        );
        assert_eq!(picked(&choice), "pro-2");
        assert!(why(&choice).contains("übersprungen pro-1: voll (5 h 95 %)"));
    }

    #[test]
    fn schliesst_abgeschaltete_abgemeldete_pausierte_und_accounts_ohne_plugin_aus() {
        let accounts = vec![
            disabled("a"),
            account("b"),
            account("c"),
            account("d"),
            account("e"),
        ];
        let choice = choose(
            &accounts,
            None,
            None,
            states(vec![
                (
                    "b",
                    AccountState {
                        logged_in: Some(false),
                        ..state()
                    },
                ),
                (
                    "c",
                    AccountState {
                        plugin: Some(false),
                        ..state()
                    },
                ),
                (
                    "d",
                    AccountState {
                        paused_until: Some("2026-10-28T11:00:00Z".to_string()),
                        ..state()
                    },
                ),
                ("e", AccountState::default()),
            ]),
            AccountStrategy::MostFree,
        );
        assert_eq!(picked(&choice), "e");
    }

    #[test]
    fn ein_profil_mit_nur_einem_vollen_account_wartet() {
        let accounts = standard_accounts();
        let choice = choose(
            &accounts,
            Some(&["max"]),
            Some("Profil schwer"),
            states(vec![(
                "max",
                AccountState {
                    seven_day: Some(90),
                    ..state()
                },
            )]),
            AccountStrategy::MostFree,
        );
        assert_eq!(
            choice,
            AccountChoice::Waiting {
                reason: "kein Account frei (max: voll (Woche 90 %))".to_string(),
                resume_at: None,
            }
        );
    }

    #[test]
    fn sind_alle_kandidaten_nur_pausiert_nennt_das_warten_die_frueheste_pause() {
        let accounts = standard_accounts();
        let choice = choose(
            &accounts,
            Some(&["pro-1", "pro-2"]),
            None,
            states(vec![
                (
                    "pro-1",
                    AccountState {
                        paused_until: Some("2026-10-28T12:00:00Z".to_string()),
                        ..state()
                    },
                ),
                (
                    "pro-2",
                    AccountState {
                        paused_until: Some("2026-10-28T11:00:00Z".to_string()),
                        ..state()
                    },
                ),
            ]),
            AccountStrategy::MostFree,
        );
        let AccountChoice::Waiting { reason, resume_at } = choice else {
            panic!("es sollte warten");
        };
        assert_eq!(resume_at.as_deref(), Some("2026-10-28T11:00:00Z"));
        assert!(reason.contains(" – wieder frei ab "));
    }

    #[test]
    fn volle_accounts_warten_bis_zur_ruecksetzung_jedes_vollen_fensters() {
        let accounts = standard_accounts();
        let choice = choose(
            &accounts,
            Some(&["pro-1", "pro-2"]),
            None,
            states(vec![
                // Beide Fenster voll: frei erst nach der späteren Rücksetzung
                (
                    "pro-1",
                    AccountState {
                        five_hour: Some(100),
                        five_hour_resets_at: Some(at(1)),
                        seven_day: Some(95),
                        seven_day_resets_at: Some(at(30)),
                        ..state()
                    },
                ),
                (
                    "pro-2",
                    AccountState {
                        five_hour: Some(97),
                        five_hour_resets_at: Some(at(2)),
                        seven_day: Some(40),
                        seven_day_resets_at: Some(at(100)),
                        ..state()
                    },
                ),
            ]),
            AccountStrategy::MostFree,
        );
        assert_eq!(
            choice,
            AccountChoice::Waiting {
                reason: format!(
                    "kein Account frei (pro-1: voll (5 h 100 %) bis {}; pro-2: voll (5 h 97 %) bis {}) – wieder frei ab {}",
                    hhmm(&at(30)),
                    hhmm(&at(2)),
                    hhmm(&at(2))
                ),
                resume_at: Some(at(2)),
            }
        );

        // Pause und volles Fenster gemischt
        let gemischt = choose(
            &accounts,
            Some(&["pro-1", "pro-2"]),
            None,
            states(vec![
                (
                    "pro-1",
                    AccountState {
                        paused_until: Some(at(3)),
                        ..state()
                    },
                ),
                (
                    "pro-2",
                    AccountState {
                        five_hour: Some(99),
                        five_hour_resets_at: Some(at(4)),
                        ..state()
                    },
                ),
            ]),
            AccountStrategy::MostFree,
        );
        assert!(
            matches!(gemischt, AccountChoice::Waiting { resume_at, .. } if resume_at == Some(at(3)))
        );

        // Ohne Rücksetzzeit (oder schon vorbei) ist das Ende unbekannt
        let unbekannt = choose(
            &accounts,
            Some(&["pro-1", "pro-2"]),
            None,
            states(vec![
                (
                    "pro-1",
                    AccountState {
                        paused_until: Some(at(3)),
                        ..state()
                    },
                ),
                (
                    "pro-2",
                    AccountState {
                        five_hour: Some(99),
                        five_hour_resets_at: Some(at(-1)),
                        ..state()
                    },
                ),
            ]),
            AccountStrategy::MostFree,
        );
        assert!(matches!(
            unbekannt,
            AccountChoice::Waiting {
                resume_at: None,
                ..
            }
        ));
    }

    #[test]
    fn unbekannte_account_ids_schliessen_sich_selbst_aus() {
        let accounts = standard_accounts();
        assert_eq!(
            choose(
                &accounts,
                Some(&["gibt-es-nicht"]),
                None,
                states(vec![]),
                AccountStrategy::MostFree
            ),
            AccountChoice::Waiting {
                reason: "kein Account frei (gibt-es-nicht: gibt es nicht)".to_string(),
                resume_at: None,
            }
        );
    }

    #[test]
    fn eine_abgelaufene_pause_zaehlt_nicht_mehr() {
        let accounts = standard_accounts();
        let choice = choose(
            &accounts,
            Some(&["max"]),
            None,
            states(vec![(
                "max",
                AccountState {
                    paused_until: Some("2026-10-28T09:00:00Z".to_string()),
                    ..state()
                },
            )]),
            AccountStrategy::MostFree,
        );
        assert_eq!(picked(&choice), "max");
    }

    #[test]
    fn fallback_accounts_nur_wenn_kein_regulaerer_frei_ist() {
        let accounts = vec![account("arbeit"), fallback("privat")];
        let busy = vec![
            (
                "arbeit",
                AccountState {
                    five_hour: Some(80),
                    ..state()
                },
            ),
            (
                "privat",
                AccountState {
                    five_hour: Some(5),
                    ..state()
                },
            ),
        ];
        let choice = choose(
            &accounts,
            None,
            None,
            states(busy.clone()),
            AccountStrategy::MostFree,
        );
        assert_eq!(picked(&choice), "arbeit");

        let mut voll = busy.clone();
        voll[0].1 = AccountState {
            five_hour: Some(95),
            ..state()
        };
        let choice = choose(
            &accounts,
            None,
            None,
            states(voll),
            AccountStrategy::MostFree,
        );
        assert_eq!(picked(&choice), "privat");
        assert_eq!(
            why(&choice),
            "alle Accounts → privat, Fallback (kein anderer frei); übersprungen arbeit: voll (5 h 95 %)"
        );

        let mut pausiert = busy.clone();
        pausiert[0].1 = AccountState {
            paused_until: Some("2026-10-28T11:00:00Z".to_string()),
            ..state()
        };
        let choice = choose(
            &accounts,
            None,
            None,
            states(pausiert),
            AccountStrategy::MostFree,
        );
        assert_eq!(picked(&choice), "privat");
    }

    #[test]
    fn mehrere_fallbacks_untereinander_nach_der_strategie() {
        let accounts = vec![account("arbeit"), fallback("f1"), fallback("f2")];
        let zustand = vec![
            (
                "arbeit",
                AccountState {
                    five_hour: Some(99),
                    ..state()
                },
            ),
            (
                "f1",
                AccountState {
                    five_hour: Some(60),
                    ..state()
                },
            ),
            (
                "f2",
                AccountState {
                    five_hour: Some(10),
                    ..state()
                },
            ),
        ];
        assert_eq!(
            picked(&choose(
                &accounts,
                None,
                None,
                states(zustand.clone()),
                AccountStrategy::MostFree
            )),
            "f2"
        );
        assert_eq!(
            picked(&choose(
                &accounts,
                None,
                None,
                states(zustand),
                AccountStrategy::Order
            )),
            "f1"
        );
    }

    #[test]
    fn eigene_obergrenzen_gelten_vor_der_vorgabe_niedriger_wie_hoeher() {
        let accounts = vec![
            ClaudeAccount {
                limit_five_hour: Some(50),
                ..account("arbeit")
            },
            ClaudeAccount {
                limit_week: Some(95),
                ..account("privat")
            },
        ];
        let zustand = vec![
            (
                "arbeit",
                AccountState {
                    five_hour: Some(55),
                    seven_day: Some(10),
                    ..state()
                },
            ),
            (
                "privat",
                AccountState {
                    five_hour: Some(70),
                    seven_day: Some(92),
                    ..state()
                },
            ),
        ];
        let choice = choose(
            &accounts,
            None,
            None,
            states(zustand.clone()),
            AccountStrategy::MostFree,
        );
        assert_eq!(picked(&choice), "privat");
        assert!(why(&choice).contains("arbeit: voll (5 h 55 %, Grenze 50 %)"));

        let mut ueber = zustand;
        ueber[1].1 = AccountState {
            seven_day: Some(96),
            ..state()
        };
        let choice = choose(
            &accounts,
            None,
            None,
            states(ueber),
            AccountStrategy::MostFree,
        );
        let AccountChoice::Waiting { reason, .. } = choice else {
            panic!("es sollte warten");
        };
        assert!(reason.contains("privat: voll (Woche 96 %, Grenze 95 %)"));
    }

    #[test]
    fn knapp_schliesst_aus_wenn_verbrauch_plus_reserve_die_grenze_ueberschreitet() {
        let accounts = vec![account("a")];
        let choice = choose_with_reserve(
            &accounts,
            states(vec![(
                "a",
                AccountState {
                    five_hour: Some(88),
                    five_hour_resets_at: Some(at(2)),
                    ..state()
                },
            )]),
            AccountReserve {
                five_hour: 10,
                week: 0,
            },
            BTreeMap::new(),
        );
        let AccountChoice::Waiting { reason, resume_at } = choice else {
            panic!("es sollte warten");
        };
        assert!(reason.contains("knapp (5 h 88 % + 10 % erwartet, Grenze 90 %)"));
        assert_eq!(resume_at, Some(at(2)));
    }

    #[test]
    fn bevorstehende_ruecksetzung_des_5_stunden_fensters_hebt_die_reserve_auf() {
        let accounts = vec![account("a")];
        let choice = choose_with_reserve(
            &accounts,
            states(vec![(
                "a",
                AccountState {
                    five_hour: Some(88),
                    five_hour_resets_at: Some(in_minutes(10)),
                    ..state()
                },
            )]),
            AccountReserve {
                five_hour: 10,
                week: 0,
            },
            BTreeMap::new(),
        );
        assert_eq!(picked(&choice), "a");
    }

    #[test]
    fn die_woche_kennt_keine_ausnahme_bei_naher_ruecksetzung() {
        let accounts = vec![account("a")];
        let choice = choose_with_reserve(
            &accounts,
            states(vec![(
                "a",
                AccountState {
                    seven_day: Some(88),
                    seven_day_resets_at: Some(in_minutes(10)),
                    ..state()
                },
            )]),
            AccountReserve {
                five_hour: 0,
                week: 5,
            },
            BTreeMap::new(),
        );
        let AccountChoice::Waiting { reason, resume_at } = choice else {
            panic!("es sollte warten");
        };
        assert!(reason.contains("knapp (Woche 88 % + 5 % erwartet, Grenze 90 %)"));
        assert_eq!(resume_at, Some(in_minutes(10)));
    }

    #[test]
    fn ausstehende_reserve_laufender_laeufe_wird_addiert() {
        let accounts = vec![account("a")];
        let pending = BTreeMap::from([(
            "a".to_string(),
            AccountReserve {
                five_hour: 15,
                week: 0,
            },
        )]);
        let choice = choose_with_reserve(
            &accounts,
            states(vec![(
                "a",
                AccountState {
                    five_hour: Some(70),
                    five_hour_resets_at: Some(at(2)),
                    ..state()
                },
            )]),
            AccountReserve {
                five_hour: 10,
                week: 0,
            },
            pending,
        );
        let AccountChoice::Waiting { reason, .. } = choice else {
            panic!("es sollte warten");
        };
        assert!(reason.contains("knapp (5 h 70 % + 25 % erwartet, Grenze 90 %)"));
    }

    #[test]
    fn ein_knapper_account_wird_uebersprungen_und_der_freie_gewaehlt() {
        let accounts = vec![account("erster"), account("zweiter")];
        let choice = choose_with_reserve(
            &accounts,
            states(vec![
                (
                    "erster",
                    AccountState {
                        five_hour: Some(88),
                        five_hour_resets_at: Some(at(2)),
                        ..state()
                    },
                ),
                (
                    "zweiter",
                    AccountState {
                        five_hour: Some(20),
                        ..state()
                    },
                ),
            ]),
            AccountReserve {
                five_hour: 10,
                week: 0,
            },
            BTreeMap::new(),
        );
        assert_eq!(picked(&choice), "zweiter");
        assert!(why(&choice).contains("übersprungen erster: knapp (5 h 88 % + 10 % erwartet"));
    }

    #[test]
    fn die_wahl_bringt_die_auslastung_des_gewaehlten_accounts_mit() {
        let accounts = vec![account("a"), account("b")];
        let choice = choose(
            &accounts,
            None,
            None,
            states(vec![
                (
                    "a",
                    AccountState {
                        five_hour: Some(95),
                        ..state()
                    },
                ),
                (
                    "b",
                    AccountState {
                        five_hour: Some(40),
                        seven_day: Some(12),
                        five_hour_resets_at: Some(at(2)),
                        seven_day_resets_at: Some(at(50)),
                        ..state()
                    },
                ),
            ]),
            AccountStrategy::MostFree,
        );
        let AccountChoice::Picked {
            account, snapshot, ..
        } = choice
        else {
            panic!("es sollte einer gewählt sein");
        };
        assert_eq!(account.id, "b");
        assert_eq!(
            snapshot,
            UsageSnapshot {
                five_hour: Some(40),
                seven_day: Some(12),
                five_hour_resets_at: Some(at(2)),
                seven_day_resets_at: Some(at(50)),
            }
        );
    }

    #[test]
    fn ohne_bekannten_zustand_ist_der_snapshot_leer() {
        let accounts = vec![account("a")];
        let choice = choose(
            &accounts,
            None,
            None,
            states(vec![]),
            AccountStrategy::MostFree,
        );
        let AccountChoice::Picked { snapshot, .. } = choice else {
            panic!("es sollte einer gewählt sein");
        };
        assert_eq!(snapshot, UsageSnapshot::default());
    }

    #[test]
    fn ohne_reserve_und_bei_unbekannter_auslastung_schliesst_knapp_nie_aus() {
        let accounts = vec![account("a"), account("b")];
        let choice = choose_with_reserve(
            &accounts,
            states(vec![
                (
                    "a",
                    AccountState {
                        five_hour: Some(89),
                        ..state()
                    },
                ),
                ("b", state()),
            ]),
            AccountReserve::default(),
            BTreeMap::new(),
        );
        assert_eq!(picked(&choice), "a");
        let choice = choose_with_reserve(
            &accounts[1..],
            states(vec![("b", state())]),
            AccountReserve {
                five_hour: 50,
                week: 50,
            },
            BTreeMap::new(),
        );
        assert_eq!(picked(&choice), "b");
    }
}
