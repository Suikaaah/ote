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
ote        # one alias
ote 5      # five aliases, ~3s apart
```

```
? Master password: [hidden]
abc123@duck.com
def456@duck.com
...
Added 5 item(s) to Bitwarden
```

`ote` asks for your Bitwarden master password on each run. To unlock once per shell instead, run:

```sh
export BW_SESSION=$(bw unlock --raw)
```

Aliases go to stdout and messages go to stderr, so `ote | wl-copy` (`ote | pbcopy` on macOS) copies just the address. All aliases are saved in a single `bw import`; if generation fails partway, the ones already generated are still saved.

## Notes

- Uses DuckDuckGo's unofficial API, which may change without notice.
- Keep `DDG_TOKEN` secret: it allows generating aliases on your account.
