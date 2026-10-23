//! What this adapter says it is, and the kit that serves its contract.

use std::sync::Arc;

use lemonfiber_adapter_kit::{Kit, Readiness};
use lemonfiber_contract::adapter::{About, Release};
use lemonfiber_contract::capabilities::download;

use crate::sabnzbd::Sabnzbd;

/// The upstream this adapter fronts, as its project names itself.
pub const UPSTREAM: &str = "SABnzbd";

/// The SABnzbd release this adapter's recordings were taken from.
pub const RELEASE: &str = "4.5.5-ls250";

/// The digest of the image that release's recordings were taken from.
pub const DIGEST: &str = "sha256:374051b90f64d107f8658dcd9b9065c28826afe14562647214f45057e119fc04";

/// What this adapter says it is: the contract it speaks and the release it supports.
#[must_use]
pub fn about() -> About {
    About {
        speaks: vec![lemonfiber_contract::spoken(
            download::usenet::CAPABILITY,
            download::usenet::MAJOR,
        )],
        upstream: UPSTREAM.to_owned(),
        releases: vec![Release {
            version: RELEASE.to_owned(),
            digest: DIGEST.to_owned(),
        }],
    }
}

/// The contract, served from `sabnzbd` under `key`, ready while SABnzbd answers.
#[must_use]
pub fn kit(key: String, sabnzbd: Arc<Sabnzbd>) -> Kit {
    let asked = Arc::clone(&sabnzbd);
    let ready: Readiness = Arc::new(move || {
        let asked = Arc::clone(&asked);
        Box::pin(async move { asked.answers().await })
    });
    Kit::new(key, about())
        .ready_when(ready)
        .serving(download::usenet::served(sabnzbd))
}

#[cfg(test)]
mod tests;
