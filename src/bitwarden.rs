//! Bitwarden vault access via the `bw` CLI.

use crate::error::{Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::process::{Command, Stdio};

const FOLDER: &str = "OTE";

pub struct Entry {
    pub name: String,
    pub alias: String,
}

#[derive(Deserialize)]
struct Folder {
    id: Option<String>,
    name: String,
}

#[derive(Deserialize)]
struct Item {
    login: Option<ItemLogin>,
}

#[derive(Deserialize)]
struct ItemLogin {
    username: Option<String>,
}

fn spawn_error(e: io::Error) -> Error {
    match e.kind() {
        io::ErrorKind::NotFound => Error::Bitwarden("`bw` not found in PATH".into()),
        _ => e.into(),
    }
}

/// An unlocked vault. The session key is passed to each `bw` call directly,
/// since setting `BW_SESSION` on our own process (`env::set_var`) is unsafe
/// in a multi-threaded program.
pub struct Vault {
    session: String,
    /// `None` until looked up; then the ID of the folder `FOLDER`, if it exists.
    folder: Option<Option<String>>,
}

impl Vault {
    /// Use `BW_SESSION` if set; otherwise run `bw unlock`, which prompts for
    /// the master password on the terminal.
    pub fn unlock() -> Result<Self> {
        if let Some(session) = std::env::var("BW_SESSION").ok().filter(|s| !s.is_empty()) {
            return Ok(Self {
                session,
                folder: None,
            });
        }

        status!("Unlocking Bitwarden (BW_SESSION not set)...");

        // stdin and stderr stay on the terminal for the prompt; stdout is the key.
        let out = Command::new("bw")
            .args(["unlock", "--raw"])
            .stdin(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(spawn_error)?;

        let session = String::from_utf8_lossy(&out.stdout).trim().to_owned();

        if out.status.success() && !session.is_empty() {
            Ok(Self {
                session,
                folder: None,
            })
        } else {
            Err(Error::Bitwarden("unlock failed".into()))
        }
    }

    /// Run the Bitwarden CLI and return its stdout.
    fn bw(&self, args: &[&str]) -> Result<Vec<u8>> {
        let out = Command::new("bw")
            .arg("--nointeraction")
            .args(args)
            .env("BW_SESSION", &self.session)
            .output()
            .map_err(spawn_error)?;

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

    /// ID of the folder `FOLDER`, if it exists.
    fn find_folder(&mut self) -> Result<Option<String>> {
        if let Some(cached) = &self.folder {
            return Ok(cached.clone());
        }

        let folders: Vec<Folder> =
            serde_json::from_slice(&self.bw(&["list", "folders", "--search", FOLDER])?)?;

        // `--search` matches substrings, so look for an exact match.
        let id = folders
            .into_iter()
            .find(|f| f.name == FOLDER)
            .and_then(|f| f.id);

        self.folder = Some(id.clone());

        Ok(id)
    }

    /// ID of the folder `FOLDER`, created if absent.
    fn ensure_folder(&mut self) -> Result<String> {
        if let Some(id) = self.find_folder()? {
            return Ok(id);
        }

        let encoded = STANDARD.encode(serde_json::to_vec(&serde_json::json!({ "name": FOLDER }))?);
        let created: Folder = serde_json::from_slice(&self.bw(&["create", "folder", &encoded])?)?;

        let id = created
            .id
            .ok_or_else(|| Error::Bitwarden("created folder has no id".into()))?;
        self.folder = Some(Some(id.clone()));

        Ok(id)
    }

    /// Refresh the local copy of the vault, which `bw list` reads.
    pub fn sync(&self) -> Result<()> {
        self.bw(&["sync"]).map(drop)
    }

    /// `@duck.com` usernames of all login items in the vault, in any folder,
    /// trimmed and lowercased. Read from the local copy of the vault (see `sync`).
    pub fn existing_aliases(&self) -> Result<HashSet<String>> {
        let items: Vec<Item> = serde_json::from_slice(&self.bw(&["list", "items"])?)?;

        Ok(items
            .into_iter()
            .filter_map(|i| i.login?.username)
            .map(|u| u.trim().to_lowercase())
            .filter(|u| u.ends_with("@duck.com"))
            .collect())
    }

    /// Write `json` to a private temporary file and `bw import` it.
    fn import(&self, json: &[u8]) -> Result<()> {
        let path = std::env::temp_dir().join(format!("ote-{}.json", std::process::id()));

        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        options.open(&path)?.write_all(json)?;

        let result = match path.to_str() {
            Some(p) => self.bw(&["import", "bitwardenjson", p]).map(drop),
            None => Err(Error::Bitwarden("temporary path is not UTF-8".into())),
        };
        let _ = fs::remove_file(&path);

        result
    }

    /// Create one login item per entry (alias as username) in the folder
    /// `FOLDER`, as a single import.
    pub fn add(&mut self, entries: &[Entry]) -> Result<()> {
        let folder_id = self.ensure_folder()?;
        let template: serde_json::Value =
            serde_json::from_slice(&self.bw(&["get", "template", "item"])?)?;

        // `Value`'s `IndexMut` panics on non-objects, so go through `as_object`.
        let template = template
            .as_object()
            .ok_or_else(|| Error::Bitwarden("item template is not a JSON object".into()))?;

        let items: Vec<_> = entries
            .iter()
            .map(|e| {
                let mut item = template.clone();

                item.insert("name".into(), e.name.as_str().into());
                item.insert("folderId".into(), folder_id.as_str().into());
                item.insert("notes".into(), serde_json::Value::Null);
                item.insert(
                    "login".into(),
                    serde_json::json!({
                        "uris": [],
                        "username": e.alias,
                        "password": null,
                        "totp": null,
                    }),
                );

                serde_json::Value::Object(item)
            })
            .collect();

        // Bitwarden's unencrypted JSON export format. Folders are matched by ID,
        // so passing the existing one avoids creating a duplicate.
        let export = serde_json::json!({
            "encrypted": false,
            "folders": [{ "id": folder_id, "name": FOLDER }],
            "items": items,
        });

        self.import(&serde_json::to_vec(&export)?)
    }
}
