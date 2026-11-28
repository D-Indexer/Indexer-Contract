use soroban_sdk::{Address, Env, String, Vec};

use crate::access_control::{require_admin, require_owner};
use crate::errors::ContractError;
use crate::events;
use crate::identity_map;
use crate::storage;
use crate::types::{CidEntry, PortfolioNFT};
use crate::validation;

/// Validated, side-effect-free description of a portfolio mint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MintPortfolioPlan {
    pub nft: PortfolioNFT,
}

/// Pure validation phase: checks auth and state, performs no writes.
pub fn plan_mint_portfolio(
    env: &Env,
    owner: &Address,
    template_id: u32,
    ipfs_cid: &String,
) -> Result<MintPortfolioPlan, ContractError> {
    require_owner(env, owner)?;
    if storage::is_blocked(env, owner) {
        return Err(ContractError::AddressBlocked);
    }

    // Must have claimed a Folder name before minting
    identity_map::get_identity(env, owner)?;

    check_mint(env, owner, template_id, ipfs_cid)?;
    apply_mint(env, owner, template_id, ipfs_cid);
    Ok(())
}

/// Claims `folder_name` and mints the owner's portfolio in one invocation.
/// Every rule of both steps is checked before the first write, so a failure
/// leaves storage untouched. Emits `folder_cl` then `port_mint`.
pub fn claim_and_mint(
    env: &Env,
    owner: &Address,
    folder_name: &String,
    template_id: u32,
    ipfs_cid: &String,
) -> Result<(), ContractError> {
    // One signature covers the claim fee transfer and both writes.
    require_owner(env, owner)?;

    let claim = identity_map::check_claim(env, owner, folder_name)?;
    check_mint(env, owner, template_id, ipfs_cid)?;

    identity_map::apply_claim(env, owner, owner, folder_name, claim);
    apply_mint(env, owner, template_id, ipfs_cid);
    Ok(())
}

/// Validation phase of a mint, minus the Folder ownership check (which
/// `claim_and_mint` satisfies in the same call). Does not write.
fn check_mint(
    env: &Env,
    owner: &Address,
    template_id: u32,
    ipfs_cid: &String,
) -> Result<(), ContractError> {
    // Prevent double-mint (single read, also refreshes the entry's TTL)
    if storage::load_portfolio(env, owner).is_some() {
        return Err(ContractError::PortfolioAlreadyMinted);
    }
    validation::require_valid_template_id(template_id)?;
    validation::require_valid_ipfs_cid(ipfs_cid)?;
    storage::get_template(env, template_id).ok_or(ContractError::TemplateNotFound)?;
    Ok(())
}

    Ok(MintPortfolioPlan {
        nft: PortfolioNFT {
            owner: owner.clone(),
            template_id,
            ipfs_cid: ipfs_cid.clone(),
            minted_at: env.ledger().timestamp(),
        },
    })
}

/// Write phase: applies a plan and emits its events.
pub fn apply_mint_portfolio(env: &Env, plan: &MintPortfolioPlan) {
    storage::save_portfolio(env, &plan.nft);
    // Keep the user's template selection consistent with the minted portfolio.
    if storage::get_user_template(env, owner).is_none() {
        let usage = storage::get_template_usage(env, template_id);
        storage::set_template_usage(env, template_id, usage.saturating_add(1));
    } else if let Some(old_id) = storage::get_user_template(env, owner) {
        if old_id != template_id {
            let old_usage = storage::get_template_usage(env, old_id);
            storage::set_template_usage(env, old_id, old_usage.saturating_sub(1));
            let new_usage = storage::get_template_usage(env, template_id);
            storage::set_template_usage(env, template_id, new_usage.saturating_add(1));
        }
    }
    storage::set_user_template(env, owner, template_id);
    let token_id = storage::assign_token_id(env, owner);
    events::portfolio_minted(env, owner, template_id, ipfs_cid, token_id);
}

pub fn mint_portfolio(
    env: &Env,
    owner: &Address,
    template_id: u32,
    ipfs_cid: &String,
) -> Result<(), ContractError> {
    let plan = plan_mint_portfolio(env, owner, template_id, ipfs_cid)?;
    apply_mint_portfolio(env, &plan);
    Ok(())
}

/// Validated, side-effect-free description of a portfolio update.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatePortfolioPlan {
    pub nft: PortfolioNFT,
    pub previous_cid: String,
}

/// Pure validation phase: checks auth and state, performs no writes.
pub fn plan_update_portfolio(
    env: &Env,
    owner: &Address,
    ipfs_cid: &String,
) -> Result<UpdatePortfolioPlan, ContractError> {
    require_owner(env, owner)?;
    if storage::is_blocked(env, owner) {
        return Err(ContractError::AddressBlocked);
    }

    validation::require_valid_ipfs_cid(ipfs_cid)?;
    let mut nft = storage::get_portfolio(env, owner)?;
    let previous_cid = nft.ipfs_cid.clone();
    nft.ipfs_cid = ipfs_cid.clone();
    Ok(UpdatePortfolioPlan { nft, previous_cid })
}

