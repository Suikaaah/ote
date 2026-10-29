//! DuckDuckGo Email Protection (unofficial API).

use crate::error::Result;
use serde::Deserialize;

const BASE: &str = "https://quack.duckduckgo.com/api";

#[derive(Deserialize)]
struct Address {
    address: String,
}

pub async fn generate_alias(client: &reqwest::Client, access_token: &str) -> Result<String> {
    let a: Address = client
        .post(format!("{BASE}/email/addresses"))
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(format!("{}@duck.com", a.address))
}
