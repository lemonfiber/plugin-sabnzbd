use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Rates, Throttled, Throttling, Wanted, Window};
use serde::Deserialize;

use super::queue::bytes_per_second;
use super::scheduling::{side, Turn, RATE};
use super::{Sabnzbd, KILOBYTE, QUEUE};

const LAST_HOUR: u8 = 23;

const LAST_MINUTE: u8 = 59;

const UNLIMITED: &str = "0";

#[derive(Deserialize)]
struct Limited {
    queue: Held,
}

#[derive(Deserialize)]
struct Held {
    #[serde(default)]
    speedlimit_abs: String,
    #[serde(default)]
    kbpersec: String,
}

#[async_trait]
impl Throttling for Sabnzbd {
    async fn throttled(&self) -> Result<Throttled, Failure> {
        let held: Limited = self
            .read(QUEUE, &[], "the rate limit could not be read")
            .await?;
        let down = absolute(&held.queue.speedlimit_abs);
        Ok(Throttled {
            rates: Rates { down, up: None },
            uploads: false,
            hours: side(&self.schedule_lines().await?, down.is_some()),
        })
    }

    async fn restrain(&self, wanted: &Wanted) -> Result<Throttled, Failure> {
        if !wanted.window.is_none_or(on_the_clock) {
            return Err(self
                .endpoint
                .refused("that window is not hours and minutes of a day"));
        }
        let turns = turns(wanted);
        self.keeping(&turns).await?;
        if turns.is_empty() {
            self.asked(
                "config",
                &[("name", RATE), ("value", &kilobytes(wanted.active.down))],
                "the rate limit could not be set",
            )
            .await?;
        }
        self.throttled().await
    }

    async fn moving(&self) -> Result<Rates, Failure> {
        let held: Limited = self
            .read(QUEUE, &[], "the current transfer rate could not be read")
            .await?;
        Ok(Rates {
            down: bytes_per_second(&held.queue.kbpersec),
            up: None,
        })
    }
}

fn turns(wanted: &Wanted) -> Vec<Turn> {
    let Some(window) = wanted.window else {
        return Vec::new();
    };
    let (active, quiet) = (kilobytes(wanted.active.down), kilobytes(wanted.quiet.down));
    if active == quiet {
        return Vec::new();
    }
    vec![
        Turn {
            hour: window.from_hour,
            minute: window.from_minute,
            figure: active,
        },
        Turn {
            hour: window.to_hour,
            minute: window.to_minute,
            figure: quiet,
        },
    ]
}

fn on_the_clock(window: Window) -> bool {
    window.from_hour <= LAST_HOUR
        && window.to_hour <= LAST_HOUR
        && window.from_minute <= LAST_MINUTE
        && window.to_minute <= LAST_MINUTE
}

fn absolute(figure: &str) -> Option<u64> {
    figure.trim().parse::<u64>().ok().filter(|held| *held > 0)
}

fn kilobytes(limit: Option<u64>) -> String {
    limit.map_or_else(
        || UNLIMITED.to_owned(),
        |bytes| format!("{}k", bytes.div_ceil(KILOBYTE).max(1)),
    )
}

#[cfg(test)]
mod tests;
