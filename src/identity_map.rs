use soroban_sdk::{Address, Env, String};

use crate::access_control::require_owner;
use crate::errors::ContractError;
use crate::events;
use crate::storage;
use crate::validation;

/// Validated, side-effect-free description of a folder claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimFolderPlan {
    pub owner: Address,
    pub folder_name: String,
}

/// Pure validation phase: checks auth and state, performs no writes.
pub fn plan_claim_folder(
    env: &Env,
    owner: &Address,
    folder_name: &String,
) -> Result<ClaimFolderPlan, ContractError> {
    require_owner(env, owner)?;

    validation::require_valid_folder_name(folder_name)?;
    if storage::get_owner_folder(env, owner).is_some() {
        return Err(ContractError::FolderAlreadyClaimed);
    }
    if storage::get_folder_owner(env, folder_name).is_some() {
        return Err(ContractError::FolderNameTaken);
    }

    Ok(ClaimFolderPlan {
        owner: owner.clone(),
        folder_name: folder_name.clone(),
    })
}

/// Write phase: applies a plan and emits its events.
pub fn apply_claim_folder(env: &Env, plan: &ClaimFolderPlan) {
    storage::set_folder_owner(env, &plan.folder_name, &plan.owner);
    events::folder_claimed(env, &plan.owner, &plan.folder_name);
}

pub fn claim_folder(env: &Env, owner: &Address, folder_name: &String) -> Result<(), ContractError> {
    let plan = plan_claim_folder(env, owner, folder_name)?;
    apply_claim_folder(env, &plan);
    Ok(())
}

pub fn get_folder(env: &Env, folder_name: &String) -> Result<Address, ContractError> {
    storage::get_folder_owner(env, folder_name).ok_or(ContractError::FolderNotFound)
}

pub fn get_identity(env: &Env, owner: &Address) -> Result<String, ContractError> {
    storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)
}
