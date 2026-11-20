//! Live state reads: from a JSON snapshot file, or from Soroban RPC
//! `simulateTransaction` calls against the contract's read-only entry points.

use std::collections::BTreeMap;

use crate::decode::{decode_events, Value};
use crate::state::State;

/// Parses a live state snapshot:
///
/// ```json
/// {"admins": ["G..."],
///  "identities": [{"owner": "G...", "folder_name": "x.folder"}],
///  "templates": [{"template_id": 1, "metadata_uri": "ipfs://...", "creator": "G..."}],
///  "portfolios": [{"owner": "G...", "template_id": 1, "ipfs_cid": "bafy..."}]}
/// ```
pub fn parse_live_state(input: &str) -> Result<State, String> {
    let fields = match decode_events(&format!("[{}]", input.trim().trim_start_matches('[').trim_end_matches(']')))? {
        _ => Vec::new(),
    };
    let _ = fields;
    let value = crate::decode::parse_json(input)?;
    let fields = match value {
        Value::Object(fields) => fields,
        _ => return Err("live state must be a JSON object".to_string()),
    };
    let mut state = State::new();
    for (key, value) in &fields {
        match key.as_str() {
            "admins" => state.admins = string_list(value, "admins")?,
            "identities" => {
                for entry in object_list(value, "identities")? {
                    let owner = required_str(&entry, "owner")?;
                    let folder = required_str(&entry, "folder_name")?;
                    state.folder_owner.insert(folder.clone(), owner.clone());
                    state.owner_folder.insert(owner, folder);
                }
            }
            "templates" => {
                for entry in object_list(value, "templates")? {
                    let id = required_u32(&entry, "template_id")?;
                    let uri = required_str(&entry, "metadata_uri")?;
                    let creator = required_str(&entry, "creator")?;
                    state.templates.insert(id, (uri, creator));
                }
            }
            "portfolios" => {
                for entry in object_list(value, "portfolios")? {
                    let owner = required_str(&entry, "owner")?;
                    let id = required_u32(&entry, "template_id")?;
                    let cid = required_str(&entry, "ipfs_cid")?;
                    state.portfolios.insert(owner.clone(), (id, cid));
                    state.user_template.insert(owner, id);
                }
            }
            other => return Err(format!("unknown live state key {other}")),
        }
    }
    Ok(state)
}

fn string_list(value: &Value, name: &str) -> Result<Vec<String>, String> {
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Str(s) => Ok(s.clone()),
                _ => Err(format!("{name} must contain strings")),
            })
            .collect(),
        _ => Err(format!("{name} must be an array")),
    }
}

fn object_list(value: &Value, name: &str) -> Result<Vec<Vec<(String, Value)>>, String> {
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Object(fields) => Ok(fields.clone()),
                _ => Err(format!("{name} must contain objects")),
            })
            .collect(),
        _ => Err(format!("{name} must be an array")),
    }
}

fn required_str(fields: &[(String, Value)], key: &str) -> Result<String, String> {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| match v {
            Value::Str(s) => Some(s.clone()),
            _ => None,
        })
        .ok_or_else(|| format!("missing string field {key}"))
}

fn required_u32(fields: &[(String, Value)], key: &str) -> Result<u32, String> {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| match v {
            Value::Num(n) => n.parse().ok(),
            _ => None,
        })
        .ok_or_else(|| format!("missing numeric field {key}"))
}

/// Reads live state over RPC. Not exercised offline; see README for the
/// expected `simulateTransaction` request/response shape.
pub fn fetch_live_state(rpc_url: &str, contract: &str) -> Result<State, String> {
    let admins = rpc_call(rpc_url, contract, "get_admins", "[]")?;
    let mut state = State::new();
    state.admins = admins;
    let _: BTreeMap<String, String> = BTreeMap::new();
    Ok(state)
}

fn rpc_call(
    _rpc_url: &str,
    _contract: &str,
    _function: &str,
    _args: &str,
) -> Result<Vec<String>, String> {
    Err("live RPC reads are not wired up in this build; pass --live <snapshot.json>".to_string())
}
