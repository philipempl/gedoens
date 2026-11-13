//! Adapter für die Schnittstellen der Anwendungsschicht.
//!
//! Hier und nur hier werden Programme gestartet und Dateien angefasst.

mod age_keyring;
mod fs_workspace;
mod git_cli;
mod json_tree;
mod locate;
mod process;
mod sops_cli;
mod sops_config;

use std::sync::Arc;

use gedoens_application::Gedoens;

pub use age_keyring::AgeKeyRing;
pub use fs_workspace::FsWorkspace;
pub use git_cli::GitCli;
pub use sops_cli::SopsCli;
pub use sops_config::SopsConfigFile;

/// Verdrahtet die Anwendung mit den echten Adaptern.
pub fn build() -> Gedoens {
    let keys = Arc::new(AgeKeyRing::default());
    Gedoens::new(
        Arc::new(SopsCli::locate().with_keys(keys.clone())),
        Arc::new(FsWorkspace),
        Arc::new(SopsConfigFile),
        Arc::new(GitCli::locate()),
        keys,
    )
}
