use soroban_sdk::{xdr::ToXdr, Bytes, Env, String, Symbol};

use crate::constants::{
    CIDV0_LEN, FOLDER_SUFFIX, MAX_FOLDER_NAME_LEN, MAX_IPFS_CID_LEN, MAX_METADATA_URI_LEN,
    MAX_RECORD_KEY_LEN, MAX_RECORD_VALUE_LEN, METADATA_URI_SCHEME, MIN_FOLDER_LABEL_LEN,
    MIN_IPFS_CID_LEN,
};
use crate::errors::ContractError;

fn starts_with(bytes: &Bytes, prefix: &[u8]) -> bool {
    if bytes.len() < prefix.len() as u32 {
        return false;
    }
    let mut i = 0;
    while i < prefix.len() {
        if bytes.get(i as u32) != Some(prefix[i]) {
            return false;
        }
        i += 1;
    }
    true
}

fn ends_with(bytes: &Bytes, suffix: &[u8]) -> bool {
    let len = bytes.len();
    let suffix_len = suffix.len() as u32;
    if len < suffix_len {
        return false;
    }
    let offset = len - suffix_len;
    let mut i = 0;
    while i < suffix.len() {
        if bytes.get(offset + i as u32) != Some(suffix[i]) {
            return false;
        }
        i += 1;
    }
    true
}

fn all_bytes(bytes: &Bytes, pred: fn(u8) -> bool) -> bool {
    for byte in bytes.iter() {
        if !pred(byte) {
            return false;
        }
    }
    true
}

fn is_label_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
}

/// base58btc alphabet used by CIDv0 (no `0`, `O`, `I`, `l`).
fn is_base58_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() && !matches!(byte, b'0' | b'O' | b'I' | b'l')
}

/// RFC 4648 lowercase base32 alphabet used by CIDv1 multibase prefix `b`.
fn is_base32_lower_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || (b'2'..=b'7').contains(&byte)
}

fn is_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/')
}

/// Checks the textual shape of a CIDv0 (`Qm` + base58btc, 46 chars) or a
/// base32 CIDv1 (`ba` + lowercase base32).
fn is_valid_cid(bytes: &Bytes) -> bool {
    let len = bytes.len();
    if starts_with(bytes, b"Qm") {
        return len == CIDV0_LEN && all_bytes(bytes, is_base58_byte);
    }
    if starts_with(bytes, b"ba") {
        return (MIN_IPFS_CID_LEN..=MAX_IPFS_CID_LEN).contains(&len)
            && all_bytes(bytes, is_base32_lower_byte);
    }
    false
}

/// Validates a Folder name: `<label>.folder`, where the label is at least
/// `MIN_FOLDER_LABEL_LEN` bytes of `[a-z0-9-]` and does not start or end with
/// a hyphen. The whole name is at most `MAX_FOLDER_NAME_LEN` bytes.
pub fn require_valid_folder_name(folder_name: &String) -> Result<(), ContractError> {
    let bytes = folder_name.to_bytes();
    let len = bytes.len();
    let suffix_len = FOLDER_SUFFIX.len() as u32;
    if len > MAX_FOLDER_NAME_LEN
        || len < MIN_FOLDER_LABEL_LEN + suffix_len
        || !ends_with(&bytes, FOLDER_SUFFIX)
    {
        return Err(ContractError::InvalidFolderName);
    }
    let label = bytes.slice(0..len - suffix_len);
    if !all_bytes(&label, is_label_byte)
        || label.first() == Some(b'-')
        || label.last() == Some(b'-')
    {
        return Err(ContractError::InvalidFolderName);
    }
    Ok(())
}

/// Validates an IPFS CID (CIDv0 or base32 CIDv1) before it is anchored on-chain.
pub fn require_valid_ipfs_cid(ipfs_cid: &String) -> Result<(), ContractError> {
    if is_valid_cid(&ipfs_cid.to_bytes()) {
        Ok(())
    } else {
        Err(ContractError::InvalidCID)
    }
}

/// Validates a template metadata URI: `ipfs://<cid>` optionally followed by
/// `/<path>` made of URL-safe characters, at most `MAX_METADATA_URI_LEN` bytes.
pub fn require_valid_metadata_uri(metadata_uri: &String) -> Result<(), ContractError> {
    let bytes = metadata_uri.to_bytes();
    let len = bytes.len();
    let scheme_len = METADATA_URI_SCHEME.len() as u32;
    if len > MAX_METADATA_URI_LEN || !starts_with(&bytes, METADATA_URI_SCHEME) {
        return Err(ContractError::InvalidMetadataURI);
    }
    let rest = bytes.slice(scheme_len..len);
    let cid_end = rest.iter().position(|b| b == b'/').map(|p| p as u32);
    let (cid, path) = match cid_end {
        Some(end) => (rest.slice(0..end), rest.slice(end..rest.len())),
        None => (rest.clone(), Bytes::new(bytes.env())),
    };
    if is_valid_cid(&cid) && all_bytes(&path, is_path_byte) {
        Ok(())
    } else {
        Err(ContractError::InvalidMetadataURI)
    }
}

/// Validates template IDs exposed through the public API.
pub fn require_valid_template_id(template_id: u32) -> Result<(), ContractError> {
    if template_id == 0 {
        Err(ContractError::InvalidTemplateID)
    } else {
        Ok(())
    }
}

fn is_record_key_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
}

/// Validates a record key: `[a-z0-9_]{1,16}`. The documented allowlist keys
/// (see `docs/STORAGE.md`) all match this pattern.
pub fn require_valid_record_key(env: &Env, key: &Symbol) -> Result<(), ContractError> {
    // ScVal XDR of a symbol: 4-byte type tag, 4-byte big-endian length, then the characters.
    let xdr = key.clone().to_xdr(env);
    if xdr.len() < 8 {
        return Err(ContractError::InvalidRecordKey);
    }
    let mut len: u32 = 0;
    for byte in xdr.slice(4..8).iter() {
        len = (len << 8) | byte as u32;
    }
    if len == 0 || len > MAX_RECORD_KEY_LEN || 8 + len > xdr.len() {
        return Err(ContractError::InvalidRecordKey);
    }
    if !all_bytes(&xdr.slice(8..8 + len), is_record_key_byte) {
        return Err(ContractError::InvalidRecordKey);
    }
    Ok(())
}

/// Validates a record value: 1 to 256 bytes of printable ASCII.
pub fn require_valid_record_value(value: &String) -> Result<(), ContractError> {
    let bytes = value.to_bytes();
    let len = bytes.len();
    if len == 0 || len > MAX_RECORD_VALUE_LEN || !all_bytes(&bytes, |b| (0x20..=0x7e).contains(&b))
    {
        return Err(ContractError::InvalidRecordValue);
    }
    Ok(())
}
