//! Routengruppen des Servers und der Notschalter, mit dem einzelne Gruppen trotz
//! Rust-Umsetzung weiter zum Node-Server gehen.
//!
//! Jede Phase-4-Umsetzung einer Routengruppe registriert ihre Routen nur, wenn die Gruppe
//! aktiv ist. Steht ihr Name in `NEXUS_PROXY_GROUPS`, bleibt sie ungeschaltet und der Proxy
//! beantwortet sie wie bisher über Node.

use std::collections::BTreeSet;
use std::fmt;

/// Eine Routengruppe des Servers. **Neue Gruppen hier eintragen**, dazu in
/// [`RouteGroup::ALL`] und in `routes::build_router`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RouteGroup {
    /// Health, Projekte und die statische Oberfläche (dieses Ticket)
    Basis,
    Settings,
    DbBoards,
    DbTickets,
    DbMisc,
    Questions,
    Events,
    Docs,
    DocsClaude,
    Sessions,
    Workflow,
    Notifications,
}

impl RouteGroup {
    /// Alle Gruppen in der Reihenfolge des Migrationsplans
    pub const ALL: [RouteGroup; 12] = [
        RouteGroup::Basis,
        RouteGroup::Settings,
        RouteGroup::DbBoards,
        RouteGroup::DbTickets,
        RouteGroup::DbMisc,
        RouteGroup::Questions,
        RouteGroup::Events,
        RouteGroup::Docs,
        RouteGroup::DocsClaude,
        RouteGroup::Sessions,
        RouteGroup::Workflow,
        RouteGroup::Notifications,
    ];

    /// Name der Gruppe, wie er in `NEXUS_PROXY_GROUPS` steht
    pub fn as_str(self) -> &'static str {
        match self {
            RouteGroup::Basis => "basis",
            RouteGroup::Settings => "settings",
            RouteGroup::DbBoards => "db-boards",
            RouteGroup::DbTickets => "db-tickets",
            RouteGroup::DbMisc => "db-misc",
            RouteGroup::Questions => "questions",
            RouteGroup::Events => "events",
            RouteGroup::Docs => "docs",
            RouteGroup::DocsClaude => "docs-claude",
            RouteGroup::Sessions => "sessions",
            RouteGroup::Workflow => "workflow",
            RouteGroup::Notifications => "notifications",
        }
    }

    /// Die Gruppe zu ihrem Namen; unbekannte Namen gibt es nicht
    pub fn parse(name: &str) -> Option<RouteGroup> {
        RouteGroup::ALL
            .into_iter()
            .find(|group| group.as_str() == name.trim())
    }
}

impl fmt::Display for RouteGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Die Gruppen, die zum Node-Server gehen sollen (Notschalter `NEXUS_PROXY_GROUPS`)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RouteGroups {
    proxied: BTreeSet<RouteGroup>,
}

impl RouteGroups {
    /// Keine Gruppe geht zum Node-Server: alles, was Rust kann, beantwortet Rust
    pub fn none() -> Self {
        Self::default()
    }

    /// Gruppen aus `NEXUS_PROXY_GROUPS=a,b`; unbekannte Namen werden übergangen
    pub fn from_env(value: Option<&str>) -> Self {
        let proxied = value
            .unwrap_or_default()
            .split(',')
            .filter_map(RouteGroup::parse)
            .collect();
        Self { proxied }
    }

    /// Soll diese Gruppe zum Node-Server gehen?
    pub fn contains(&self, group: RouteGroup) -> bool {
        self.proxied.contains(&group)
    }

    /// Registriert Rust die Routen dieser Gruppe? (Umkehrung von [`RouteGroups::contains`])
    pub fn enabled(&self, group: RouteGroup) -> bool {
        !self.contains(group)
    }

    /// Die Namen der weitergeleiteten Gruppen, für die Logzeile beim Start
    pub fn names(&self) -> Vec<&'static str> {
        self.proxied.iter().map(|group| group.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jede_gruppe_hat_einen_eindeutigen_namen_und_wird_wiedergefunden() {
        let mut namen = BTreeSet::new();
        for group in RouteGroup::ALL {
            assert!(namen.insert(group.as_str()), "{group} doppelt");
            assert_eq!(RouteGroup::parse(group.as_str()), Some(group));
        }
        assert_eq!(namen.len(), RouteGroup::ALL.len());
        assert_eq!(RouteGroup::parse("gibtesnicht"), None);
    }

    #[test]
    fn ohne_notschalter_ist_jede_gruppe_aktiv() {
        let groups = RouteGroups::none();
        for group in RouteGroup::ALL {
            assert!(groups.enabled(group), "{group}");
        }
        assert_eq!(RouteGroups::from_env(None), RouteGroups::none());
        assert_eq!(RouteGroups::from_env(Some("")), RouteGroups::none());
    }

    #[test]
    fn genannte_gruppen_gehen_zum_node_server() {
        let groups = RouteGroups::from_env(Some("db-tickets, basis ,gibtesnicht"));
        assert!(groups.contains(RouteGroup::Basis));
        assert!(groups.contains(RouteGroup::DbTickets));
        assert!(!groups.enabled(RouteGroup::Basis));
        assert!(groups.enabled(RouteGroup::Docs));
        assert_eq!(groups.names(), ["basis", "db-tickets"]);
    }
}
