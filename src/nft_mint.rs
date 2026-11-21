use soroban_sdk::{Address, Env, String};

use crate::access_control::require_owner;
use crate::errors::ContractError;
use crate::events;
use crate::identity_map;
use crate::storage;
use crate::types::PortfolioNFT;
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

    // Prevent double-mint (single read, also refreshes the entry's TTL)
    if storage::load_portfolio(env, owner).is_some() {
        return Err(ContractError::PortfolioAlreadyMinted);
    }

    validation::require_valid_template_id(template_id)?;
    validation::require_valid_ipfs_cid(ipfs_cid)?;
    storage::get_template(env, template_id).ok_or(ContractError::TemplateNotFound)?;

    let nft = PortfolioNFT {
        owner: owner.clone(),
        template_id,
        ipfs_cid: ipfs_cid.clone(),
        minted_at: env.ledger().timestamp(),
    };
    storage::save_portfolio(env, &nft);
    // Keep the user's template selection consistent with the minted portfolio.
    storage::set_user_template(env, owner, template_id);
    events::portfolio_minted(env, owner, template_id, ipfs_cid);
    Ok(())
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
    events::portfolio_updated(env, owner, ipfs_cid);
    Ok(())
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
