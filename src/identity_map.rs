use soroban_sdk::{Address, Env, String};

use crate::access_control::{require_admin, require_owner};
use crate::constants::RELEASE_COOLDOWN_LEDGERS;
use crate::errors::ContractError;
use crate::events;
use crate::pricing;
use crate::storage;
use crate::types::FolderMeta;
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
    validation::require_valid_folder_name(folder_name)?;
    if storage::is_reserved(env, folder_name) {
        return Err(ContractError::FolderReserved);
    }
    if let Some(released_at) = storage::get_released_at(env, folder_name) {
        if env.ledger().sequence() < released_at.saturating_add(RELEASE_COOLDOWN_LEDGERS) {
            return Err(ContractError::FolderReleaseCooldown);
        }
        storage::remove_released_at(env, folder_name);
    }
    if let Some(current) = storage::get_owner_folder(env, owner) {
        match lease_state(env, &current) {
            LeaseState::Released => storage::remove_folder(env, &current, owner),
            _ => return Err(ContractError::FolderAlreadyClaimed),
        }
    }
    if let Some(previous) = storage::get_folder_owner(env, folder_name) {
        match lease_state(env, folder_name) {
            // Drops the previous owner's stale reverse mapping as well.
            LeaseState::Released => storage::remove_folder(env, folder_name, &previous),
            _ => return Err(ContractError::FolderNameTaken),
        }
    }

    // Charged before any write: a failed transfer aborts the whole invocation.
    let fee_paid = pricing::quote_claim(env, folder_name)?;
    pricing::charge(env, payer, fee_paid);
    storage::set_folder_owner(env, folder_name, owner);
    storage::set_folder_meta(
        env,
        folder_name,
        &FolderMeta {
            claimed_at: env.ledger().timestamp(),
            claimed_ledger: env.ledger().sequence(),
        },
    );
    if let Some(lease) = storage::get_lease_config(env) {
        let expiry = env
            .ledger()
            .timestamp()
            .checked_add(lease.period_secs)
            .ok_or(ContractError::InvalidRenewal)?;
        storage::set_folder_expiry(env, folder_name, expiry);
    }
    events::folder_claimed(env, owner, folder_name, fee_paid);
    Ok(())
}

/// Gives up the owner's Folder name. Both directions of the mapping, the
/// lease expiry and the owner's template selection are deleted, so the
/// owner can claim a new name right away. The released name can be claimed
/// again by anyone (the former owner included) once
/// `RELEASE_COOLDOWN_LEDGERS` have passed.
///
/// Fails with `PortfolioExists` while the owner holds a minted portfolio: the
/// portfolio is tied to the identity and must be burned first.
pub fn release_folder(env: &Env, owner: &Address) -> Result<(), ContractError> {
    require_owner(env, owner)?;
    let folder_name = storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)?;
    // A lapsed lease is already up for grabs; there is nothing to release.
    if let LeaseState::Released = lease_state(env, &folder_name) {
        return Err(ContractError::FolderNotFound);
    }
    if storage::load_portfolio(env, owner).is_some() {
        return Err(ContractError::PortfolioExists);
    }

    storage::remove_folder_owner(env, &folder_name);
    storage::remove_owner_folder(env, owner);
    storage::remove_folder_expiry(env, &folder_name);
    storage::remove_folder_meta(env, &folder_name);
    storage::remove_user_template(env, owner);
    storage::set_released_at(env, &folder_name, env.ledger().sequence());
    events::folder_released(env, owner, &folder_name);
    Ok(())
}

/// Reserves `folder_name` so it cannot be claimed. Admin only. Fails with
/// `FolderNameTaken` if the name has an owner (active or in its grace
/// period); a released lease is cleared first. Reserving an already
/// reserved name is a no-op apart from the event.
pub fn reserve_folder(
    env: &Env,
    admin: &Address,
    folder_name: &String,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    validation::require_valid_folder_name(folder_name)?;
    if let Some(previous) = storage::get_folder_owner(env, folder_name) {
        match lease_state(env, folder_name) {
            LeaseState::Released => storage::remove_folder(env, folder_name, &previous),
            _ => return Err(ContractError::FolderNameTaken),
        }
    }
    storage::set_reserved(env, folder_name);
    events::folder_reserved(env, admin, folder_name);
    Ok(())
}

/// Lifts a reservation so the name can be claimed again. Admin only.
/// Unreserving a name that is not reserved is a no-op apart from the event.
pub fn unreserve_folder(
    env: &Env,
    admin: &Address,
    folder_name: &String,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    validation::require_valid_folder_name(folder_name)?;
    storage::remove_reserved(env, folder_name);
    events::folder_unreserved(env, admin, folder_name);
    Ok(())
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

/// Claim metadata of an active name. Names claimed before `FolderMeta`
/// existed have no entry and fail with `FolderNotFound`, like unclaimed
/// names; `get_folder` tells the two apart.
pub fn get_folder_meta(env: &Env, folder_name: &String) -> Result<FolderMeta, ContractError> {
    get_folder(env, folder_name)?;
    storage::get_folder_meta(env, folder_name).ok_or(ContractError::FolderNotFound)
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
