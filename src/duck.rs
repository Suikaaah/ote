//! DuckDuckGo Email Protection (unofficial API).

use crate::error::Result;
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;

const BASE: &str = "https://quack.duckduckgo.com/api";

/// Pause between consecutive aliases. At least 1s, so timestamps in item names differ.
pub const COOLDOWN: Duration = Duration::from_secs(2);

/// Per-request timeout, so a stalled connection fails instead of hanging.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// Retries on HTTP 429, waiting `BACKOFF`, then twice as long each time.
const MAX_RETRIES: u32 = 4;
const BACKOFF: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
struct Address {
    address: String,
}

async fn request(client: &reqwest::Client, access_token: &str) -> Result<reqwest::Response> {
    Ok(client
        .post(format!("{BASE}/email/addresses"))
        .bearer_auth(access_token)
        .send()
        .await?)
}

async fn parse(response: reqwest::Response) -> Result<String> {
    let a: Address = response.error_for_status()?.json().await?;

    Ok(format!("{}@duck.com", a.address))
}

pub async fn generate_alias(client: &reqwest::Client, access_token: &str) -> Result<String> {
    let mut backoff = BACKOFF;

    for _ in 0..MAX_RETRIES {
        let response = request(client, access_token).await?;

        if response.status() != StatusCode::TOO_MANY_REQUESTS {
            return parse(response).await;
        }

        status!("Rate limited; retrying in {}s", backoff.as_secs());
        tokio::time::sleep(backoff).await;
        backoff = backoff.saturating_mul(2);
    }

    parse(request(client, access_token).await?).await
}
