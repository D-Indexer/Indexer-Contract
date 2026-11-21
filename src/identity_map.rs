use soroban_sdk::{xdr::ToXdr, Address, Bytes, BytesN, Env, String};

use crate::access_control::require_owner;
use crate::constants::{MIN_REVEAL_DELAY, REVEAL_WINDOW};
use crate::errors::ContractError;
use crate::events;
use crate::pricing;
use crate::storage;
use crate::types::ClaimCommitment;
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

/// `sha256(folder_name_bytes || salt || owner_xdr)`. Binding the owner means a
/// commitment copied from the ledger cannot be revealed by anyone else.
pub fn claim_commitment(
    env: &Env,
    owner: &Address,
    folder_name: &String,
    salt: &BytesN<32>,
) -> BytesN<32> {
    let mut preimage = Bytes::new(env);
    preimage.append(&folder_name.to_bytes());
    preimage.append(&Bytes::from(salt.clone()));
    preimage.append(&owner.clone().to_xdr(env));
    env.crypto().sha256(&preimage).into()
}

/// Phase 1 of a front-running-resistant claim: records a commitment that
/// hides the name. A new commit replaces the owner's previous one.
pub fn commit_claim(
    env: &Env,
    owner: &Address,
    commitment: &BytesN<32>,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;
    storage::set_claim_commit(
        env,
        owner,
        &ClaimCommitment {
            commitment: commitment.clone(),
            ledger: env.ledger().sequence(),
        },
    );
    Ok(())
}

/// Phase 2: opens the owner's commitment and runs the normal claim path. The
/// commit is consumed; if the claim fails the whole call reverts and the
/// commit is kept.
pub fn reveal_claim(
    env: &Env,
    owner: &Address,
    folder_name: &String,
    salt: &BytesN<32>,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;
    let commit = storage::get_claim_commit(env, owner).ok_or(ContractError::CommitmentNotFound)?;
    let now = env.ledger().sequence();
    if now < commit.ledger.saturating_add(MIN_REVEAL_DELAY) {
        return Err(ContractError::RevealTooEarly);
    }
    if now > commit.ledger.saturating_add(REVEAL_WINDOW) {
        return Err(ContractError::CommitmentNotFound);
    }
    if claim_commitment(env, owner, folder_name, salt) != commit.commitment {
        return Err(ContractError::CommitmentMismatch);
    }
    storage::remove_claim_commit(env, owner);
    claim(env, owner, owner, folder_name)
}

fn claim(
    env: &Env,
    payer: &Address,
    owner: &Address,
    folder_name: &String,
) -> Result<(), ContractError> {
    validation::require_valid_folder_name(folder_name)?;
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

    // Covered by the payer's require_auth. A failed transfer aborts the whole
    // invocation, so no name mapping is written.
    let fee_paid = pricing::quote_claim(env, folder_name)?;
    pricing::charge(env, payer, fee_paid);

    storage::set_folder_owner(env, folder_name, owner);
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
