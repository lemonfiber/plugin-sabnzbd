use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::Hours;

use super::super::tests::{asked, client, BASE, KEY};
use super::{is_rate, side, Sabnzbd, Turn};

const THEIRS: [&str; 2] = ["1 0 3 1234567 pause ", "1 15 4 12345 resume "];

const BEFORE: &str = r#"{"config":{"misc":{"schedlines":[
    "1 0 3 1234567 pause ","1 15 4 12345 resume ","1 0 9 1234567 speedlimit 100k"]}}}"#;

const AFTER: &str = r#"{"config":{"misc":{"schedlines":[
    "1 0 3 1234567 pause ","1 15 4 12345 resume ",
    "1 0 7 1234567 speedlimit 5120k","1 30 23 1234567 speedlimit 0"]}}}"#;

fn a_window() -> Vec<Turn> {
    vec![
        Turn {
            hour: 7,
            minute: 0,
            figure: "5120k".to_owned(),
        },
        Turn {
            hour: 23,
            minute: 30,
            figure: "0".to_owned(),
        },
    ]
}

#[tokio::test]
async fn a_line_the_operator_wrote_survives_the_household_s_hours_being_written() {
    let (sabnzbd, http) = client(vec![
        (200, BEFORE),
        (200, "<html/>"),
        (200, "<html/>"),
        (200, "<html/>"),
        (200, AFTER),
    ]);
    let kept = sabnzbd.keeping(&a_window()).await;
    assert!(kept.is_ok_and(|lines| THEIRS
        .iter()
        .all(|theirs| lines.iter().any(|held| held == theirs))));
    let sent = asked(&http);
    assert_eq!(
        sent.get(1).map(String::as_str),
        Some(
            format!(
                "{BASE}/config/scheduling/delSchedule?line=1+0+9+1234567+speedlimit+100k&apikey={KEY}"
            )
            .as_str()
        )
    );
    assert_eq!(
        sent.get(2).map(String::as_str),
        Some(
            format!(
                "{BASE}/config/scheduling/addSchedule?minute=0&hour=7&daysofweek=1234567&action=speedlimit&arguments=5120k&apikey={KEY}"
            )
            .as_str()
        )
    );
    let joined = sent.join(" ");
    assert!(
        !joined.contains("pause") && !joined.contains("resume"),
        "{joined}"
    );
}

#[tokio::test]
async fn a_run_that_wants_what_the_client_already_holds_writes_nothing() {
    let (sabnzbd, http) = client(vec![(200, AFTER), (200, AFTER)]);
    assert!(sabnzbd.keeping(&a_window()).await.is_ok());
    assert!(asked(&http)
        .iter()
        .all(|url| url.contains("mode=get_config&section=misc&keyword=schedlines")));
}

#[tokio::test]
async fn taking_the_window_away_takes_every_rate_line_with_it() {
    let (sabnzbd, http) = client(vec![
        (200, AFTER),
        (200, "<html/>"),
        (200, "<html/>"),
        (
            200,
            r#"{"config":{"misc":{"schedlines":["1 0 3 1234567 pause "]}}}"#,
        ),
    ]);
    assert!(sabnzbd
        .keeping(&[])
        .await
        .is_ok_and(|lines| lines.len() == 1));
    assert_eq!(
        asked(&http)
            .iter()
            .filter(|url| url.contains("delSchedule"))
            .count(),
        2
    );
}

#[tokio::test]
async fn a_schedule_that_does_not_come_back_as_it_was_written_is_a_failure() {
    let (sabnzbd, _) = client(vec![
        (200, BEFORE),
        (200, "<html/>"),
        (200, "<html/>"),
        (200, "<html/>"),
        (200, BEFORE),
    ]);
    assert!(sabnzbd.keeping(&a_window()).await.is_err());
}

#[tokio::test]
async fn a_schedule_holding_one_rate_line_too_many_is_a_failure() {
    let (sabnzbd, _) = client(vec![
        (200, AFTER),
        (
            200,
            r#"{"config":{"misc":{"schedlines":[
            "1 0 7 1234567 speedlimit 5120k","1 30 23 1234567 speedlimit 0",
            "1 0 12 1234567 speedlimit 1k"]}}}"#,
        ),
    ]);
    assert!(sabnzbd.keeping(&a_window()).await.is_err());
}

#[tokio::test]
async fn a_client_that_will_not_say_what_it_is_keeping_is_a_failure() {
    assert!(client(vec![(200, "not json")])
        .0
        .keeping(&[])
        .await
        .is_err());
    assert!(client(vec![(403, "no")]).0.keeping(&[]).await.is_err());
}

#[tokio::test]
async fn a_page_that_refuses_the_line_is_a_failure_rather_than_a_read_back() {
    let (sabnzbd, _) = client(vec![(200, BEFORE), (403, "no")]);
    assert!(sabnzbd.keeping(&a_window()).await.is_err());
}

#[tokio::test]
async fn a_line_is_built_in_the_field_order_the_client_stores_it_in() {
    let http = Fake::always(Answer::reply(200, AFTER));
    let kept = Sabnzbd::new(http, BASE, Arc::new(KEY.to_owned()))
        .keeping(&a_window())
        .await;
    assert!(kept.is_ok());
}

#[test]
fn only_the_action_says_whether_a_line_is_this_errand_s() {
    assert!(is_rate("1 0 7 1234567 speedlimit 5120k"));
    assert!(is_rate("0 0 7 12345 speedlimit 5120k"));
    assert!(!is_rate("1 0 3 1234567 pause "));
    assert!(!is_rate("1 15 4 12345 resume "));
    assert!(!is_rate(""));
}

#[test]
fn a_schedule_that_never_changes_the_rate_puts_the_client_on_no_side_of_the_day() {
    let one = ["1 0 7 1234567 speedlimit 5120k".to_owned()];
    assert_eq!(side(&one, true), None);
    assert_eq!(side(&[], false), None);
    let same = [
        "1 0 7 1234567 speedlimit 5120k".to_owned(),
        "1 30 23 1234567 speedlimit 5120k".to_owned(),
    ];
    assert_eq!(side(&same, true), None);
}

#[test]
fn a_schedule_that_switches_the_rate_puts_it_on_the_side_the_limit_says() {
    let switching = [
        "1 0 7 1234567 speedlimit 5120k".to_owned(),
        "1 30 23 1234567 speedlimit 0".to_owned(),
    ];
    assert_eq!(side(&switching, true), Some(Hours::Active));
    assert_eq!(side(&switching, false), Some(Hours::Quiet));
}

#[test]
fn a_line_the_client_will_not_act_on_is_not_a_switch() {
    let off = [
        "1 0 7 1234567 speedlimit 5120k".to_owned(),
        "0 30 23 1234567 speedlimit 0".to_owned(),
    ];
    assert_eq!(side(&off, true), None);
}
