use soroban_sdk::{Address, Env, Vec};

use crate::constants::{ADMIN_TIMELOCK_LEDGERS, MAX_ADMINS, MAX_PENDING_ADMIN_CHANGES};
use crate::errors::ContractError;
use crate::events;
use crate::storage;
use crate::types::{AdminChange, AdminOp};

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

/// Queues an admin change and returns its op ID. The change becomes
/// executable once `ADMIN_TIMELOCK_LEDGERS` ledgers have elapsed.
pub fn queue_admin_change(
    env: &Env,
    caller: &Address,
    op: AdminOp,
    target: &Address,
) -> Result<u32, ContractError> {
    require_admin(env, caller)?;
    let mut pending = storage::get_pending_admin_changes(env);
    if pending.len() >= MAX_PENDING_ADMIN_CHANGES {
        return Err(ContractError::AdminChangeQueueFull);
    }
    let op_id = storage::next_admin_change_id(env);
    let eta_ledger = env.ledger().sequence() + ADMIN_TIMELOCK_LEDGERS;
    pending.push_back((
        op_id,
        AdminChange {
            op,
            target: target.clone(),
            eta_ledger,
        },
    ));
    storage::save_pending_admin_changes(env, &pending);
    events::admin_change_queued(env, caller, op_id, target, eta_ledger);
    Ok(op_id)
}

/// Executes a queued admin change once its ETA has passed. The entry is
/// removed before applying the change, so it can never run twice.
pub fn execute_admin_change(env: &Env, caller: &Address, op_id: u32) -> Result<(), ContractError> {
    require_admin(env, caller)?;
    let pending = storage::get_pending_admin_changes(env);
    let mut found: Option<(u32, AdminChange)> = None;
    let mut remaining: Vec<(u32, AdminChange)> = Vec::new(env);
    for i in 0..pending.len() {
        let entry = pending.get(i).unwrap();
        if entry.0 == op_id {
            found = Some(entry);
        } else {
            remaining.push_back(entry);
        }
    }
    let (_, change) = found.ok_or(ContractError::AdminChangeNotFound)?;
    if env.ledger().sequence() < change.eta_ledger {
        return Err(ContractError::AdminChangeNotReady);
    }
    storage::save_pending_admin_changes(env, &remaining);
    match change.op {
        AdminOp::Add => apply_add_admin(env, caller, &change.target)?,
        AdminOp::Remove => apply_remove_admin(env, caller, &change.target)?,
    }
    events::admin_change_executed(env, caller, op_id, &change.target);
    Ok(())
}

/// Cancels a queued admin change that has not been executed yet.
pub fn cancel_admin_change(env: &Env, caller: &Address, op_id: u32) -> Result<(), ContractError> {
    require_admin(env, caller)?;
    let pending = storage::get_pending_admin_changes(env);
    let mut found: Option<AdminChange> = None;
    let mut remaining: Vec<(u32, AdminChange)> = Vec::new(env);
    for i in 0..pending.len() {
        let entry = pending.get(i).unwrap();
        if entry.0 == op_id {
            found = Some(entry.1);
        } else {
            remaining.push_back(entry);
        }
    }
    let change = found.ok_or(ContractError::AdminChangeNotFound)?;
    storage::save_pending_admin_changes(env, &remaining);
    events::admin_change_cancelled(env, caller, op_id, &change.target);
    Ok(())
}

fn apply_add_admin(env: &Env, caller: &Address, new_admin: &Address) -> Result<(), ContractError> {
    if storage::is_admin(env, new_admin) {
        return Err(ContractError::AdminAlreadyExists);
    }
    if storage::admin_count(env) >= MAX_ADMINS {
        return Err(ContractError::TooManyAdmins);
    }
    storage::add_admin_entry(env, new_admin);
    events::admin_added(env, caller, new_admin);
    Ok(())
}

fn apply_remove_admin(env: &Env, caller: &Address, target: &Address) -> Result<(), ContractError> {
    if !storage::is_admin(env, target) {
        return Err(ContractError::AdminNotFound);
    }
    if storage::admin_count(env) == 1 {
        return Err(ContractError::CannotRemoveLastAdmin);
    }
    storage::remove_admin_entry(env, target);
    events::admin_removed(env, caller, target);
    Ok(())
}
