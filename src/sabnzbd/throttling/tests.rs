use lemonfiber_ports::service::{Failure, Hours, Rates, Throttling, Wanted, Window};

use super::super::tests::{asked, client, BASE, KEY};
use super::{absolute, kilobytes, on_the_clock, turns};

const LIMITED: &str = r#"{"queue":{"speedlimit_abs":"1048576","kbpersec":"512.5"}}"#;

const FREE: &str = r#"{"queue":{"speedlimit_abs":"","kbpersec":"0.00"}}"#;

const UNSCHEDULED: &str = r#"{"config":{"misc":{"schedlines":[]}}}"#;

const SCHEDULED: &str = r#"{"config":{"misc":{"schedlines":[
    "1 0 7 1234567 speedlimit 1024k","1 0 23 1234567 speedlimit 0"]}}}"#;

const DID: &str = r#"{"status":true}"#;

fn a_day() -> Window {
    Window {
        from_hour: 7,
        from_minute: 0,
        to_hour: 23,
        to_minute: 0,
    }
}

fn a_household() -> Wanted {
    Wanted {
        active: Rates {
            down: Some(1_048_576),
            up: Some(1),
        },
        quiet: Rates::default(),
        window: Some(a_day()),
    }
}

#[tokio::test]
async fn a_usenet_client_has_a_download_limit_and_no_upload_to_have_one() {
    let held = client(vec![(200, LIMITED), (200, UNSCHEDULED)])
        .0
        .throttled()
        .await;
    assert!(held.is_ok_and(|held| held.rates.down == Some(1_048_576)
        && held.rates.up.is_none()
        && !held.uploads
        && held.hours.is_none()));
}

#[tokio::test]
async fn nothing_holding_it_back_reads_as_no_limit_rather_than_a_limit_of_nothing() {
    let held = client(vec![(200, FREE), (200, UNSCHEDULED)])
        .0
        .throttled()
        .await;
    assert!(held.is_ok_and(|held| held.rates == Rates::default()));
}

#[tokio::test]
async fn a_client_keeping_the_household_s_hours_says_which_side_of_them_it_is_on() {
    let awake = client(vec![(200, LIMITED), (200, SCHEDULED)])
        .0
        .throttled()
        .await;
    assert!(awake.is_ok_and(|held| held.hours == Some(Hours::Active)));
    let asleep = client(vec![(200, FREE), (200, SCHEDULED)])
        .0
        .throttled()
        .await;
    assert!(asleep.is_ok_and(|held| held.hours == Some(Hours::Quiet)));
}

#[tokio::test]
async fn a_window_is_written_into_the_client_s_own_scheduler_and_no_rate_is_set() {
    let (sabnzbd, http) = client(vec![
        (200, UNSCHEDULED),
        (200, "<html/>"),
        (200, "<html/>"),
        (200, SCHEDULED),
        (200, LIMITED),
        (200, SCHEDULED),
    ]);
    let held = sabnzbd.restrain(&a_household()).await;
    assert!(held.is_ok_and(|held| held.hours == Some(Hours::Active)));
    let sent = asked(&http).join(" ");
    assert!(sent.contains("addSchedule"), "{sent}");
    assert!(!sent.contains("name=speedlimit"), "{sent}");
}

#[tokio::test]
async fn a_client_with_no_window_is_held_to_the_active_rate_around_the_clock() {
    let wanted = Wanted {
        window: None,
        ..a_household()
    };
    let (sabnzbd, http) = client(vec![
        (200, UNSCHEDULED),
        (200, UNSCHEDULED),
        (200, DID),
        (200, LIMITED),
        (200, UNSCHEDULED),
    ]);
    let held = sabnzbd.restrain(&wanted).await;
    assert!(held.is_ok_and(|held| held.rates.down == Some(1_048_576) && held.hours.is_none()));
    assert_eq!(
        asked(&http).get(2).map(String::as_str),
        Some(
            format!("{BASE}/api?mode=config&name=speedlimit&value=1024k&output=json&apikey={KEY}")
                .as_str()
        )
    );
}

#[tokio::test]
async fn a_client_that_answers_that_it_did_not_take_the_limit_is_a_failure() {
    let refused = client(vec![
        (200, UNSCHEDULED),
        (200, UNSCHEDULED),
        (200, r#"{"status":false}"#),
    ])
    .0
    .restrain(&Wanted {
        active: Rates::default(),
        quiet: Rates::default(),
        window: None,
    })
    .await;
    assert!(matches!(
        refused,
        Err(Failure::Refused { detail, .. }) if detail.contains("the rate limit could not be set")
    ));
}

#[tokio::test]
async fn a_schedule_that_could_not_be_written_is_a_failure_before_any_rate_is_set() {
    assert!(client(vec![(403, "no")])
        .0
        .restrain(&a_household())
        .await
        .is_err());
}

#[tokio::test]
async fn a_window_off_the_clock_is_refused_before_anything_is_asked() {
    for window in [
        Window {
            from_hour: 24,
            ..a_day()
        },
        Window {
            to_hour: 99,
            ..a_day()
        },
        Window {
            from_minute: 60,
            ..a_day()
        },
        Window {
            to_minute: 255,
            ..a_day()
        },
    ] {
        let (sabnzbd, http) = client(Vec::new());
        let refused = sabnzbd
            .restrain(&Wanted {
                window: Some(window),
                ..a_household()
            })
            .await;
        assert!(matches!(refused, Err(Failure::Refused { .. })));
        assert!(http.requests().is_empty());
    }
    assert!(on_the_clock(Window {
        from_hour: 23,
        from_minute: 59,
        to_hour: 0,
        to_minute: 0,
    }));
}

#[tokio::test]
async fn what_it_is_moving_comes_off_the_queue_it_already_answers_with() {
    assert_eq!(
        client(vec![(200, LIMITED)]).0.moving().await.ok(),
        Some(Rates {
            down: Some(512 * 1024),
            up: None
        })
    );
    assert!(client(vec![(200, "not json")]).0.moving().await.is_err());
    assert!(client(vec![(200, "not json")]).0.throttled().await.is_err());
}

#[test]
fn a_window_whose_two_sides_come_to_one_figure_is_no_schedule_at_all() {
    let flat = Wanted {
        active: Rates::default(),
        ..a_household()
    };
    assert!(turns(&flat).is_empty());
    assert_eq!(turns(&a_household()).len(), 2);
    assert!(turns(&Wanted {
        window: None,
        ..a_household()
    })
    .is_empty());
}

#[test]
fn the_clients_own_zero_is_no_limit_rather_than_a_limit_of_nothing() {
    assert_eq!(absolute(""), None);
    assert_eq!(absolute("0"), None);
    assert_eq!(absolute("not a number"), None);
    assert_eq!(absolute(" 1048576 "), Some(1_048_576));
}

#[test]
fn a_limit_is_written_with_its_unit_so_it_is_not_read_as_a_percentage() {
    assert_eq!(kilobytes(Some(2 * 1024 * 1024)), "2048k");
    assert_eq!(kilobytes(None), "0");
}

#[test]
fn a_limit_below_the_smallest_the_client_holds_becomes_that_rather_than_none() {
    assert_eq!(kilobytes(Some(1)), "1k");
    assert_eq!(kilobytes(Some(1023)), "1k");
    assert_eq!(kilobytes(Some(1025)), "2k");
}
