use soroban_sdk::{Address, Env, String, Vec};

use crate::access_control::{require_admin, require_owner};
use crate::errors::ContractError;
use crate::events;
use crate::identity_map;
use crate::storage;
use crate::types::{CidEntry, PortfolioNFT};
use crate::validation;

pub fn mint_portfolio(
    env: &Env,
    owner: &Address,
    template_id: u32,
    ipfs_cid: &String,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;

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

/// Write phase of a mint: stores the portfolio, syncs the template
/// selection, assigns the next token ID and emits `port_mint`. Only call it after `check_mint`.
fn apply_mint(env: &Env, owner: &Address, template_id: u32, ipfs_cid: &String) {
    let nft = PortfolioNFT {
        owner: owner.clone(),
        template_id,
        ipfs_cid: ipfs_cid.clone(),
        minted_at: env.ledger().timestamp(),
    };
    storage::save_portfolio(env, &nft);
    storage::append_portfolio_history(env, owner, ipfs_cid);
    // Keep the user's template selection consistent with the minted portfolio.
    storage::set_user_template(env, owner, template_id);
    let token_id = storage::assign_token_id(env, owner);
    events::portfolio_minted(env, owner, template_id, ipfs_cid, token_id);
}

pub fn update_portfolio(
    env: &Env,
    owner: &Address,
    ipfs_cid: &String,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;

    validation::require_valid_ipfs_cid(ipfs_cid)?;
    let mut nft = storage::get_portfolio(env, owner)?;
    nft.ipfs_cid = ipfs_cid.clone();
    storage::save_portfolio(env, &nft);
    storage::append_portfolio_history(env, owner, ipfs_cid);
    events::portfolio_updated(env, owner, ipfs_cid);
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

pub fn verify_portfolio(env: &Env, owner: &Address) -> Result<bool, ContractError> {
    match storage::get_portfolio(env, owner) {
        Ok(_) => Ok(true),
        Err(ContractError::PortfolioNotFound) => Ok(false),
        Err(err) => Err(err),
    }
}
