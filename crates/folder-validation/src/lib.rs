//! Shared, dependency-free validation rules for Folder names, IPFS CIDs and
//! template metadata URIs.
//!
//! This crate is the single source of truth used by the Soroban contract, the
//! backend and (through `folder-validation-js`) the frontend. It is `no_std`
//! and operates on `&[u8]` so it can run anywhere.

#![no_std]

/// Required Folder name suffix.
pub const FOLDER_SUFFIX: &[u8] = b".folder";
/// Minimum length of the label before `.folder` (e.g. `bob` in `bob.folder`).
pub const MIN_FOLDER_LABEL_LEN: usize = 3;
/// Maximum accepted Folder name length, suffix included.
pub const MAX_FOLDER_NAME_LEN: usize = 64;
/// Exact length of a CIDv0 (`Qm...`, base58btc sha2-256).
pub const CIDV0_LEN: usize = 46;
/// Minimum accepted IPFS CID length.
pub const MIN_IPFS_CID_LEN: usize = 46;
/// Maximum accepted IPFS CID length.
pub const MAX_IPFS_CID_LEN: usize = 128;
/// Required template metadata URI scheme.
pub const METADATA_URI_SCHEME: &[u8] = b"ipfs://";
/// Maximum accepted template metadata URI length.
pub const MAX_METADATA_URI_LEN: usize = 256;

fn starts_with(bytes: &[u8], prefix: &[u8]) -> bool {
    bytes.len() >= prefix.len() && &bytes[..prefix.len()] == prefix
}

fn ends_with(bytes: &[u8], suffix: &[u8]) -> bool {
    bytes.len() >= suffix.len() && &bytes[bytes.len() - suffix.len()..] == suffix
}

fn all_bytes(bytes: &[u8], pred: fn(u8) -> bool) -> bool {
    bytes.iter().all(|byte| pred(*byte))
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
pub fn is_valid_cid(bytes: &[u8]) -> bool {
    if starts_with(bytes, b"Qm") {
        return bytes.len() == CIDV0_LEN && all_bytes(bytes, is_base58_byte);
    }
    if starts_with(bytes, b"ba") {
        return (MIN_IPFS_CID_LEN..=MAX_IPFS_CID_LEN).contains(&bytes.len())
            && all_bytes(bytes, is_base32_lower_byte);
    }
    false
}

/// Validates a Folder name: `<label>.folder`, where the label is at least
/// `MIN_FOLDER_LABEL_LEN` bytes of `[a-z0-9-]` and does not start or end with
/// a hyphen. The whole name is at most `MAX_FOLDER_NAME_LEN` bytes.
pub fn is_valid_folder_name(bytes: &[u8]) -> bool {
    let len = bytes.len();
    let suffix_len = FOLDER_SUFFIX.len();
    if len > MAX_FOLDER_NAME_LEN
        || len < MIN_FOLDER_LABEL_LEN + suffix_len
        || !ends_with(bytes, FOLDER_SUFFIX)
    {
        return false;
    }
    let label = &bytes[..len - suffix_len];
    all_bytes(label, is_label_byte) && label.first() != Some(&b'-') && label.last() != Some(&b'-')
}

/// Validates a template metadata URI: `ipfs://<cid>` optionally followed by
/// `/<path>` made of URL-safe characters, at most `MAX_METADATA_URI_LEN` bytes.
pub fn is_valid_metadata_uri(bytes: &[u8]) -> bool {
    let len = bytes.len();
    let scheme_len = METADATA_URI_SCHEME.len();
    if len > MAX_METADATA_URI_LEN || !starts_with(bytes, METADATA_URI_SCHEME) {
        return false;
    }
    let rest = &bytes[scheme_len..];
    let (cid, path) = match rest.iter().position(|b| *b == b'/') {
        Some(end) => (&rest[..end], &rest[end..]),
        None => (rest, &rest[rest.len()..]),
    };
    is_valid_cid(cid) && all_bytes(path, is_path_byte)
}

#[cfg(test)]
mod tests;
