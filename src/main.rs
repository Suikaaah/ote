use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use std::io::{self, BufRead, Write};
use std::process::{Command, ExitCode};
use std::time::SystemTime;

const BASE: &str = "https://quack.duckduckgo.com/api";

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("time error: {0}")]
    Time(#[from] jiff::Error),

    #[error("usage: ote <duck-username> (or set DDG_TOKEN)")]
    Usage,

    #[error("no passphrase entered")]
    NoPassphrase,

    #[error("bitwarden: {0}")]
    Bitwarden(String),
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

/// `ote [<duck-username>]`
fn parse_username() -> Result<Option<String>> {
    // `args_os` instead of `args`: the latter panics on non-UTF-8 arguments.
    let mut args = std::env::args_os().skip(1);

    let username = args
        .next()
        .map(|a| a.into_string().map_err(|_| Error::Usage))
        .transpose()?;

    if args.next().is_some() {
        Err(Error::Usage)
    } else {
        Ok(username)
    }
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

/// Run the Bitwarden CLI and return its stdout.
fn bw(args: &[&str]) -> Result<Vec<u8>> {
    let out = Command::new("bw")
        .arg("--nointeraction")
        .args(args)
        .output()
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => Error::Bitwarden("`bw` not found in PATH".into()),
            _ => e.into(),
        })?;

    if out.status.success() {
        Ok(out.stdout)
    } else {
        let msg = if out.stderr.is_empty() {
            &out.stdout
        } else {
            &out.stderr
        };

        Err(Error::Bitwarden(
            String::from_utf8_lossy(msg).trim().to_owned(),
        ))
    }
}

/// Create a Bitwarden login item named `name` with `alias` as its username.
/// Requires an unlocked vault (`BW_SESSION` set).
fn add_to_bitwarden(name: &str, alias: &str) -> Result<()> {
    let mut template: serde_json::Value =
        serde_json::from_slice(&bw(&["get", "template", "item"])?)?;

    // `Value`'s `IndexMut` panics on non-objects, so go through `as_object_mut`.
    let item = template
        .as_object_mut()
        .ok_or_else(|| Error::Bitwarden("item template is not a JSON object".into()))?;

    item.insert("name".into(), name.into());
    item.insert("notes".into(), serde_json::Value::Null);
    item.insert(
        "login".into(),
        serde_json::json!({
            "uris": [],
            "username": alias,
            "password": null,
            "totp": null,
        }),
    );

    let encoded = STANDARD.encode(serde_json::to_vec(&template)?);
    bw(&["create", "item", &encoded])?;

    Ok(())
}

/// Current local time as `YYYY-MM-DD HH:MM:SS`.
fn local_timestamp() -> Result<String> {
    // Fallible counterparts of `Zoned::now()` and `.strftime().to_string()`,
    // which panic on out-of-range clocks and formatting errors respectively.
    let now = jiff::Timestamp::try_from(SystemTime::now())?;
    let zoned = now.to_zoned(jiff::tz::TimeZone::try_system()?);

    Ok(jiff::fmt::strtime::format("%Y-%m-%d %H:%M:%S", &zoned)?)
}

async fn run() -> Result<()> {
    let username = parse_username()?;
    let client = reqwest::Client::builder().build()?;

    let token = match std::env::var("DDG_TOKEN") {
        Ok(t) => t,
        Err(_) => {
            let user = username.as_deref().ok_or(Error::Usage)?;
            let t = get_access_token(&client, user).await?;
            writeln!(io::stderr(), "Save this: export DDG_TOKEN={t}")?;
            t
        }
    };

    let alias = generate_alias(&client, &token).await?;

    // Print before saving, so the alias isn't lost if Bitwarden fails.
    writeln!(io::stdout(), "{alias}")?;

    let name = format!("OTE-{}", local_timestamp()?);
    add_to_bitwarden(&name, &alias)?;
    writeln!(io::stderr(), "Added to Bitwarden as \"{name}\"")?;

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
