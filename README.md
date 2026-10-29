# OTE

Generate [DuckDuckGo Email Protection](https://duckduckgo.com/email/) private addresses and save them to Bitwarden.

Each generated `xxxx@duck.com` alias is printed and saved as a login item `OTE YYYY-MM-DD HH:MM:SS` (with the alias as username) in the Bitwarden folder `OTE`, which is created if needed.

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
ote 5      # up to five aliases, requested ~2s apart
```

```
Checking Bitwarden for existing aliases...
Found 4 @duck.com address(es) in Bitwarden
[1/5] Requesting address...
abc123@duck.com
[2/5] Requesting address...
def456@duck.com
[3/5] Requesting address...
Stopped: xyz789@duck.com is already in Bitwarden (limit reached?)
Saving 2 item(s) to Bitwarden...
Registered 2 of 5 requested address(es)
```

- **Bitwarden unlock:** `ote` uses `BW_SESSION` if it is set (e.g. by `export BW_SESSION=$(bw unlock --raw)`). Otherwise it runs `bw unlock` itself, which prompts for your master password.
- **Output:** new aliases go to stdout and progress messages to stderr, so `ote | wl-copy` (`ote | pbcopy` on macOS) copies just the addresses.
- **Duplicates:** once you hit DuckDuckGo's limit, it returns addresses you already have instead of an error. `ote` stops at the first address repeated within the run or already used as a login username anywhere in your vault (any folder, case-insensitive), so `ote n` registers *at most* `n`. If Bitwarden can't be checked, `ote` warns and continues without this check.
- **Saving:** all aliases are saved in a single `bw import`. If generation fails partway, the ones already generated are still saved.

## Notes

- Uses DuckDuckGo's unofficial API, which may change without notice.
- Keep `DDG_TOKEN` secret: it allows generating aliases on your account.
