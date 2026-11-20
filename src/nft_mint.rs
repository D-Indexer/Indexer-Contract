use soroban_sdk::{Address, Env, String};

use crate::access_control::{require_admin, require_owner};
use crate::constants::MAX_UPDATE_INTERVAL_LEDGERS;
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
    // Keep the user's template selection consistent with the minted portfolio.
    storage::set_user_template(env, owner, template_id);
    // Mint isn't rate-limited, but it starts the owner's update interval.
    storage::set_last_update(env, owner, storage::get_min_update_interval(env));
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
    let interval = storage::get_min_update_interval(env);
    if interval > 0 {
        if let Some(last) = storage::get_last_update(env, owner) {
            if env.ledger().sequence() < last.saturating_add(interval) {
                return Err(ContractError::UpdateTooSoon);
            }
        }
    }
    nft.ipfs_cid = ipfs_cid.clone();
    storage::save_portfolio(env, &nft);
    storage::set_last_update(env, owner, interval);
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

pub fn set_min_update_interval(
    env: &Env,
    admin: &Address,
    ledgers: u32,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    if ledgers > MAX_UPDATE_INTERVAL_LEDGERS {
        return Err(ContractError::InvalidUpdateInterval);
    }
    storage::set_min_update_interval(env, ledgers);
    events::update_interval_set(env, admin, ledgers);
    Ok(())
}
