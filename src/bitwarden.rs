//! Bitwarden vault access via the `bw` CLI.

use crate::error::{Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use std::io;
use std::process::Command;

const FOLDER: &str = "OTE";

#[derive(Deserialize)]
struct Folder {
    id: Option<String>,
    name: String,
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

/// ID of the Bitwarden folder named `name`, created if absent.
fn ensure_folder(name: &str) -> Result<String> {
    let folders: Vec<Folder> =
        serde_json::from_slice(&bw(&["list", "folders", "--search", name])?)?;

    // `--search` matches substrings, so look for an exact match.
    match folders
        .into_iter()
        .find(|f| f.name == name)
        .and_then(|f| f.id)
    {
        Some(id) => Ok(id),
        None => {
            let encoded =
                STANDARD.encode(serde_json::to_vec(&serde_json::json!({ "name": name }))?);
            let created: Folder = serde_json::from_slice(&bw(&["create", "folder", &encoded])?)?;

            created
                .id
                .ok_or_else(|| Error::Bitwarden("created folder has no id".into()))
        }
    }
}

/// Create a Bitwarden login item named `name` with `alias` as its username,
/// in the folder `FOLDER`. Requires an unlocked vault (`BW_SESSION` set).
pub fn add_to_bitwarden(name: &str, alias: &str) -> Result<()> {
    let folder_id = ensure_folder(FOLDER)?;
    let mut template: serde_json::Value =
        serde_json::from_slice(&bw(&["get", "template", "item"])?)?;

    // `Value`'s `IndexMut` panics on non-objects, so go through `as_object_mut`.
    let item = template
        .as_object_mut()
        .ok_or_else(|| Error::Bitwarden("item template is not a JSON object".into()))?;

    item.insert("name".into(), name.into());
    item.insert("folderId".into(), folder_id.into());
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
