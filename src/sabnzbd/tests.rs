use std::sync::Arc;

use lemonfiber_adapter_kit::credential::Holding;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::{Failure, Fetching};

use super::Sabnzbd;

pub(super) const BASE: &str = "http://127.0.0.1:8080";

pub(super) const KEY: &str = "the-key";

pub(super) fn client(replies: Vec<(u16, &'static str)>) -> (Sabnzbd, Arc<Fake>) {
    let http = Fake::scripted(replies);
    (
        Sabnzbd::new(http.clone(), BASE, Arc::new(KEY.to_owned())),
        http,
    )
}

pub(super) fn asked(http: &Fake) -> Vec<String> {
    http.requests().into_iter().map(|one| one.url).collect()
}

struct Nothing;

impl Holding for Nothing {
    fn key(&self) -> Result<String, String> {
        Err("upstream.json could not be read: not found".to_owned())
    }
}

#[tokio::test]
async fn it_answers_while_its_version_is_answered_without_a_key() {
    let up = Fake::always(Answer::reply(200, r#"{"version":"4.5.5"}"#));
    assert!(
        Sabnzbd::new(up.clone(), BASE, Arc::new(Nothing))
            .answers()
            .await
    );
    assert_eq!(asked(&up), [format!("{BASE}/api?mode=version&output=json")]);
    for down in [Fake::silent(), Fake::always(Answer::reply(503, ""))] {
        assert!(!Sabnzbd::new(down, BASE, Arc::new(Nothing)).answers().await);
    }
}

#[tokio::test]
async fn without_a_key_nothing_is_asked_and_the_refusal_says_why() {
    let http = Fake::silent();
    let refused = Sabnzbd::new(http.clone(), BASE, Arc::new(Nothing))
        .pulling()
        .await;
    assert!(
        matches!(&refused, Err(Failure::Refused { detail, .. })
            if detail == "this adapter holds no key for SABnzbd: upstream.json could not be read: not found"),
        "{refused:?}"
    );
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn the_key_is_presented_as_a_query_field_written_as_a_form_writes_it() {
    let http = Fake::scripted(vec![(200, r#"{"queue":{"paused":false}}"#)]);
    let pulled = Sabnzbd::new(http.clone(), BASE, Arc::new("a+b/c=".to_owned()))
        .pulling()
        .await;
    assert!(pulled.is_ok());
    assert_eq!(
        asked(&http),
        [format!(
            "{BASE}/api?mode=queue&output=json&apikey=a%2Bb%2Fc%3D"
        )]
    );
}

#[tokio::test]
async fn a_key_sabnzbd_refuses_is_unauthorised_and_blamed_on_it() {
    let (sabnzbd, _) = client(vec![(403, "API Key Incorrect")]);
    assert!(matches!(
        sabnzbd.pulling().await,
        Err(Failure::Unauthorised { service }) if service == "sabnzbd"
    ));
}
