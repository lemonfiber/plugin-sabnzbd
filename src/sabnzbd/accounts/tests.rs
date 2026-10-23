use lemonfiber_manifest::Date;
use lemonfiber_ports::service::{Failure, Recorded, Standing, UsenetAccount, UsenetAccounts};

use super::super::tests::{asked, client, BASE, KEY};
use super::{recorded_quota, size_of};

const SERVERS: &str = r#"{"config":{"servers":[
    {"name":"news.example.com","displayname":"Block 500","enable":1,"quota":"500 G",
     "usage_at_start":100,"expire_date":"2026-09-01","username":"someone","password":"****"},
    {"name":"backup.example.com","displayname":"","enable":0,"quota":"","usage_at_start":0,
     "expire_date":""}
]}}"#;

const STATS: &str = r#"{"total":9,"servers":{"news.example.com":{
    "total":2048,"month":10,"week":5,
    "daily":{"2026-08-14":600,"2026-08-15":400,"not-a-day":999}}}}"#;

const STATUS: &str = r#"{"status":{"servers":[
    {"servername":"Block 500","serveractive":true,"serveractiveconn":2,"servertotalconn":8,
     "servererror":"Too many connections to server news.example.com [502 Too many connections]"}
]}}"#;

const ONE_SERVER: &str = r#"{"config":{"servers":[{"name":"news.example.com"}]}}"#;

const NO_STATS: &str = r#"{"servers":{}}"#;

fn day(year: u16, month: u8, day: u8) -> Date {
    Date { year, month, day }
}

#[tokio::test]
async fn each_account_carries_what_was_recorded_and_what_was_measured() {
    let (sabnzbd, http) = client(vec![(200, SERVERS), (200, STATS), (200, STATUS)]);
    assert_eq!(
        sabnzbd.accounts().await.unwrap_or_default(),
        vec![
            UsenetAccount {
                name: "Block 500".to_owned(),
                enabled: true,
                quota: Some(Recorded {
                    cap: 500 * (1 << 30),
                    from: 100,
                }),
                downloaded: 2048,
                daily: vec![(day(2026, 8, 14), 600), (day(2026, 8, 15), 400)],
                expires_on: Some(day(2026, 9, 1)),
                standing: Some(Standing {
                    ready: 2,
                    configured: 8,
                    serving: true,
                    trouble: Some(
                        "Too many connections to server news.example.com [502 Too many connections]"
                            .to_owned()
                    ),
                }),
            },
            UsenetAccount {
                name: "backup.example.com".to_owned(),
                enabled: false,
                quota: None,
                downloaded: 0,
                daily: Vec::new(),
                expires_on: None,
                standing: None,
            },
        ]
    );
    assert_eq!(
        asked(&http),
        [
            format!("{BASE}/api?mode=get_config&section=servers&output=json&apikey={KEY}"),
            format!("{BASE}/api?mode=server_stats&output=json&apikey={KEY}"),
            format!("{BASE}/api?mode=fullstatus&skip_dashboard=1&output=json&apikey={KEY}"),
        ]
    );
}

#[tokio::test]
async fn a_live_view_with_nothing_filled_in_reads_as_an_account_in_rotation() {
    let bare = r#"{"status":{"servers":[{"servername":"news.example.com"}]}}"#;
    let (sabnzbd, _) = client(vec![(200, ONE_SERVER), (200, NO_STATS), (200, bare)]);
    assert_eq!(
        sabnzbd
            .accounts()
            .await
            .unwrap_or_default()
            .first()
            .and_then(|account| account.standing.clone()),
        Some(Standing {
            ready: 0,
            configured: 0,
            serving: true,
            trouble: None,
        })
    );
}

#[tokio::test]
async fn connection_counts_that_make_no_sense_are_read_as_none_held() {
    let impossible = r#"{"status":{"servers":[{"servername":"news.example.com",
        "serveractiveconn":-4,"servertotalconn":-8,"servererror":"   "}]}}"#;
    let (sabnzbd, _) = client(vec![(200, ONE_SERVER), (200, NO_STATS), (200, impossible)]);
    assert_eq!(
        sabnzbd
            .accounts()
            .await
            .unwrap_or_default()
            .first()
            .and_then(|account| account.standing.clone()),
        Some(Standing {
            ready: 0,
            configured: 0,
            serving: true,
            trouble: None,
        })
    );
}

#[tokio::test]
async fn an_account_with_no_enabled_flag_is_read_as_one_in_use() {
    let (sabnzbd, _) = client(vec![
        (200, ONE_SERVER),
        (200, NO_STATS),
        (200, r#"{"status":{}}"#),
    ]);
    assert_eq!(
        sabnzbd.accounts().await.unwrap_or_default(),
        vec![UsenetAccount {
            name: "news.example.com".to_owned(),
            enabled: true,
            quota: None,
            downloaded: 0,
            daily: Vec::new(),
            expires_on: None,
            standing: None,
        }]
    );
}

#[tokio::test]
async fn accounts_that_cannot_be_read_are_a_failure_rather_than_an_empty_list() {
    assert!(matches!(
        client(vec![(200, "not json")]).0.accounts().await,
        Err(Failure::Refused { .. })
    ));
    assert!(matches!(
        client(vec![(200, SERVERS)]).0.accounts().await,
        Err(Failure::Unavailable { .. })
    ));
    assert!(matches!(
        client(vec![(200, SERVERS), (200, STATS)])
            .0
            .accounts()
            .await,
        Err(Failure::Unavailable { .. })
    ));
}

#[test]
fn a_recorded_size_reads_the_way_the_client_wrote_it() {
    for (written, bytes) in [
        ("1024", Some(1024_u64)),
        ("1K", Some(1 << 10)),
        ("10 M", Some(10 * (1 << 20))),
        ("100G", Some(100 * (1 << 30))),
        ("1.5G", Some(1024 * 1024 * 1024 + 512 * 1024 * 1024)),
        ("2 GiB", Some(2 * (1 << 30))),
        ("2T", Some(2 * (1 << 40))),
        ("1P", Some(1 << 50)),
        ("", None),
        ("-1", None),
        ("unlimited", None),
        ("100Z", None),
        ("1.x", None),
        ("999999999999P", None),
        ("16383.99999P", None),
    ] {
        assert_eq!(size_of(written), bytes, "{written:?}");
    }
}

#[test]
fn an_allowance_of_zero_is_not_an_allowance() {
    assert_eq!(recorded_quota("0", 0), None);
    assert_eq!(
        recorded_quota("1G", -5),
        Some(Recorded {
            cap: 1 << 30,
            from: 0,
        })
    );
}
