//! DuckDuckGo Email Protection (unofficial API).

use crate::error::{Error, Result};
use serde::Deserialize;
use std::io::{self, BufRead, Write};

const BASE: &str = "https://quack.duckduckgo.com/api";

#[derive(Deserialize)]
struct Login {
    token: String,
}

#[derive(Deserialize)]
struct Dashboard {
    user: User,
}

#[derive(Deserialize)]
struct User {
    access_token: String,
}

#[derive(Deserialize)]
struct Address {
    address: String,
}

/// One-time: obtain a long-lived access token.
pub async fn get_access_token(client: &reqwest::Client, user: &str) -> Result<String> {
    client
        .get(format!("{BASE}/auth/loginlink"))
        .query(&[("user", user)])
        .send()
        .await?
        .error_for_status()?;

    {
        let mut stdout = io::stdout();
        write!(stdout, "Passphrase from email: ")?;
        stdout.flush()?;
    }

    let otp = io::stdin()
        .lock()
        .lines()
        .next()
        .ok_or(Error::NoPassphrase)??;

    let Login { token } = client
        .get(format!("{BASE}/auth/login"))
        .query(&[("user", user), ("otp", otp.trim())])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let d: Dashboard = client
        .get(format!("{BASE}/email/dashboard"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(d.user.access_token)
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
