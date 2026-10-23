//! SABnzbd, the Usenet client, asked through its own API with the key lemonfiber wrote
//! for this adapter.

use std::sync::Arc;

use lemonfiber_adapter_kit::credential::Holding;
use lemonfiber_adapter_kit::endpoint::{form_encoded, Endpoint};
use lemonfiber_ports::http::{Http, Method, Request};
use lemonfiber_ports::service::Failure;

mod accounts;
mod fetching;
mod filing;
mod metering;
mod queue;
mod scheduling;
mod throttling;

/// What every failure is blamed on.
pub const SERVICE: &str = "sabnzbd";

const KEY_FIELD: &str = "apikey";

const MODE: &str = "mode";

const OUTPUT: (&str, &str) = ("output", "json");

const VERSION: &str = "version";

const QUEUE: &str = "queue";

const KILOBYTE: u64 = 1024;

const GET_CONFIG: &str = "get_config";

const SERVER_STATS: &str = "server_stats";

const FULLSTATUS: &str = "fullstatus";

const SKIP_DASHBOARD: (&str, &str) = ("skip_dashboard", "1");

const SECTION: &str = "section";

const KEYWORD: &str = "keyword";

const MISC: &str = "misc";

const DOWNLOADING: &str = "Downloading";

/// A client for one SABnzbd, presenting the key `held` supplies on every call.
pub struct Sabnzbd {
    endpoint: Endpoint,
    held: Arc<dyn Holding>,
}

impl Sabnzbd {
    /// The SABnzbd reached at `base` over `http`, asked with the key `held` supplies.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>, held: Arc<dyn Holding>) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, SERVICE),
            held,
        }
    }

    /// Whether SABnzbd answers what it answers without a key: its version.
    pub async fn answers(&self) -> bool {
        let fields = form_encoded(&[(MODE, VERSION), OUTPUT]);
        self.endpoint
            .send(&get(self.endpoint.url(&format!("/api?{fields}"))))
            .await
            .is_ok_and(|response| response.is_success())
    }

    fn key(&self) -> Result<String, Failure> {
        self.held.key().map_err(|why| {
            self.endpoint
                .refused(&format!("this adapter holds no key for SABnzbd: {why}"))
        })
    }

    async fn read<T: serde::de::DeserializeOwned>(
        &self,
        mode: &str,
        fields: &[(&str, &str)],
        whenever: &str,
    ) -> Result<T, Failure> {
        let key = self.key()?;
        let mut asked = vec![(MODE, mode)];
        asked.extend_from_slice(fields);
        asked.extend([OUTPUT, (KEY_FIELD, key.as_str())]);
        let request = get(self.endpoint.url(&format!("/api?{}", form_encoded(&asked))));
        let response = self.endpoint.send(&request).await?;
        self.endpoint.decode(&response, whenever)
    }

    async fn page(&self, path: &str, fields: &[(&str, &str)]) -> Result<(), Failure> {
        let key = self.key()?;
        let mut asked = fields.to_vec();
        asked.push((KEY_FIELD, key.as_str()));
        let request = get(self
            .endpoint
            .url(&format!("/config/{path}?{}", form_encoded(&asked))));
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)
    }

    async fn asked(
        &self,
        mode: &str,
        fields: &[(&str, &str)],
        whenever: &str,
    ) -> Result<(), Failure> {
        let wrote: Wrote = self.read(mode, fields, whenever).await?;
        if wrote.status {
            Ok(())
        } else {
            Err(self.endpoint.refused(&format!(
                "{whenever}: the client answered that it would not"
            )))
        }
    }
}

fn get(url: String) -> Request {
    Request {
        method: Method::Get,
        url,
        headers: Vec::new(),
        body: None,
        pinned: None,
    }
}

#[derive(serde::Deserialize)]
struct Wrote {
    #[serde(default)]
    status: bool,
}

#[cfg(test)]
mod tests;
