use serde::Deserialize;
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

const BASE: &str = "https://quack.duckduckgo.com/api";

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("usage: ote <duck-username> (or set DDG_TOKEN)")]
    MissingUsername,

    #[error("no passphrase entered")]
    NoPassphrase,
}

type Result<T> = std::result::Result<T, Error>;

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
async fn get_access_token(client: &reqwest::Client, user: &str) -> Result<String> {
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

async fn generate_alias(client: &reqwest::Client, access_token: &str) -> Result<String> {
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

async fn run() -> Result<()> {
    let client = reqwest::Client::builder().build()?;

    let token = match std::env::var("DDG_TOKEN") {
        Ok(t) => t,
        Err(_) => {
            let user = std::env::args().nth(1).ok_or(Error::MissingUsername)?;
            let t = get_access_token(&client, &user).await?;
            writeln!(io::stderr(), "Save this: export DDG_TOKEN={t}")?;
            t
        }
    };

    let alias = generate_alias(&client, &token).await?;

    writeln!(io::stdout(), "{alias}")?;

    Ok(())
}

fn main() -> ExitCode {
    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(Error::from)
        .and_then(|rt| rt.block_on(run()));

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(io::stderr(), "error: {e}");
            ExitCode::FAILURE
        }
    }
}
