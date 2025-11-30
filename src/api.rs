use anyhow::Result;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::parser::Leaderboard;

// We're only allowed to fetch the JSON once every 15 min. See:
// https://www.reddit.com/r/adventofcode/comments/1pa472d/reminder_please_throttle_your_aoc_traffic/
const MIN_FETCH_INTERVAL: Duration = Duration::from_secs(15 * 60);

pub struct Client {
    session: String,
    cache_dir: PathBuf,
    contact_info: String,
}

impl Client {
    pub fn new<S: Into<String>, P: Into<PathBuf>, C: Into<String>>(
        session: S,
        cache_dir: P,
        contact_info: C,
    ) -> Self {
        Self {
            session: session.into(),
            cache_dir: cache_dir.into(),
            contact_info: contact_info.into(),
        }
    }

    pub async fn fetch(&self, year: i32, id: usize) -> Result<Leaderboard> {
        let cache_path = self
            .cache_dir
            .join(format!("aoc-leaderboard-{}-{}.json", year, id));

        // Check if we have a cached version before trying to fetch
        let use_cached_json = if let Ok(m) = cache_path.as_path().metadata() {
            let last_modified = SystemTime::now()
                .duration_since(m.modified()?)
                .unwrap_or(Duration::ZERO);
            last_modified < MIN_FETCH_INTERVAL
        } else {
            false
        };

        let json_str = if use_cached_json {
            tracing::info!("Using cached leaderboard {} ({})", id, year);
            std::fs::read_to_string(cache_path)?
        } else {
            // TODO: Detect if session is wrong since it redirects
            tracing::info!("Refreshing cached leaderboard {} ({})", id, year);
            const PACKAGE_NAME_AND_VERSION: &str =
                concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
            let user_agent = format!(
                "{PACKAGE_NAME_AND_VERSION} (contact: {})",
                &self.contact_info,
            );
            let client = reqwest::Client::builder().user_agent(&user_agent).build()?;
            let response = client
                .get(format!(
                    "https://adventofcode.com/{}/leaderboard/private/view/{}.json",
                    year, id
                ))
                .header("Cookie", &format!("session={}", &self.session))
                .send()
                .await?
                .text()
                .await?;

            // Save updated content in the cache
            let mut f = File::create(cache_path)?;
            f.write_all(response.as_ref())?;

            response
        };

        Ok(serde_json::from_str(&json_str)?)
    }
}
