//! Claude-Accounts des Dispatchers: Modell, Pfade von Claude Code und der Speicher
//! `~/.nexus/accounts.json`. Portiert aus `packages/core/src/accounts`.

pub mod account;
pub mod paths;
pub mod store;
pub mod tokens;

pub use account::{
    account_limit, is_account_id, standard_account, ClaudeAccount, MAX_ACCOUNT_NAME_LENGTH,
    STANDARD_ACCOUNT_ID,
};
pub use paths::{has_plugin, keychain_service, AccountFiles, AccountPaths};
pub use store::{AccountPatch, AccountStore, NewAccount};
pub use tokens::AccountTokenStore;
