use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_ports::service::{Download, Failure, Transfers};
use serde::Deserialize;

use super::{Sabnzbd, DOWNLOADING, KILOBYTE, QUEUE};

const MEGABYTE: u64 = KILOBYTE * KILOBYTE;

const MOST_PERCENT: u8 = 100;

const SIXTY: u64 = 60;

#[derive(Deserialize)]
struct Queued {
    queue: Queue,
}

#[derive(Deserialize)]
struct Queue {
    #[serde(default)]
    kbpersec: String,
    #[serde(default)]
    slots: Vec<Slot>,
}

#[derive(Deserialize)]
struct Slot {
    filename: String,
    percentage: String,
    status: String,
    timeleft: String,
    #[serde(default)]
    mbleft: String,
}

#[async_trait]
impl Transfers for Sabnzbd {
    async fn transfers(&self) -> Result<Vec<Download>, Failure> {
        let queued: Queued = self.read(QUEUE, &[], "the queue could not be read").await?;
        let speed = bytes_per_second(&queued.queue.kbpersec);
        Ok(queued
            .queue
            .slots
            .into_iter()
            .map(|slot| download_of(slot, speed))
            .collect())
    }
}

fn download_of(slot: Slot, active_speed: Option<u64>) -> Download {
    let speed = if slot.status == DOWNLOADING {
        active_speed
    } else {
        Some(0)
    };
    Download {
        name: slot.filename,
        progress: slot
            .percentage
            .trim()
            .parse::<u8>()
            .unwrap_or(0)
            .min(MOST_PERCENT),
        speed,
        eta: seconds_left(&slot.timeleft)
            .filter(|left| *left > 0)
            .map(Duration::from_secs),
        remaining: whole(&slot.mbleft).map(|mb| mb.saturating_mul(MEGABYTE)),
    }
}

pub(super) fn bytes_per_second(kbpersec: &str) -> Option<u64> {
    whole(kbpersec).map(|kb| kb.saturating_mul(KILOBYTE))
}

fn whole(decimal: &str) -> Option<u64> {
    let whole = decimal.split_once('.').map_or(decimal, |(whole, _)| whole);
    whole.trim().parse::<u64>().ok()
}

fn seconds_left(timeleft: &str) -> Option<u64> {
    if timeleft.is_empty() {
        return None;
    }
    timeleft.split(':').try_fold(0_u64, |total, field| {
        let value: u64 = field.trim().parse().ok()?;
        Some(total.saturating_mul(SIXTY).saturating_add(value))
    })
}

#[cfg(test)]
mod tests;
