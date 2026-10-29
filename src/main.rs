mod bitwarden;
mod duck;
mod error;

use error::{Error, Result};
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::SystemTime;

/// Current local time as `YYYY-MM-DD HH:MM:SS`.
fn local_timestamp() -> Result<String> {
    // Fallible counterparts of `Zoned::now()` and `.strftime().to_string()`,
    // which panic on out-of-range clocks and formatting errors respectively.
    let now = jiff::Timestamp::try_from(SystemTime::now())?;
    let zoned = now.to_zoned(jiff::tz::TimeZone::try_system()?);

    Ok(jiff::fmt::strtime::format("%Y-%m-%d %H:%M:%S", &zoned)?)
}

async fn run() -> Result<()> {
    if std::env::args_os().nth(1).is_some() {
        return Err(Error::Usage);
    }

    let token = std::env::var("DDG_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .ok_or(Error::MissingToken)?;

    let client = reqwest::Client::builder().build()?;

    let alias = duck::generate_alias(&client, &token).await?;

    // Print before saving, so the alias isn't lost if Bitwarden fails.
    writeln!(io::stdout(), "{alias}")?;

    let name = format!("OTE {}", local_timestamp()?);
    bitwarden::add_to_bitwarden(&name, &alias)?;
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
