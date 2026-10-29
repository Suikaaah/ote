# OTE

Generate a [DuckDuckGo Email Protection](https://duckduckgo.com/email/) private address and save it to Bitwarden.

Each run creates a new `xxxx@duck.com` alias, prints it, and adds a login item `OTE YYYY-MM-DD HH:MM:SS` (with the alias as username) to the Bitwarden folder `OTE`, creating the folder if needed.

## Requirements

- A DuckDuckGo Email Protection account
- [Bitwarden CLI](https://bitwarden.com/help/cli/) (`bw`) in `PATH`, logged in

## Install

```sh
cargo install --path .
```

## Setup (once)

Get your DuckDuckGo access token:

1. While signed in to Email Protection, open <https://duckduckgo.com/email/settings/autofill>.
2. Open devtools → Network, then click **Generate Private Duck Address**.
3. In the `addresses` request, copy the `Authorization` header value after `Bearer`.

```sh
export DDG_TOKEN=...
```

## Usage

```sh
export BW_SESSION=$(bw unlock --raw)
ote
```

```
abc123@duck.com
Added to Bitwarden as "OTE 2026-10-29 10:19:31"
```

The alias goes to stdout and messages go to stderr, so `ote | wl-copy` copies just the address.

## Notes

- Uses DuckDuckGo's unofficial API, which may change without notice.
- Keep `DDG_TOKEN` secret: it allows generating aliases on your account.
