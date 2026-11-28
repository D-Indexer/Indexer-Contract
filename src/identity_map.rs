use soroban_sdk::{xdr::ToXdr, Address, Bytes, BytesN, Env, String};

use crate::access_control::{require_admin, require_owner};
use crate::constants::{MAX_REVOKE_REASON, MIN_REVEAL_DELAY, REVEAL_WINDOW};
use crate::errors::ContractError;
use crate::events;
use crate::pricing;
use crate::storage;
use crate::types::{ClaimCommitment, Revocation};
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
    if storage::is_blocked(env, owner) {
        return Err(ContractError::AddressBlocked);
    }
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
    if storage::is_blocked(env, owner) || storage::is_blocked(env, sponsor) {
        return Err(ContractError::AddressBlocked);
    }
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
    if storage::get_revocation(env, folder_name).is_some() {
        return Err(ContractError::FolderRevoked);
    }
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
}

/// Extends the owner's lease by `periods` lease periods, counted from the
/// current expiry. Allowed before expiry and during the grace period.
pub fn renew_folder(env: &Env, owner: &Address, periods: u32) -> Result<u64, ContractError> {
    require_owner(env, owner)?;
    if storage::is_blocked(env, owner) {
        return Err(ContractError::AddressBlocked);
    }
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

/// Admin removal of an abusive or impersonating name. Deletes both identity
/// mappings and the owner's portfolio, template selection and pending
/// recovery, then reserves the name under `Revoked` so nobody (the admin
/// included) can claim it. The former owner may claim a different name.
pub fn revoke_folder(
    env: &Env,
    admin: &Address,
    folder_name: &String,
    reason_code: u32,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    if reason_code == 0 || reason_code > MAX_REVOKE_REASON {
        return Err(ContractError::InvalidReasonCode);
    }
    let owner = storage::get_folder_owner(env, folder_name).ok_or(ContractError::FolderNotFound)?;

    storage::remove_folder(env, folder_name, &owner);
    storage::remove_owner_records(env, &owner);
    storage::set_revocation(
        env,
        folder_name,
        &Revocation {
            admin: admin.clone(),
            reason_code,
            ledger: env.ledger().sequence(),
        },
    );
    events::folder_revoked(env, admin, folder_name, &owner, reason_code);
    Ok(())
}
