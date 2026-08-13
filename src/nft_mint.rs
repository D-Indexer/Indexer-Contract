use soroban_sdk::{Address, Env, String};

use crate::access_control::require_owner;
use crate::errors::ContractError;
use crate::events;
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
    storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)?;

    // Prevent double-mint
    if storage::portfolio_exists(env, owner) {
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
