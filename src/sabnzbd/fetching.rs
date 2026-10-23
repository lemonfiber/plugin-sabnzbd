use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Fetching, Pulling};
use serde::Deserialize;

use super::{Sabnzbd, QUEUE};

#[derive(Deserialize)]
struct Halted {
    queue: Paused,
}

#[derive(Deserialize)]
struct Paused {
    #[serde(default)]
    paused: bool,
}

#[async_trait]
impl Fetching for Sabnzbd {
    async fn pulling(&self) -> Result<Pulling, Failure> {
        let held: Halted = self
            .read(QUEUE, &[], "whether it is fetching could not be read")
            .await?;
        let fetching = !held.queue.paused;
        Ok(Pulling::of(fetching, fetching))
    }

    async fn stop(&self) -> Result<Pulling, Failure> {
        self.asked("pause", &[], "the client could not be stopped")
            .await?;
        self.pulling().await
    }

    async fn resume(&self) -> Result<Pulling, Failure> {
        self.asked("resume", &[], "the client could not be started")
            .await?;
        self.pulling().await
    }
}

#[cfg(test)]
mod tests;
