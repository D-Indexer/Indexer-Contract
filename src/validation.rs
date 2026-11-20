use soroban_sdk::{Bytes, String};

use crate::errors::ContractError;

/// Copies a Soroban `Bytes` value into a fixed-size stack buffer so the shared
/// validator can run on `&[u8]` without allocating. Inputs longer than the
/// buffer are rejected by the validators themselves (all limits are smaller).
fn with_bytes<R>(bytes: &Bytes, f: impl FnOnce(&[u8]) -> R) -> R {
    const MAX: usize = 256;
    let mut buf = [0u8; MAX];
    let len = bytes.len() as usize;
    if len > MAX {
        return f(&[]);
    }
    for (i, byte) in bytes.iter().enumerate() {
        buf[i] = byte;
    }
    f(&buf[..len])
}

/// Validates a Folder name: `<label>.folder`, where the label is at least
/// `MIN_FOLDER_LABEL_LEN` bytes of `[a-z0-9-]` and does not start or end with
/// a hyphen. The whole name is at most `MAX_FOLDER_NAME_LEN` bytes.
///
/// The rules themselves live in the shared `folder-validation` crate.
pub fn require_valid_folder_name(folder_name: &String) -> Result<(), ContractError> {
    if folder_validation::is_valid_folder_name(&folder_name.to_bytes().to_alloc_vec()) {
        Ok(())
    } else {
        Err(ContractError::InvalidFolderName)
    }
}

/// Validates an IPFS CID (CIDv0 or base32 CIDv1) before it is anchored on-chain.
pub fn require_valid_ipfs_cid(ipfs_cid: &String) -> Result<(), ContractError> {
    if folder_validation::is_valid_cid(&ipfs_cid.to_bytes().to_alloc_vec()) {
        Ok(())
    } else {
        Err(ContractError::InvalidCID)
    }
}

/// Validates a template metadata URI: `ipfs://<cid>` optionally followed by
/// `/<path>` made of URL-safe characters, at most `MAX_METADATA_URI_LEN` bytes.
pub fn require_valid_metadata_uri(metadata_uri: &String) -> Result<(), ContractError> {
    if folder_validation::is_valid_metadata_uri(&metadata_uri.to_bytes().to_alloc_vec()) {
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
