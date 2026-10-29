mod bitwarden;
mod duck;
mod error;

use error::{Error, Result};
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::SystemTime;

/// `ote [<count>]`, where `count` is a positive integer (default 1).
fn parse_count() -> Result<usize> {
    // `args_os` instead of `args`: the latter panics on non-UTF-8 arguments.
    let mut args = std::env::args_os().skip(1);

    let count = match args.next() {
        None => 1,
        Some(a) => a
            .to_str()
            .and_then(|s| s.parse().ok())
            .filter(|&n| n > 0)
            .ok_or(Error::Usage)?,
    };

    if args.next().is_some() {
        Err(Error::Usage)
    } else {
        Ok(count)
    }
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
    let count = parse_count()?;

    let token = std::env::var("DDG_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .ok_or(Error::MissingToken)?;

    // Unlock before generating, so a wrong password doesn't strand new aliases.
    let vault = bitwarden::Vault::unlock()?;
    let client = reqwest::Client::builder().build()?;

    // On failure, stop generating but still save what was generated so far:
    // those aliases already exist on DuckDuckGo's side.
    let mut entries = Vec::new();
    let mut error = None;

    for i in 0..count {
        if i > 0 {
            tokio::time::sleep(duck::COOLDOWN).await;
        }

        let entry = match duck::generate_alias(&client, &token).await {
            Ok(alias) => local_timestamp().map(|t| bitwarden::Entry {
                name: format!("OTE {t}"),
                alias,
            }),
            Err(e) => Err(e),
        };

        match entry {
            Ok(entry) => {
                // Print as we go, so aliases survive Ctrl-C or a Bitwarden failure.
                if let Err(e) = writeln!(io::stdout(), "{}", entry.alias) {
                    error.get_or_insert(e.into());
                }
                entries.push(entry);
            }
            Err(e) => {
                error.get_or_insert(e);
                break;
            }
        }
    }

    if !entries.is_empty() {
        match vault.add(&entries) {
            Ok(()) => {
                let _ = writeln!(io::stderr(), "Added {} item(s) to Bitwarden", entries.len());
            }
            Err(e) => {
                error.get_or_insert(e);
            }
        }
    }

    error.map_or(Ok(()), Err)
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
