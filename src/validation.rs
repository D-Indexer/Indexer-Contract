use soroban_sdk::String;

use crate::constants::{
    MAX_FOLDER_NAME_LEN, MAX_IPFS_CID_LEN, MAX_METADATA_URI_LEN, MIN_FOLDER_NAME_LEN,
    MIN_IPFS_CID_LEN, MIN_METADATA_URI_LEN,
};
use crate::errors::ContractError;

fn validate_len(value: &String, min: u32, max: u32) -> bool {
    let len = value.len();
    len >= min && len <= max
}

pub fn require_valid_folder_name(folder_name: &String) -> Result<(), ContractError> {
    if validate_len(folder_name, MIN_FOLDER_NAME_LEN, MAX_FOLDER_NAME_LEN) {
        Ok(())
    } else {
        Err(ContractError::InvalidFolderName)
    }
}

pub fn require_valid_ipfs_cid(ipfs_cid: &String) -> Result<(), ContractError> {
    if validate_len(ipfs_cid, MIN_IPFS_CID_LEN, MAX_IPFS_CID_LEN) {
        Ok(())
    } else {
        Err(ContractError::InvalidCID)
    }
}

pub fn require_valid_metadata_uri(metadata_uri: &String) -> Result<(), ContractError> {
    if validate_len(metadata_uri, MIN_METADATA_URI_LEN, MAX_METADATA_URI_LEN) {
        Ok(())
    } else {
        Err(ContractError::InvalidMetadataURI)
    }
}
