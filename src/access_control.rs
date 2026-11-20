use soroban_sdk::{Address, Env};

use crate::constants::{MAX_ADMINS, MIN_ADMINS_AFTER_RESIGN};
use crate::errors::ContractError;
use crate::events;
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
    if admins.len() >= MAX_ADMINS {
        return Err(ContractError::TooManyAdmins);
    }
    admins.push_back(new_admin.clone());
    storage::save_admins(env, &admins);
    events::admin_added(env, caller, new_admin);
    Ok(())
}

pub fn remove_admin(env: &Env, caller: &Address, target: &Address) -> Result<(), ContractError> {
    require_admin(env, caller)?;
    let mut admins = storage::get_admins(env);
    let mut found_index: Option<u32> = None;
    for i in 0..admins.len() {
        if admins.get(i) == Some(target.clone()) {
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
            events::admin_removed(env, caller, target);
            Ok(())
        }
        None => Err(ContractError::AdminNotFound),
    }
}

/// Lets an admin remove themselves. Refuses when the remaining admin count
/// would drop below `MIN_ADMINS_AFTER_RESIGN`. Emits `adm_rm` with
/// `admin == target == caller`.
pub fn resign_admin(env: &Env, admin: &Address) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    let mut admins = storage::get_admins(env);
    let mut found_index: Option<u32> = None;
    for i in 0..admins.len() {
        if admins.get(i) == Some(admin.clone()) {
            found_index = Some(i);
            break;
        }
    }
    match found_index {
        Some(i) => {
            if admins.len() <= MIN_ADMINS_AFTER_RESIGN {
                return Err(ContractError::AdminQuorumTooLow);
            }
            admins.remove(i);
            storage::save_admins(env, &admins);
            events::admin_removed(env, admin, admin);
            Ok(())
        }
        None => Err(ContractError::AdminNotFound),
    }
}
