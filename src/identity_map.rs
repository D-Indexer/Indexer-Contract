use soroban_sdk::{token, Address, Env, String};

use crate::access_control::require_owner;
use crate::errors::ContractError;
use crate::events;
use crate::pricing;
use crate::storage;
use crate::validation;

/// Lease state of a Folder name that has an owner.
enum LeaseState {
    /// Permanent, or leased and not yet expired.
    Active,
    /// Expired but inside the grace period: only the owner can renew.
    Grace,
    /// Expired and past the grace period: anyone can claim it.
    Released,
}

fn lease_state(env: &Env, folder_name: &String) -> LeaseState {
    let Some(expiry) = storage::get_folder_expiry(env, folder_name) else {
        return LeaseState::Active;
    };
    let now = env.ledger().timestamp();
    if now < expiry {
        return LeaseState::Active;
    }
    // Leases granted before leases were disabled keep expiring, with no grace.
    let grace = storage::get_lease_config(env).map_or(0, |c| c.grace_secs);
    if now < expiry.saturating_add(grace) {
        LeaseState::Grace
    } else {
        LeaseState::Released
    }
}

pub fn claim_folder(env: &Env, owner: &Address, folder_name: &String) -> Result<(), ContractError> {
    require_owner(env, owner)?;
    claim(env, owner, owner, folder_name)
}

/// Claim paid by `sponsor` and owned by `owner`; both must authorize.
pub fn sponsored_claim(
    env: &Env,
    sponsor: &Address,
    owner: &Address,
    folder_name: &String,
) -> Result<(), ContractError> {
    sponsor.require_auth();
    require_owner(env, owner)?;
    claim(env, sponsor, owner, folder_name)?;
    events::claim_sponsored(env, sponsor, owner, folder_name);
    Ok(())
}

fn claim(
    env: &Env,
    payer: &Address,
    owner: &Address,
    folder_name: &String,
) -> Result<(), ContractError> {
    let plan = check_claim(env, owner, folder_name)?;
    apply_claim(env, payer, owner, folder_name, plan);
    Ok(())
}

/// Result of a successful `check_claim`: the clean-up and lease expiry
/// `apply_claim` has to write. Holding one means every claim rule passed.
pub(crate) struct ClaimPlan {
    /// The owner's own released name, to drop before the new claim.
    release_current: Option<String>,
    /// Previous holder of `folder_name`, whose released mapping is dropped.
    release_previous: Option<Address>,
    /// Lease expiry for the new name, when leases are enabled.
    expiry: Option<u64>,
}

/// Validation phase of a claim: checks every rule without writing to
/// storage. Reads extend the TTL of entries they touch, nothing else.
pub(crate) fn check_claim(
    env: &Env,
    owner: &Address,
    folder_name: &String,
) -> Result<ClaimPlan, ContractError> {
    validation::require_valid_folder_name(folder_name)?;
    let mut release_current = None;
    if let Some(current) = storage::get_owner_folder(env, owner) {
        match lease_state(env, &current) {
            LeaseState::Released => release_current = Some(current),
            _ => return Err(ContractError::FolderAlreadyClaimed),
        }
    }
    let mut release_previous = None;
    if let Some(previous) = storage::get_folder_owner(env, folder_name) {
        match lease_state(env, folder_name) {
            LeaseState::Released => release_previous = Some(previous),
            _ => return Err(ContractError::FolderNameTaken),
        }
    }
    let expiry = match storage::get_lease_config(env) {
        Some(lease) => Some(
            env.ledger()
                .timestamp()
                .checked_add(lease.period_secs)
                .ok_or(ContractError::InvalidRenewal)?,
        ),
        None => None,
    };
    Ok(ClaimPlan {
        release_current,
        release_previous,
        expiry,
    })
}

/// Write phase of a claim: charges the fee, writes the mappings and emits
/// `folder_cl`. Only call it with a plan from `check_claim` in the same
/// invocation, after the caller's `require_auth`.
pub(crate) fn apply_claim(
    env: &Env,
    payer: &Address,
    owner: &Address,
    folder_name: &String,
    plan: ClaimPlan,
) {
    if let Some(current) = plan.release_current {
        storage::remove_folder(env, &current, owner);
    }
    if let Some(previous) = plan.release_previous {
        // Drops the previous owner's stale reverse mapping as well.
        storage::remove_folder(env, folder_name, &previous);
    }

    let fee_paid = match storage::get_claim_fee(env) {
        Some(fee) if fee.amount > 0 => {
            // Covered by the payer's require_auth (the owner, or the sponsor
            // in `sponsored_claim`). A failed transfer aborts the whole
            // invocation, so no name mapping is written.
            token::Client::new(env, &fee.token).transfer(payer, &fee.treasury, &fee.amount);
            fee.amount
        }
        _ => 0,
    };

    storage::set_folder_owner(env, folder_name, owner);
    if let Some(expiry) = plan.expiry {
        storage::set_folder_expiry(env, folder_name, expiry);
    }
    events::folder_claimed(env, owner, folder_name, fee_paid);
}

/// Extends the owner's lease by `periods` lease periods, counted from the
/// current expiry. Allowed before expiry and during the grace period.
pub fn renew_folder(env: &Env, owner: &Address, periods: u32) -> Result<u64, ContractError> {
    require_owner(env, owner)?;
    let lease = storage::get_lease_config(env).ok_or(ContractError::LeasesDisabled)?;
    if periods == 0 {
        return Err(ContractError::InvalidRenewal);
    }
    let folder_name = storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)?;
    // Names claimed in permanent mode have no expiry and cannot be renewed.
    let expiry =
        storage::get_folder_expiry(env, &folder_name).ok_or(ContractError::InvalidRenewal)?;
    if let LeaseState::Released = lease_state(env, &folder_name) {
        return Err(ContractError::FolderNotFound);
    }

    let new_expiry = lease
        .period_secs
        .checked_mul(periods as u64)
        .and_then(|extra| expiry.checked_add(extra))
        .ok_or(ContractError::InvalidRenewal)?;
    let fee = pricing::quote_claim(env, &folder_name)?
        .checked_mul(periods as i128)
        .ok_or(ContractError::InvalidRenewal)?;
    pricing::charge(env, owner, fee);
    storage::set_folder_expiry(env, &folder_name, new_expiry);
    events::folder_renewed(env, owner, &folder_name, new_expiry);
    Ok(new_expiry)
}

pub fn get_folder(env: &Env, folder_name: &String) -> Result<Address, ContractError> {
    let owner = storage::get_folder_owner(env, folder_name).ok_or(ContractError::FolderNotFound)?;
    match lease_state(env, folder_name) {
        LeaseState::Active => Ok(owner),
        LeaseState::Grace => Err(ContractError::FolderExpired),
        LeaseState::Released => Err(ContractError::FolderNotFound),
    }
}

pub fn get_identity(env: &Env, owner: &Address) -> Result<String, ContractError> {
    let folder_name = storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)?;
    match lease_state(env, &folder_name) {
        LeaseState::Active => Ok(folder_name),
        LeaseState::Grace => Err(ContractError::FolderExpired),
        LeaseState::Released => Err(ContractError::FolderNotFound),
    }
}

pub fn get_folder_expiry(env: &Env, folder_name: &String) -> Option<u64> {
    storage::get_folder_expiry(env, folder_name)
}
