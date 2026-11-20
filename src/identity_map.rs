use soroban_sdk::{token, Address, Env, String};

use crate::access_control::require_owner;
use crate::errors::ContractError;
use crate::events;
use crate::storage;
use crate::validation;

pub fn claim_folder(env: &Env, owner: &Address, folder_name: &String) -> Result<(), ContractError> {
    require_owner(env, owner)?;

    validation::require_valid_folder_name(folder_name)?;
    if storage::get_owner_folder(env, owner).is_some() {
        return Err(ContractError::FolderAlreadyClaimed);
    }
    if storage::get_folder_owner(env, folder_name).is_some() {
        return Err(ContractError::FolderNameTaken);
    }

    let fee_paid = match storage::get_claim_fee(env) {
        Some(fee) if fee.amount > 0 => {
            // Covered by the owner's require_auth above. A failed transfer
            // aborts the whole invocation, so no name mapping is written.
            token::Client::new(env, &fee.token).transfer(owner, &fee.treasury, &fee.amount);
            fee.amount
        }
        _ => 0,
    };

    storage::set_folder_owner(env, folder_name, owner);
    events::folder_claimed(env, owner, folder_name, fee_paid);
    Ok(())
}

pub fn get_folder(env: &Env, folder_name: &String) -> Result<Address, ContractError> {
    storage::get_folder_owner(env, folder_name).ok_or(ContractError::FolderNotFound)
}

pub fn get_identity(env: &Env, owner: &Address) -> Result<String, ContractError> {
    storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)
}
