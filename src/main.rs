/// Progress message on stderr. Unlike `eprintln!`, ignores write errors
/// instead of panicking. Defined before the `mod`s so they can use it too.
macro_rules! status {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

mod bitwarden;
mod duck;
mod error;

use error::{Error, Result};
use std::collections::HashSet;
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::SystemTime;

/// `ote [<count>]`, where `count` is a positive integer (default 1): the
/// number of aliases to request, and so the most that can be registered.
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
    let mut vault = bitwarden::Vault::unlock()?;
    let client = reqwest::Client::builder().timeout(duck::TIMEOUT).build()?;

    // DDG returns addresses you already have instead of an error once the
    // limit is reached, so stop at the first one repeated in this run or
    // already in the vault. Neither failure below is fatal: generating can
    // go ahead without the vault check.
    status!("Checking Bitwarden for existing aliases...");

    if let Err(e) = vault.sync() {
        status!("Warning: sync failed, using the local copy of the vault: {e}");
    }

    let existing = match vault.existing_aliases() {
        Ok(aliases) => {
            status!("Found {} @duck.com address(es) in Bitwarden", aliases.len());
            aliases
        }
        Err(e) => {
            status!("Warning: couldn't read existing aliases, skipping that check: {e}");
            HashSet::new()
        }
    };
    let mut seen = HashSet::new();

    // On failure, stop generating but still save what was generated so far:
    // those aliases already exist on DuckDuckGo's side.
    let mut entries = Vec::new();
    let mut error = None;

    for i in 0..count {
        status!("[{}/{count}] Requesting address...", i + 1);

        if i > 0 {
            tokio::time::sleep(duck::COOLDOWN).await;
        }

        let alias = match duck::generate_alias(&client, &token).await {
            Ok(alias) => alias,
            Err(e) => {
                error.get_or_insert(e);
                break;
            }
        };

        if !seen.insert(alias.clone()) {
            status!("Stopped: {alias} was repeated in this run (limit reached?)");
            break;
        }
        if existing.contains(&alias.to_lowercase()) {
            status!("Stopped: {alias} is already in Bitwarden (limit reached?)");
            break;
        }

        // Print as we go, so aliases survive Ctrl-C or a Bitwarden failure.
        if let Err(e) = writeln!(io::stdout(), "{alias}") {
            error.get_or_insert(e.into());
        }

        // The alias already exists on DDG, so save it under a plain name
        // rather than lose it if the timestamp fails.
        let name = match local_timestamp() {
            Ok(t) => format!("OTE {t}"),
            Err(e) => {
                error.get_or_insert(e);
                "OTE".to_owned()
            }
        };
        entries.push(bitwarden::Entry { name, alias });
    }

    let saved = if entries.is_empty() {
        Ok(())
    } else {
        status!("Saving {} item(s) to Bitwarden...", entries.len());
        vault.add(&entries)
    };

    match saved {
        Ok(()) => {
            status!(
                "Registered {} of {count} requested address(es)",
                entries.len()
            );
        }
        Err(e) => {
            error.get_or_insert(e);
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
            status!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
