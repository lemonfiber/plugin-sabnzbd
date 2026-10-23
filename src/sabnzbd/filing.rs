use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Filed, Filing};
use serde::Deserialize;

use super::{Sabnzbd, FULLSTATUS, GET_CONFIG, KEYWORD, SECTION, SKIP_DASHBOARD};

const UNCATEGORISED: &str = "*";

const SET_CONFIG: &str = "set_config";

const CATEGORIES: &str = "categories";

#[derive(Deserialize)]
struct Held {
    config: Categories,
}

#[derive(Deserialize)]
struct Categories {
    #[serde(default)]
    categories: Vec<Category>,
}

#[derive(Deserialize)]
struct Category {
    name: String,
    #[serde(default)]
    dir: String,
}

#[derive(Deserialize)]
struct Status {
    status: Completing,
}

#[derive(Deserialize)]
struct Completing {
    completedir: String,
}

#[derive(Deserialize)]
struct Answered {
    #[serde(default)]
    status: Option<bool>,
}

#[async_trait]
impl Filing for Sabnzbd {
    async fn filed(&self) -> Result<Vec<Filed>, Failure> {
        let held: Held = self
            .read(
                GET_CONFIG,
                &[(SECTION, CATEGORIES)],
                "the categories could not be read",
            )
            .await?;
        let status: Status = self
            .read(
                FULLSTATUS,
                &[SKIP_DASHBOARD],
                "the completed folder could not be read",
            )
            .await?;
        let below = status.status.completedir;
        Ok(held
            .config
            .categories
            .into_iter()
            .filter(|category| category.name != UNCATEGORISED)
            .map(|category| Filed {
                folder: folder_of(&below, &category),
                category: category.name,
            })
            .collect())
    }

    async fn file_under(&self, folders: &[Filed]) -> Result<(), Failure> {
        if let Some(misshapen) = folders.iter().find(|one| !held_to_shape(one)) {
            return Err(self.endpoint.refused(&format!(
                "{:?} under {:?} is not a category and folder SABnzbd could hold",
                misshapen.category, misshapen.folder
            )));
        }
        for one in folders {
            let answered: Answered = self
                .read(
                    SET_CONFIG,
                    &[
                        (SECTION, CATEGORIES),
                        (KEYWORD, &one.category),
                        ("dir", &one.folder),
                    ],
                    "a category's completed folder could not be set",
                )
                .await?;
            if answered.status == Some(false) {
                return Err(self.endpoint.refused(&format!(
                    "the client would not file {} under {}",
                    one.category, one.folder
                )));
            }
        }
        Ok(())
    }
}

fn held_to_shape(one: &Filed) -> bool {
    let category = one.category.as_str();
    let named = !category.is_empty()
        && category != UNCATEGORISED
        && category
            .chars()
            .all(|letter| letter.is_alphanumeric() || matches!(letter, '-' | '_' | '.' | ' '));
    let placed = one.folder.starts_with('/')
        && !one.folder.chars().any(char::is_control)
        && !one.folder.split('/').any(|segment| segment == "..");
    named && placed
}

fn folder_of(below: &str, category: &Category) -> String {
    let dir = category.dir.trim();
    let named = if dir.is_empty() {
        category.name.as_str()
    } else {
        dir
    };
    if named.starts_with('/') {
        named.to_owned()
    } else {
        format!("{}/{named}", below.trim_end_matches('/'))
    }
}

#[cfg(test)]
mod tests;
