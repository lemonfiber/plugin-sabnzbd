use std::collections::BTreeMap;

use async_trait::async_trait;
use lemonfiber_manifest::Date;
use lemonfiber_ports::service::{Failure, Recorded, Standing, UsenetAccount, UsenetAccounts};
use serde::Deserialize;

use super::{Sabnzbd, FULLSTATUS, GET_CONFIG, SECTION, SERVER_STATS, SKIP_DASHBOARD};

const SERVERS: &str = "servers";

const UNITS: [(&str, u64); 6] = [
    ("", 1),
    ("K", 1 << 10),
    ("M", 1 << 20),
    ("G", 1 << 30),
    ("T", 1 << 40),
    ("P", 1 << 50),
];

const DECIMAL: u64 = 10;

#[derive(Deserialize)]
struct Configured {
    config: Servers,
}

#[derive(Deserialize)]
struct Servers {
    #[serde(default)]
    servers: Vec<Server>,
}

#[derive(Deserialize)]
struct Server {
    name: String,
    #[serde(default)]
    displayname: String,
    #[serde(default = "in_use")]
    enable: i64,
    #[serde(default)]
    quota: String,
    #[serde(default)]
    usage_at_start: i64,
    #[serde(default)]
    expire_date: String,
}

const fn in_use() -> i64 {
    1
}

#[derive(Deserialize)]
pub(super) struct Measured {
    #[serde(default)]
    servers: BTreeMap<String, Pulled>,
}

#[derive(Deserialize)]
struct Pulled {
    #[serde(default)]
    total: u64,
    #[serde(default)]
    daily: BTreeMap<String, u64>,
}

#[derive(Deserialize)]
struct Live {
    status: Statuses,
}

#[derive(Deserialize)]
struct Statuses {
    #[serde(default)]
    servers: Vec<Serving>,
}

#[derive(Deserialize)]
struct Serving {
    #[serde(default)]
    servername: String,
    #[serde(default = "in_rotation")]
    serveractive: bool,
    #[serde(default)]
    serveractiveconn: i64,
    #[serde(default)]
    servertotalconn: i64,
    #[serde(default)]
    servererror: String,
}

const fn in_rotation() -> bool {
    true
}

#[async_trait]
impl UsenetAccounts for Sabnzbd {
    async fn accounts(&self) -> Result<Vec<UsenetAccount>, Failure> {
        let configured: Configured = self
            .read(
                GET_CONFIG,
                &[(SECTION, SERVERS)],
                "the Usenet accounts could not be read",
            )
            .await?;
        let measured = self.measured().await?;
        let live: Live = self
            .read(
                FULLSTATUS,
                &[SKIP_DASHBOARD],
                "how the accounts are being served could not be read",
            )
            .await?;
        Ok(configured
            .config
            .servers
            .into_iter()
            .map(|server| {
                let pulled = measured.servers.get(&server.name);
                account_of(server, pulled, &live.status.servers)
            })
            .collect())
    }
}

impl Sabnzbd {
    pub(super) async fn measured(&self) -> Result<Measured, Failure> {
        self.read(
            SERVER_STATS,
            &[],
            "the account statistics could not be read",
        )
        .await
    }
}

fn account_of(server: Server, pulled: Option<&Pulled>, live: &[Serving]) -> UsenetAccount {
    let name = if server.displayname.trim().is_empty() {
        server.name
    } else {
        server.displayname
    };
    let standing = live
        .iter()
        .find(|serving| serving.servername == name)
        .map(standing_of);
    UsenetAccount {
        name,
        enabled: server.enable != 0,
        quota: recorded_quota(&server.quota, server.usage_at_start),
        downloaded: pulled.map_or(0, |pulled| pulled.total),
        daily: pulled.map(daily_of).unwrap_or_default(),
        expires_on: Date::parse(server.expire_date.trim()),
        standing,
    }
}

fn standing_of(serving: &Serving) -> Standing {
    let words = serving.servererror.trim();
    Standing {
        ready: u64::try_from(serving.serveractiveconn).unwrap_or(0),
        configured: u64::try_from(serving.servertotalconn).unwrap_or(0),
        serving: serving.serveractive,
        trouble: (!words.is_empty()).then(|| words.to_owned()),
    }
}

fn recorded_quota(quota: &str, usage_at_start: i64) -> Option<Recorded> {
    let cap = size_of(quota).filter(|cap| *cap > 0)?;
    Some(Recorded {
        cap,
        from: u64::try_from(usage_at_start).unwrap_or(0),
    })
}

fn size_of(text: &str) -> Option<u64> {
    let text = text.trim();
    let split = text
        .find(|letter: char| letter.is_ascii_alphabetic())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let unit = unit.trim().to_ascii_uppercase();
    let unit = unit.trim_end_matches("IB");
    let scale = UNITS
        .iter()
        .find(|(letter, _)| *letter == unit)
        .map(|(_, scale)| *scale)?;
    let (whole, fraction) = number.trim().split_once('.').unwrap_or((number.trim(), ""));
    let bytes = whole.trim().parse::<u64>().ok()?.checked_mul(scale)?;
    if fraction.is_empty() {
        return Some(bytes);
    }
    let places = u32::try_from(fraction.len()).ok()?;
    let scaled = fraction.parse::<u64>().ok()?.checked_mul(scale)?;
    bytes.checked_add(scaled / DECIMAL.checked_pow(places)?)
}

pub(super) fn daily_across(measured: &Measured) -> Vec<(Date, u64)> {
    measured.servers.values().flat_map(daily_of).collect()
}

fn daily_of(pulled: &Pulled) -> Vec<(Date, u64)> {
    pulled
        .daily
        .iter()
        .filter_map(|(day, bytes)| Date::parse(day).map(|day| (day, *bytes)))
        .collect()
}

#[cfg(test)]
mod tests;
