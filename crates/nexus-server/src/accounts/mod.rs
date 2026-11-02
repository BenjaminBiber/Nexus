//! Claude-Accounts im Server: Anmeldung, Plugin und Abo-Limits je Account. Portiert aus
//! `packages/server/src/usage/Account*.ts`.
//!
//! Der Speicher der Accounts selbst liegt im Fachkern
//! ([`nexus_core::accounts::AccountStore`]); hier kommt alles dazu, was den Rechner braucht:
//! der macOS-Schlüsselbund, der OAuth-Endpunkt von Anthropic und `claude auth login`.

pub mod login;
pub mod plugin;
pub mod usage;

pub use login::{AccountLoginDto, AccountLoginService, LoginProcess, LoginSpawner};
pub use plugin::AccountPluginInstaller;
pub use usage::{
    no_keychain, system_keychain, AccountInfoDto, AccountList, AccountUsageEntryDto,
    AccountUsageOptions, AccountUsageService, Keychain,
};
