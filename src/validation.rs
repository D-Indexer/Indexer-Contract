use soroban_sdk::String;

use crate::constants::{MAX_FOLDER_NAME_LEN, MAX_IPFS_CID_LEN, MAX_METADATA_URI_LEN};
use crate::errors::ContractError;
use crate::validation_core;

/// Copies `s` into a stack buffer of `N` bytes and runs `check` on it.
/// Strings longer than `N` are rejected without being copied.
fn check_bounded<const N: usize>(s: &String, check: fn(&[u8]) -> bool) -> bool {
    let len = s.len() as usize;
    if len > N {
        return false;
    }
    let mut buf = [0u8; N];
    s.copy_into_slice(&mut buf[..len]);
    check(&buf[..len])
}

/// Validates a Folder name: `<label>.folder`, where the label is at least
/// `MIN_FOLDER_LABEL_LEN` bytes of `[a-z0-9-]` and does not start or end with
/// a hyphen. The whole name is at most `MAX_FOLDER_NAME_LEN` bytes.
pub fn require_valid_folder_name(folder_name: &String) -> Result<(), ContractError> {
    if check_bounded::<{ MAX_FOLDER_NAME_LEN as usize }>(
        folder_name,
        validation_core::is_valid_folder_name,
    ) {
        Ok(())
    } else {
        Err(ContractError::InvalidFolderName)
    }
}

/// Validates an IPFS CID (CIDv0 or base32 CIDv1) before it is anchored on-chain.
pub fn require_valid_ipfs_cid(ipfs_cid: &String) -> Result<(), ContractError> {
    if check_bounded::<{ MAX_IPFS_CID_LEN as usize }>(ipfs_cid, validation_core::is_valid_cid) {
        Ok(())
    } else {
        Err(ContractError::InvalidCID)
    }
}

/// Validates a template metadata URI: `ipfs://<cid>` optionally followed by
/// `/<path>` made of URL-safe characters, at most `MAX_METADATA_URI_LEN` bytes.
pub fn require_valid_metadata_uri(metadata_uri: &String) -> Result<(), ContractError> {
    if check_bounded::<{ MAX_METADATA_URI_LEN as usize }>(
        metadata_uri,
        validation_core::is_valid_metadata_uri,
    ) {
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
