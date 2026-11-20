use soroban_sdk::{Address, Env, Map, String, Symbol};

use crate::access_control::require_owner;
use crate::constants::MAX_RECORDS;
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

    storage::set_folder_owner(env, folder_name, owner);
    events::folder_claimed(env, owner, folder_name);
    Ok(())
}

pub fn get_folder(env: &Env, folder_name: &String) -> Result<Address, ContractError> {
    storage::get_folder_owner(env, folder_name).ok_or(ContractError::FolderNotFound)
}

pub fn get_identity(env: &Env, owner: &Address) -> Result<String, ContractError> {
    storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)
}

fn owned_folder(env: &Env, owner: &Address) -> Result<String, ContractError> {
    require_owner(env, owner)?;
    storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)
}

// ── Payment address ───────────────────────────────────────────────────────────

pub fn set_payment_address(
    env: &Env,
    owner: &Address,
    payment: &Address,
) -> Result<(), ContractError> {
    let folder_name = owned_folder(env, owner)?;
    storage::set_payment_address(env, &folder_name, payment);
    events::payment_address_set(env, owner, &folder_name, payment);
    Ok(())
}

pub fn clear_payment_address(env: &Env, owner: &Address) -> Result<(), ContractError> {
    let folder_name = owned_folder(env, owner)?;
    storage::remove_payment_address(env, &folder_name);
    events::payment_address_cleared(env, owner, &folder_name);
    Ok(())
}

/// Returns the Folder's payment address, falling back to its owner.
pub fn resolve_payment(env: &Env, folder_name: &String) -> Result<Address, ContractError> {
    let owner = get_folder(env, folder_name)?;
    Ok(storage::get_payment_address(env, folder_name).unwrap_or(owner))
}

// ── Text records ──────────────────────────────────────────────────────────────

pub fn set_record(
    env: &Env,
    owner: &Address,
    key: &Symbol,
    value: &String,
) -> Result<(), ContractError> {
    let folder_name = owned_folder(env, owner)?;
    validation::require_valid_record_key(env, key)?;
    validation::require_valid_record_value(value)?;
    let mut keys = storage::get_record_keys(env, &folder_name);
    if !keys.contains(key) {
        if keys.len() >= MAX_RECORDS {
            return Err(ContractError::RecordLimitReached);
        }
        keys.push_back(key.clone());
        storage::set_record_keys(env, &folder_name, &keys);
    }
    storage::set_record(env, &folder_name, key, value);
    events::record_set(env, owner, &folder_name, key, value);
    Ok(())
}

pub fn remove_record(env: &Env, owner: &Address, key: &Symbol) -> Result<(), ContractError> {
    let folder_name = owned_folder(env, owner)?;
    let mut keys = storage::get_record_keys(env, &folder_name);
    let index = keys
        .first_index_of(key)
        .ok_or(ContractError::RecordNotFound)?;
    keys.remove(index);
    storage::set_record_keys(env, &folder_name, &keys);
    storage::remove_record(env, &folder_name, key);
    events::record_removed(env, owner, &folder_name, key);
    Ok(())
}

pub fn get_record(env: &Env, folder_name: &String, key: &Symbol) -> Result<String, ContractError> {
    get_folder(env, folder_name)?;
    storage::get_record(env, folder_name, key).ok_or(ContractError::RecordNotFound)
}

pub fn get_records(env: &Env, folder_name: &String) -> Result<Map<Symbol, String>, ContractError> {
    get_folder(env, folder_name)?;
    let mut records = Map::new(env);
    for key in storage::get_record_keys(env, folder_name).iter() {
        if let Some(value) = storage::get_record(env, folder_name, &key) {
            records.set(key, value);
        }
    }
    Ok(records)
}
