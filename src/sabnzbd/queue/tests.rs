use std::time::Duration;

use lemonfiber_ports::service::{Failure, Transfers};

use super::super::tests::client;
use super::{bytes_per_second, seconds_left};

const QUEUE: &str = r#"{"queue":{"kbpersec":"2048.5","slots":[
    {"filename":"Active.nzb","percentage":"45","status":"Downloading","timeleft":"0:10:00","mbleft":"1024.5"},
    {"filename":"Waiting.nzb","percentage":"0","status":"Queued","timeleft":"0:00:00","mbleft":"512"}
]}}"#;

const QUEUE_EDGES: &str = r#"{"queue":{"kbpersec":"nan","slots":[
    {"filename":"BadSpeed.nzb","percentage":"oops","status":"Downloading","timeleft":"","mbleft":"oops"},
    {"filename":"Paused.nzb","percentage":"200","status":"Paused","timeleft":"1:bad:3"}
]}}"#;

#[tokio::test]
async fn the_active_slot_carries_the_queue_speed_and_the_rest_read_zero() {
    let (sabnzbd, _) = client(vec![(200, QUEUE)]);
    let transfers = sabnzbd.transfers().await.unwrap_or_default();
    assert_eq!(transfers.len(), 2);
    assert!(matches!(
        transfers.first(),
        Some(t) if t.name == "Active.nzb"
            && t.progress == 45
            && t.speed == Some(2048 * 1024)
            && t.eta == Some(Duration::from_secs(600))
            && t.remaining == Some(1024 * 1024 * 1024)
    ));
    assert!(matches!(
        transfers.get(1),
        Some(t) if t.progress == 0 && t.speed == Some(0) && t.eta.is_none()
            && t.remaining == Some(512 * 1024 * 1024)
    ));
}

#[tokio::test]
async fn unreadable_figures_degrade_rather_than_fail_the_read() {
    let (sabnzbd, _) = client(vec![(200, QUEUE_EDGES)]);
    let transfers = sabnzbd.transfers().await.unwrap_or_default();
    assert_eq!(transfers.len(), 2);
    assert!(matches!(
        transfers.first(),
        Some(t) if t.progress == 0 && t.speed.is_none() && t.eta.is_none()
            && t.remaining.is_none()
    ));
    assert!(matches!(
        transfers.get(1),
        Some(t) if t.progress == 100 && t.speed == Some(0) && t.eta.is_none()
            && t.remaining.is_none()
    ));
}

#[tokio::test]
async fn a_client_that_will_not_answer_is_unavailable() {
    let (sabnzbd, _) = client(Vec::new());
    assert!(matches!(
        sabnzbd.transfers().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn a_queue_that_will_not_parse_is_refused() {
    let (sabnzbd, _) = client(vec![(200, "not json")]);
    assert!(matches!(
        sabnzbd.transfers().await,
        Err(Failure::Refused { .. })
    ));
}

#[test]
fn a_rate_is_whole_kilobytes_as_bytes() {
    assert_eq!(bytes_per_second("2.9"), Some(2048));
    assert_eq!(bytes_per_second(" 0.00 "), Some(0));
    assert_eq!(bytes_per_second(""), None);
}

#[test]
fn a_time_left_is_every_field_in_base_sixty() {
    assert_eq!(seconds_left("1:00:01"), Some(3601));
    assert_eq!(seconds_left("100:00:00"), Some(360_000));
    assert_eq!(seconds_left(""), None);
    assert_eq!(seconds_left("1:x"), None);
}
