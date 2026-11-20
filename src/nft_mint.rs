use soroban_sdk::{Address, Env, String, Vec};

use crate::access_control::require_owner;
use crate::constants::{DEFAULT_CONTENT_SCHEMA, MAX_PORTFOLIOS_PER_OWNER};
use crate::errors::ContractError;
use crate::events;
use crate::storage;
use crate::template_registry::{require_schema_supported, require_template_allowed};
use crate::types::PortfolioNFT;
use crate::validation;

pub fn mint_portfolio(
    env: &Env,
    owner: &Address,
    template_id: u32,
    ipfs_cid: &String,
) -> Result<(), ContractError> {
    mint(env, owner, 0, template_id, ipfs_cid, DEFAULT_CONTENT_SCHEMA)
}

pub fn mint_portfolio_v2(
    env: &Env,
    owner: &Address,
    template_id: u32,
    ipfs_cid: &String,
    content_schema: u32,
) -> Result<(), ContractError> {
    mint(env, owner, 0, template_id, ipfs_cid, content_schema)
}

pub fn mint_portfolio_slot(
    env: &Env,
    owner: &Address,
    slot: u32,
    template_id: u32,
    ipfs_cid: &String,
) -> Result<(), ContractError> {
    mint(
        env,
        owner,
        slot,
        template_id,
        ipfs_cid,
        DEFAULT_CONTENT_SCHEMA,
    )
}

fn mint(
    env: &Env,
    owner: &Address,
    slot: u32,
    template_id: u32,
    ipfs_cid: &String,
    content_schema: u32,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;

    // Must have claimed a Folder name before minting
    storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)?;

    // Prevent double-mint
    if storage::portfolio_exists_at(env, owner, slot) {
        return Err(ContractError::PortfolioAlreadyMinted);
    }
    // Slot 0 is always available, so extra slots get one less than the cap.
    let mut extra_slots = storage::get_extra_slots(env, owner);
    if slot != 0 && extra_slots.len() >= MAX_PORTFOLIOS_PER_OWNER - 1 {
        return Err(ContractError::PortfolioLimitReached);
    }

    validation::require_valid_template_id(template_id)?;
    validation::require_valid_ipfs_cid(ipfs_cid)?;
    storage::get_template(env, template_id).ok_or(ContractError::TemplateNotFound)?;
    require_template_allowed(env, template_id, owner)?;
    require_schema_supported(env, template_id, content_schema)?;

    let nft = PortfolioNFT {
        owner: owner.clone(),
        template_id,
        ipfs_cid: ipfs_cid.clone(),
        minted_at: env.ledger().timestamp(),
    };
    storage::save_portfolio_at(env, slot, &nft);
    if slot == 0 {
        // Keep the user's template selection consistent with the minted portfolio.
        storage::set_user_template(env, owner, template_id);
        storage::set_portfolio_schema(env, owner, content_schema);
    } else {
        extra_slots.push_back(slot);
        storage::save_extra_slots(env, owner, &extra_slots);
    }
    events::portfolio_minted(env, owner, template_id, ipfs_cid, slot);
    Ok(())
}

pub fn update_portfolio(
    env: &Env,
    owner: &Address,
    ipfs_cid: &String,
) -> Result<(), ContractError> {
    update(env, owner, 0, ipfs_cid, DEFAULT_CONTENT_SCHEMA)
}

pub fn update_portfolio_v2(
    env: &Env,
    owner: &Address,
    ipfs_cid: &String,
    content_schema: u32,
) -> Result<(), ContractError> {
    update(env, owner, 0, ipfs_cid, content_schema)
}

pub fn update_portfolio_slot(
    env: &Env,
    owner: &Address,
    slot: u32,
    ipfs_cid: &String,
) -> Result<(), ContractError> {
    update(env, owner, slot, ipfs_cid, DEFAULT_CONTENT_SCHEMA)
}

fn update(
    env: &Env,
    owner: &Address,
    slot: u32,
    ipfs_cid: &String,
    content_schema: u32,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;

    validation::require_valid_ipfs_cid(ipfs_cid)?;
    let mut nft = storage::get_portfolio_at(env, owner, slot)?;
    require_schema_supported(env, nft.template_id, content_schema)?;
    nft.ipfs_cid = ipfs_cid.clone();
    storage::save_portfolio_at(env, slot, &nft);
    if slot == 0 {
        storage::set_portfolio_schema(env, owner, content_schema);
    }
    events::portfolio_updated(env, owner, ipfs_cid, slot);
    Ok(())
}

pub fn get_portfolio(env: &Env, owner: &Address) -> Result<PortfolioNFT, ContractError> {
    storage::get_portfolio(env, owner)
}

pub fn get_portfolio_slot(
    env: &Env,
    owner: &Address,
    slot: u32,
) -> Result<PortfolioNFT, ContractError> {
    storage::get_portfolio_at(env, owner, slot)
}

/// Minted slots of `owner`: 0 first when the legacy portfolio exists.
pub fn list_portfolio_slots(env: &Env, owner: &Address) -> Vec<u32> {
    let mut slots = Vec::new(env);
    if storage::portfolio_exists(env, owner) {
        slots.push_back(0);
    }
    slots.append(&storage::get_extra_slots(env, owner));
    slots
}

pub fn verify_portfolio(env: &Env, owner: &Address) -> Result<bool, ContractError> {
    match storage::get_portfolio(env, owner) {
        Ok(_) => Ok(true),
        Err(ContractError::PortfolioNotFound) => Ok(false),
        Err(err) => Err(err),
    }
}
