use soroban_sdk::{Address, Env};

use crate::errors::ContractError;
use crate::storage;

pub fn require_admin(env: &Env, caller: &Address) -> Result<(), ContractError> {
    caller.require_auth();
    if !storage::is_admin(env, caller) {
        return Err(ContractError::Unauthorized);
    }
    Ok(())
}

pub fn require_owner(env: &Env, owner: &Address) -> Result<(), ContractError> {
    owner.require_auth();
    // Folder ownership is validated by the caller passing their own address;
    // Soroban's require_auth enforces the signature.
    let _ = env;
    Ok(())
}

pub fn add_admin(env: &Env, caller: &Address, new_admin: &Address) -> Result<(), ContractError> {
    require_admin(env, caller)?;
    let mut admins = storage::get_admins(env);
    if admins.contains(new_admin) {
        return Err(ContractError::AdminAlreadyExists);
    }
    admins.push_back(new_admin.clone());
    storage::save_admins(env, &admins);
    Ok(())
}

pub fn remove_admin(env: &Env, caller: &Address, target: &Address) -> Result<(), ContractError> {
    require_admin(env, caller)?;
    let mut admins = storage::get_admins(env);
    let mut found_index: Option<u32> = None;
    for i in 0..admins.len() {
        if admins.get(i).unwrap() == *target {
            found_index = Some(i);
            break;
        }
    }
    match found_index {
        Some(i) => {
            if admins.len() == 1 {
                return Err(ContractError::CannotRemoveLastAdmin);
            }
            admins.remove(i);
            storage::save_admins(env, &admins);
            Ok(())
        }
        None => Err(ContractError::AdminNotFound),
    }
}
