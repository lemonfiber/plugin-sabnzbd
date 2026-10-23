use async_trait::async_trait;
use lemonfiber_manifest::Date;
use lemonfiber_ports::service::{Failure, Metering, Moved};

use super::accounts::daily_across;
use super::Sabnzbd;

#[async_trait]
impl Metering for Sabnzbd {
    async fn moved(&self, month: &str) -> Result<Moved, Failure> {
        let measured = self.measured().await?;
        Ok(Moved {
            down: pulled_in(&daily_across(&measured), month),
            up: 0,
            since_start: false,
        })
    }
}

fn pulled_in(daily: &[(Date, u64)], month: &str) -> u64 {
    daily
        .iter()
        .filter(|(day, _)| falls_in(*day, month))
        .map(|(_, bytes)| *bytes)
        .fold(0, u64::saturating_add)
}

fn falls_in(day: Date, month: &str) -> bool {
    written(day) == month.trim()
}

fn written(day: Date) -> String {
    format!("{:04}-{:02}", day.year, day.month)
}

#[cfg(test)]
mod tests;