/// Write phase: applies a plan and emits its events.
pub fn apply_update_portfolio(env: &Env, plan: &UpdatePortfolioPlan) {
    storage::save_portfolio(env, &plan.nft);
    events::portfolio_updated(
        env,
        &plan.nft.owner,
        &plan.nft.ipfs_cid,
        &plan.previous_cid,
    );
}

pub fn update_portfolio(
    env: &Env,
    owner: &Address,
    ipfs_cid: &String,
) -> Result<(), ContractError> {
    let plan = plan_update_portfolio(env, owner, ipfs_cid)?;
    apply_update_portfolio(env, &plan);
    Ok(())
}

/// Token ID of the owner's portfolio.
pub fn token_of(env: &Env, owner: &Address) -> Result<u64, ContractError> {
    storage::get_owner_token(env, owner).ok_or(ContractError::TokenNotFound)
}

/// Owner of portfolio token `token_id`.
pub fn owner_of(env: &Env, token_id: u64) -> Result<Address, ContractError> {
    storage::get_token_owner(env, token_id).ok_or(ContractError::TokenNotFound)
}

/// Assigns a token ID to a portfolio minted before token IDs existed.
/// Admin only and idempotent: an owner that already has an ID gets the same
/// ID back and nothing is written.
pub fn backfill_token_id(
    env: &Env,
    admin: &Address,
    owner: &Address,
) -> Result<u64, ContractError> {
    require_admin(env, admin)?;
    storage::get_portfolio(env, owner)?;
    if let Some(token_id) = storage::get_owner_token(env, owner) {
        return Ok(token_id);
    }
    let token_id = storage::assign_token_id(env, owner);
    events::token_id_backfilled(env, admin, owner, token_id);
    Ok(token_id)
}

/// The owner's last `MAX_CID_HISTORY` portfolio CIDs, oldest first.
pub fn get_portfolio_history(env: &Env, owner: &Address) -> Vec<CidEntry> {
    storage::get_portfolio_history(env, owner)
}

pub fn get_portfolio(env: &Env, owner: &Address) -> Result<PortfolioNFT, ContractError> {
    storage::get_portfolio(env, owner)
}

/// Resolves a Folder name to its owner and returns that owner's portfolio.
/// Equivalent to `get_folder` followed by `get_portfolio`.
pub fn get_portfolio_by_folder(
    env: &Env,
    folder_name: &String,
) -> Result<PortfolioNFT, ContractError> {
    validation::require_valid_folder_name(folder_name)?;
    let owner = storage::get_folder_owner(env, folder_name).ok_or(ContractError::FolderNotFound)?;
    storage::get_portfolio(env, &owner)
}

/// Returns `true` when the Folder name is valid, claimed, and has a portfolio.
pub fn verify_folder(env: &Env, folder_name: &String) -> Result<bool, ContractError> {
    match get_portfolio_by_folder(env, folder_name) {
        Ok(_) => Ok(true),
        Err(ContractError::PortfolioNotFound) => Ok(false),
        Err(err) => Err(err),
    }
}

pub fn verify_portfolio(env: &Env, owner: &Address) -> Result<bool, ContractError> {
    match storage::get_portfolio(env, owner) {
        Ok(_) => Ok(true),
        Err(ContractError::PortfolioNotFound) => Ok(false),
        Err(err) => Err(err),
    }
}

/// Whether `ipfs_cid` is exactly (byte for byte) the owner's current on-chain
/// portfolio CID. A malformed CID is an error so clients can tell bad input
/// from a mismatch; a missing portfolio is simply `false`.
pub fn verify_portfolio_cid(
    env: &Env,
    owner: &Address,
    ipfs_cid: &String,
) -> Result<bool, ContractError> {
    validation::require_valid_ipfs_cid(ipfs_cid)?;
    Ok(storage::load_portfolio(env, owner).is_some_and(|nft| &nft.ipfs_cid == ipfs_cid))
}

/// `verify_portfolio_cid` for the current owner of `folder_name`. An unclaimed,
/// expired or released name is `false`.
pub fn verify_folder_portfolio(
    env: &Env,
    folder_name: &String,
    ipfs_cid: &String,
) -> Result<bool, ContractError> {
    validation::require_valid_ipfs_cid(ipfs_cid)?;
    validation::require_valid_folder_name(folder_name)?;
    match identity_map::get_folder(env, folder_name) {
        Ok(owner) => verify_portfolio_cid(env, &owner, ipfs_cid),
        Err(ContractError::FolderNotFound | ContractError::FolderExpired) => Ok(false),
        Err(err) => Err(err),
    }
}
