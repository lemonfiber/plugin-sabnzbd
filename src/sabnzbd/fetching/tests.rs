use lemonfiber_ports::service::{Fetching, Pulling};

use super::super::tests::{asked, client, BASE, KEY};

const FETCHING: &str = r#"{"queue":{"paused":false}}"#;

const STOPPED: &str = r#"{"queue":{"paused":true}}"#;

const DID: &str = r#"{"status":true}"#;

const WOULD_NOT: &str = r#"{"status":false}"#;

#[tokio::test]
async fn a_paused_downloader_is_stopped_in_both_the_senses_that_matter() {
    assert_eq!(
        client(vec![(200, STOPPED)]).0.pulling().await.ok(),
        Some(Pulling::Stopped)
    );
    assert_eq!(
        client(vec![(200, FETCHING)]).0.pulling().await.ok(),
        Some(Pulling::Fetching)
    );
}

#[tokio::test]
async fn stopping_it_is_confirmed_by_asking_the_queue_rather_than_by_the_answer() {
    let (sabnzbd, http) = client(vec![(200, DID), (200, STOPPED)]);
    assert_eq!(sabnzbd.stop().await.ok(), Some(Pulling::Stopped));
    assert_eq!(
        asked(&http),
        [
            format!("{BASE}/api?mode=pause&output=json&apikey={KEY}"),
            format!("{BASE}/api?mode=queue&output=json&apikey={KEY}"),
        ]
    );
}

#[tokio::test]
async fn a_client_that_took_the_request_and_went_on_fetching_says_so() {
    let (sabnzbd, _) = client(vec![(200, DID), (200, FETCHING)]);
    assert_eq!(sabnzbd.stop().await.ok(), Some(Pulling::Fetching));
}

#[tokio::test]
async fn starting_it_again_is_confirmed_the_same_way() {
    let (sabnzbd, http) = client(vec![(200, DID), (200, FETCHING)]);
    assert_eq!(sabnzbd.resume().await.ok(), Some(Pulling::Fetching));
    assert!(http.asked_for("mode=resume"));
}

#[tokio::test]
async fn a_request_the_client_answers_that_it_would_not_carry_out_is_a_failure() {
    assert!(client(vec![(200, WOULD_NOT)]).0.stop().await.is_err());
    assert!(client(vec![(200, WOULD_NOT)]).0.resume().await.is_err());
}

#[tokio::test]
async fn a_client_that_will_not_answer_at_all_is_a_failure() {
    assert!(client(vec![(200, "not json")]).0.pulling().await.is_err());
    assert!(client(vec![(503, "")]).0.stop().await.is_err());
}
