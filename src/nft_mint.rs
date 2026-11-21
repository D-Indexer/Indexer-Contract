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
