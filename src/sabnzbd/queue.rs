use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_ports::service::{Download, Failure, TransferState, Transfers};
use serde::Deserialize;

use super::{Sabnzbd, DOWNLOADING, KILOBYTE, QUEUE};

const MEGABYTE: u64 = KILOBYTE * KILOBYTE;

const MOST_PERCENT: u8 = 100;

const SIXTY: u64 = 60;

const NO_CATEGORY: &str = "None";

const STATES: [(&str, TransferState); 5] = [
    (DOWNLOADING, TransferState::Downloading),
    ("Fetching", TransferState::Downloading),
    ("Queued", TransferState::Queued),
    ("Propagating", TransferState::Queued),
    ("Paused", TransferState::Paused),
];

#[derive(Deserialize)]
struct Queued {
    queue: Queue,
}

#[derive(Deserialize)]
struct Queue {
    #[serde(default)]
    kbpersec: String,
    #[serde(default)]
    paused: bool,
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
    nzo_id: Option<String>,
    cat: Option<String>,
    time_added: Option<i64>,
}

#[async_trait]
impl Transfers for Sabnzbd {
    async fn transfers(&self) -> Result<Vec<Download>, Failure> {
        let queued: Queued = self.read(QUEUE, &[], "the queue could not be read").await?;
        let speed = bytes_per_second(&queued.queue.kbpersec);
        let paused = queued.queue.paused;
        Ok(queued
            .queue
            .slots
            .into_iter()
            .map(|slot| download_of(slot, speed, paused))
            .collect())
    }
}

fn download_of(slot: Slot, active_speed: Option<u64>, paused: bool) -> Download {
    let speed = if slot.status == DOWNLOADING {
        active_speed
    } else {
        Some(0)
    };
    let state = if paused {
        Some(TransferState::Paused)
    } else {
        state_of(&slot.status)
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
        id: slot.nzo_id.filter(|id| !id.is_empty()),
        category: slot
            .cat
            .filter(|category| !category.is_empty() && category != NO_CATEGORY),
        state,
        added: slot
            .time_added
            .and_then(|added| u64::try_from(added).ok())
            .filter(|added| *added > 0),
    }
}

fn state_of(status: &str) -> Option<TransferState> {
    STATES
        .iter()
        .find(|(named, _)| *named == status)
        .map(|(_, said)| *said)
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
