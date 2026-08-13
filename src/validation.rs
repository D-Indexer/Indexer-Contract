use soroban_sdk::{Bytes, String};

use crate::constants::{
    MAX_FOLDER_NAME_LEN, MAX_IPFS_CID_LEN, MAX_METADATA_URI_LEN, MIN_FOLDER_NAME_LEN,
    MIN_IPFS_CID_LEN, MIN_METADATA_URI_LEN,
};
use crate::errors::ContractError;

/// Returns true when a Soroban string is within the inclusive length range.
fn validate_len(value: &String, min: u32, max: u32) -> bool {
    let len = value.len();
    len >= min && len <= max
}

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

fn is_folder_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'.'
}

fn has_only_folder_bytes(value: &String) -> bool {
    let bytes = value.to_bytes();
    for byte in bytes.iter() {
        if !is_folder_byte(byte) {
            return false;
        }
    }
    true
}

/// Validates a Folder name before it is stored.
pub fn require_valid_folder_name(folder_name: &String) -> Result<(), ContractError> {
    let bytes = folder_name.to_bytes();
    if validate_len(folder_name, MIN_FOLDER_NAME_LEN, MAX_FOLDER_NAME_LEN)
        && ends_with(&bytes, b".folder")
        && has_only_folder_bytes(folder_name)
    {
        Ok(())
    } else {
        Err(ContractError::InvalidFolderName)
    }
}

/// Validates an IPFS CID before it is anchored on-chain.
pub fn require_valid_ipfs_cid(ipfs_cid: &String) -> Result<(), ContractError> {
    let bytes = ipfs_cid.to_bytes();
    if validate_len(ipfs_cid, MIN_IPFS_CID_LEN, MAX_IPFS_CID_LEN)
        && (starts_with(&bytes, b"bafy") || starts_with(&bytes, b"Qm"))
    {
        Ok(())
    } else {
        Err(ContractError::InvalidCID)
    }
}

/// Validates template metadata URI length before registration.
pub fn require_valid_metadata_uri(metadata_uri: &String) -> Result<(), ContractError> {
    let bytes = metadata_uri.to_bytes();
    if validate_len(metadata_uri, MIN_METADATA_URI_LEN, MAX_METADATA_URI_LEN)
        && starts_with(&bytes, b"ipfs://")
    {
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
