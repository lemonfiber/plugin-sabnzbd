use lemonfiber_ports::service::{Failure, Hours};
use serde::Deserialize;

use super::{Sabnzbd, GET_CONFIG, KEYWORD, MISC, SECTION};

const EVERY_DAY: &str = "1234567";

pub(super) const RATE: &str = "speedlimit";

const ENABLED: &str = "1";

const SCHEDLINES: &str = "schedlines";

const ACTION_FIELD: usize = 4;

const ARGUMENT_FIELD: usize = 5;

const ADD: &str = "scheduling/addSchedule";

const DELETE: &str = "scheduling/delSchedule";

#[derive(Deserialize)]
struct Held {
    config: Sections,
}

#[derive(Deserialize)]
struct Sections {
    misc: Schedule,
}

#[derive(Deserialize)]
struct Schedule {
    #[serde(default)]
    schedlines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Turn {
    pub(super) hour: u8,
    pub(super) minute: u8,
    pub(super) figure: String,
}

impl Turn {
    fn stored(&self) -> String {
        format!(
            "{ENABLED} {} {} {EVERY_DAY} {RATE} {}",
            self.minute, self.hour, self.figure
        )
    }
}

impl Sabnzbd {
    pub(super) async fn schedule_lines(&self) -> Result<Vec<String>, Failure> {
        let held: Held = self
            .read(
                GET_CONFIG,
                &[(SECTION, MISC), (KEYWORD, SCHEDLINES)],
                "the schedule could not be read",
            )
            .await?;
        Ok(held.config.misc.schedlines)
    }

    pub(super) async fn keeping(&self, turns: &[Turn]) -> Result<Vec<String>, Failure> {
        let wanted: Vec<String> = turns.iter().map(Turn::stored).collect();
        let held = self.schedule_lines().await?;
        for line in held.iter().filter(|line| is_rate(line)) {
            if !wanted.contains(line) {
                self.page(DELETE, &[("line", line.as_str())]).await?;
            }
        }
        for turn in turns.iter().filter(|turn| !held.contains(&turn.stored())) {
            self.page(
                ADD,
                &[
                    ("minute", &turn.minute.to_string()),
                    ("hour", &turn.hour.to_string()),
                    ("daysofweek", EVERY_DAY),
                    ("action", RATE),
                    ("arguments", &turn.figure),
                ],
            )
            .await?;
        }
        let after = self.schedule_lines().await?;
        let rates = after.iter().filter(|line| is_rate(line)).count();
        if rates != wanted.len() || !wanted.iter().all(|line| after.contains(line)) {
            return Err(self.endpoint.refused(&format!(
                "the household's hours were written and the client is keeping {rates} rate \
                 instructions instead"
            )));
        }
        Ok(after)
    }
}

fn is_rate(line: &str) -> bool {
    field(line, ACTION_FIELD) == Some(RATE)
}

fn field(line: &str, at: usize) -> Option<&str> {
    line.split_whitespace().nth(at)
}

pub(super) fn side(lines: &[String], limited: bool) -> Option<Hours> {
    let mut figures: Vec<&str> = lines
        .iter()
        .filter(|line| is_rate(line) && field(line, 0) == Some(ENABLED))
        .filter_map(|line| field(line, ARGUMENT_FIELD))
        .collect();
    figures.sort_unstable();
    figures.dedup();
    (figures.len() > 1).then_some(if limited { Hours::Active } else { Hours::Quiet })
}

#[cfg(test)]
mod tests;
