use lemonfiber_ports::service::{Failure, Filed, Filing};

use super::super::tests::{asked, client, BASE, KEY};

const CATEGORIES: &str = r#"{"config":{"categories":[
    {"name":"*","dir":""},
    {"name":"tv","dir":"/data/downloads/complete/tv"},
    {"name":"movies","dir":"films"},
    {"name":"music","dir":""}]}}"#;

const COMPLETED: &str = r#"{"status":{"completedir":"/data/downloads/usenet/complete/"}}"#;

fn filed(category: &str, folder: &str) -> Filed {
    Filed {
        category: category.to_owned(),
        folder: folder.to_owned(),
    }
}

#[tokio::test]
async fn each_category_is_read_as_the_folder_its_downloads_complete_into() {
    let (sabnzbd, http) = client(vec![(200, CATEGORIES), (200, COMPLETED)]);
    assert_eq!(
        sabnzbd.filed().await.ok(),
        Some(vec![
            filed("tv", "/data/downloads/complete/tv"),
            filed("movies", "/data/downloads/usenet/complete/films"),
            filed("music", "/data/downloads/usenet/complete/music"),
        ])
    );
    assert_eq!(
        asked(&http),
        [
            format!("{BASE}/api?mode=get_config&section=categories&output=json&apikey={KEY}"),
            format!("{BASE}/api?mode=fullstatus&skip_dashboard=1&output=json&apikey={KEY}"),
        ]
    );
}

#[tokio::test]
async fn a_status_naming_no_completed_folder_is_a_failure_rather_than_a_guess() {
    let (sabnzbd, _) = client(vec![
        (200, r#"{"config":{"categories":[{"name":"tv"}]}}"#),
        (200, r#"{"status":{}}"#),
    ]);
    assert!(matches!(
        sabnzbd.filed().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn categories_that_will_not_read_are_said_rather_than_read_as_none() {
    assert!(client(vec![(500, "")]).0.filed().await.is_err());
    assert!(client(vec![(200, CATEGORIES), (500, "")])
        .0
        .filed()
        .await
        .is_err());
}

#[tokio::test]
async fn each_category_is_written_with_its_folder_and_nothing_else() {
    let (sabnzbd, http) = client(vec![(200, "{}"), (200, r#"{"status":true}"#)]);
    let written = sabnzbd
        .file_under(&[
            filed("tv", "/data/downloads/complete/tv"),
            filed("music", "/data/downloads/complete/music"),
        ])
        .await;
    assert_eq!(written.ok(), Some(()));
    assert_eq!(
        asked(&http),
        [
            format!("{BASE}/api?mode=set_config&section=categories&keyword=tv&dir=%2Fdata%2Fdownloads%2Fcomplete%2Ftv&output=json&apikey={KEY}"),
            format!("{BASE}/api?mode=set_config&section=categories&keyword=music&dir=%2Fdata%2Fdownloads%2Fcomplete%2Fmusic&output=json&apikey={KEY}"),
        ]
    );
}

#[tokio::test]
async fn a_category_the_client_refuses_stops_the_filing_and_says_which() {
    let (sabnzbd, http) = client(vec![(200, r#"{"status":false,"error":"no"}"#)]);
    let written = sabnzbd
        .file_under(&[
            filed("tv", "/data/downloads/complete/tv"),
            filed("music", "/data/downloads/complete/music"),
        ])
        .await;
    assert!(matches!(
        written,
        Err(Failure::Refused { detail, .. }) if detail.contains("tv")
    ));
    assert_eq!(http.requests().len(), 1);
}

#[tokio::test]
async fn a_category_or_folder_sabnzbd_could_not_hold_is_refused_before_anything_is_asked() {
    for (category, folder) in [
        ("", "/data/downloads/complete/tv"),
        ("*", "/data/downloads/complete/tv"),
        ("tv&mode=shutdown", "/data/downloads/complete/tv"),
        ("tv", "data/downloads/complete/tv"),
        ("tv", "/data/downloads/../../config"),
        ("tv", "/data/downloads/complete/tv\n"),
    ] {
        let (sabnzbd, http) = client(vec![(200, r#"{"status":true}"#)]);
        let refused = sabnzbd
            .file_under(&[
                filed("movies", "/data/downloads/complete/movies"),
                filed(category, folder),
            ])
            .await;
        assert!(
            matches!(&refused, Err(Failure::Refused { detail, .. }) if detail.contains("could hold")),
            "{category:?} {folder:?}: {refused:?}"
        );
        assert!(http.requests().is_empty(), "{category:?} {folder:?}");
    }
}

#[tokio::test]
async fn a_category_named_in_words_and_marks_is_one_it_holds() {
    let (sabnzbd, _) = client(vec![(200, r#"{"status":true}"#)]);
    let written = sabnzbd
        .file_under(&[filed("tv-4k_new.one Two", "/data/downloads/complete/x")])
        .await;
    assert_eq!(written.ok(), Some(()));
}
