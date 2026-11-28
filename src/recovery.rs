//! Guardian-based identity recovery.
//!
//! An owner nominates up to `MAX_GUARDIANS` guardians and a threshold. If the
//! owner loses their key, a guardian proposes a `new_owner`. Once `threshold`
//! guardians have approved and `RECOVERY_DELAY` ledgers have passed, anyone can
//! execute the recovery, which re-keys the Folder, portfolio, template
//! selection and guardian set to `new_owner`. Until then the current owner can
//! cancel, which is the defence against colluding guardians.

use soroban_sdk::{Address, Env, Vec};

use crate::access_control::require_owner;
use crate::constants::{MAX_GUARDIANS, RECOVERY_DELAY};
use crate::errors::ContractError;
use crate::events;
use crate::storage;
use crate::types::{GuardianConfig, Recovery};

pub fn set_guardians(
    env: &Env,
    owner: &Address,
    guardians: &Vec<Address>,
    threshold: u32,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;
    storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)?;
    // Changing guardians mid-recovery would let a key thief swap them out;
    // the owner must cancel first.
    if storage::get_recovery(env, owner).is_some() {
        return Err(ContractError::RecoveryInProgress);
    }
    let len = guardians.len();
    if len > MAX_GUARDIANS || threshold == 0 || threshold > len {
        return Err(ContractError::InvalidGuardians);
    }
    for (i, guardian) in guardians.iter().enumerate() {
        if &guardian == owner || guardians.slice(i as u32 + 1..len).contains(&guardian) {
            return Err(ContractError::InvalidGuardians);
        }
    }
    storage::set_guardians(
        env,
        owner,
        &GuardianConfig {
            guardians: guardians.clone(),
            threshold,
        },
    );
    Ok(())
}

fn require_guardian(env: &Env, guardian: &Address, owner: &Address) -> Result<(), ContractError> {
    guardian.require_auth();
    let config = storage::get_guardians(env, owner).ok_or(ContractError::NotGuardian)?;
    if !config.guardians.contains(guardian) {
        return Err(ContractError::NotGuardian);
    }
    Ok(())
}

/// `new_owner` must be a fresh address: no Folder and no portfolio to clobber.
fn require_free_target(
    env: &Env,
    owner: &Address,
    new_owner: &Address,
) -> Result<(), ContractError> {
    if new_owner == owner
        || storage::get_owner_folder(env, new_owner).is_some()
        || storage::load_portfolio(env, new_owner).is_some()
    {
        return Err(ContractError::FolderAlreadyClaimed);
    }
    Ok(())
}

pub fn initiate_recovery(
    env: &Env,
    guardian: &Address,
    owner: &Address,
    new_owner: &Address,
) -> Result<(), ContractError> {
    require_guardian(env, guardian, owner)?;
    storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)?;
    if storage::get_recovery(env, owner).is_some() {
        return Err(ContractError::RecoveryInProgress);
    }
    require_free_target(env, owner, new_owner)?;

    let mut approvals = Vec::new(env);
    approvals.push_back(guardian.clone());
    storage::set_recovery(
        env,
        owner,
        &Recovery {
            new_owner: new_owner.clone(),
            approvals,
            initiated_ledger: env.ledger().sequence(),
        },
    );
    events::recovery_initiated(env, guardian, owner, new_owner);
    Ok(())
}

pub fn approve_recovery(
    env: &Env,
    guardian: &Address,
    owner: &Address,
) -> Result<(), ContractError> {
    require_guardian(env, guardian, owner)?;
    let mut recovery = storage::get_recovery(env, owner).ok_or(ContractError::RecoveryNotFound)?;
    if recovery.approvals.contains(guardian) {
        return Err(ContractError::AlreadyApproved);
    }
    recovery.approvals.push_back(guardian.clone());
    storage::set_recovery(env, owner, &recovery);
    events::recovery_approved(env, guardian, owner, recovery.approvals.len());
    Ok(())
}

/// Needs no signature: the owner has lost their key. Safety comes from the
/// guardian threshold and the delay during which the owner can cancel.
pub fn execute_recovery(env: &Env, owner: &Address) -> Result<Address, ContractError> {
    let recovery = storage::get_recovery(env, owner).ok_or(ContractError::RecoveryNotFound)?;
    let config = storage::get_guardians(env, owner).ok_or(ContractError::RecoveryNotFound)?;
    let ready_at = recovery.initiated_ledger.saturating_add(RECOVERY_DELAY);
    if recovery.approvals.len() < config.threshold || env.ledger().sequence() < ready_at {
        return Err(ContractError::RecoveryNotReady);
    }
    let folder_name = storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)?;
    // A released lease may have handed the name to someone else.
    if storage::get_folder_owner(env, &folder_name).as_ref() != Some(owner) {
        return Err(ContractError::FolderNotFound);
    }
    // new_owner may have claimed a Folder since the recovery was initiated.
    require_free_target(env, owner, &recovery.new_owner)?;

    storage::rekey_identity(env, &folder_name, owner, &recovery.new_owner);
    events::recovery_executed(env, owner, &recovery.new_owner, &folder_name);
    Ok(recovery.new_owner)
}

pub fn cancel_recovery(env: &Env, owner: &Address) -> Result<(), ContractError> {
    require_owner(env, owner)?;
    let recovery = storage::get_recovery(env, owner).ok_or(ContractError::RecoveryNotFound)?;
    storage::remove_recovery(env, owner);
    events::recovery_cancelled(env, owner, &recovery.new_owner);
    Ok(())
}
