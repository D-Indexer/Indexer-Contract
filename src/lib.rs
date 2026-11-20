#![no_std]

mod access_control;
mod constants;
mod errors;
mod events;
mod identity_map;
mod nft_mint;
mod storage;
mod template_registry;
mod types;
mod validation;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String, Vec};

use crate::errors::ContractError;
use crate::types::PortfolioNFT;
use crate::types::Template;

#[contract]
pub struct FolderContract;

/// Guards state-changing entry points: fails before initialization and keeps
/// the contract instance alive afterwards.
fn require_initialized(env: &Env) -> Result<(), ContractError> {
    if !storage::is_initialized(env) {
        return Err(ContractError::NotInitialized);
    }
    storage::extend_instance(env);
    Ok(())
}

#[contractimpl]
impl FolderContract {
    // ── Admin ─────────────────────────────────────────────────────────────────

    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        admin.require_auth();
        if storage::is_initialized(&env) {
            return Err(ContractError::AlreadyInitialized);
        }
        let mut admins = soroban_sdk::Vec::new(&env);
        admins.push_back(admin.clone());
        storage::save_admins(&env, &admins);
        storage::set_initialized(&env);
        storage::extend_instance(&env);
        events::contract_initialized(&env, &admin);
        Ok(())
    }

    /// Returns the current admin list (empty before initialization).
    pub fn get_admins(env: Env) -> Vec<Address> {
        storage::get_admins(&env)
    }

    pub fn add_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), ContractError> {
        require_initialized(&env)?;
        access_control::add_admin(&env, &admin, &new_admin)
    }

    pub fn remove_admin(env: Env, admin: Address, target: Address) -> Result<(), ContractError> {
        require_initialized(&env)?;
        access_control::remove_admin(&env, &admin, &target)
    }

    // ── Events ────────────────────────────────────────────────────────────────

    /// Returns the last event sequence number and the rolling event digest.
    pub fn get_event_cursor(env: Env) -> (u64, BytesN<32>) {
        (
            storage::get_event_seq(&env),
            storage::get_event_digest(&env),
        )
    }

    // ── Identity ──────────────────────────────────────────────────────────────

    pub fn claim_folder(
        env: Env,
        owner: Address,
        folder_name: String,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        identity_map::claim_folder(&env, &owner, &folder_name)
    }

    pub fn get_folder(env: Env, folder_name: String) -> Result<Address, ContractError> {
        identity_map::get_folder(&env, &folder_name)
    }

    pub fn get_identity(env: Env, owner: Address) -> Result<String, ContractError> {
        identity_map::get_identity(&env, &owner)
    }

    // ── Template Registry ─────────────────────────────────────────────────────

    pub fn register_template(
        env: Env,
        admin: Address,
        template_id: u32,
        metadata_uri: String,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        template_registry::register_template(&env, &admin, template_id, &metadata_uri)
    }

    pub fn get_template(env: Env, template_id: u32) -> Result<Template, ContractError> {
        template_registry::get_template(&env, template_id)
    }

    pub fn list_templates(env: Env) -> Vec<u32> {
        template_registry::list_templates(&env)
    }

    pub fn set_user_template(
        env: Env,
        owner: Address,
        template_id: u32,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        template_registry::set_user_template(&env, &owner, template_id)
    }

    /// Returns the template ID currently assigned to the owner's Folder.
    pub fn get_user_template(env: Env, owner: Address) -> Option<u32> {
        storage::get_user_template(&env, &owner)
    }

    /// Sets the content schemas a template supports (1 to 8 non-zero values).
    pub fn set_template_schemas(
        env: Env,
        admin: Address,
        template_id: u32,
        schemas: Vec<u32>,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        template_registry::set_template_schemas(&env, &admin, template_id, &schemas)
    }

    /// Returns the content schemas a template supports (`[1]` by default).
    pub fn get_template_schemas(env: Env, template_id: u32) -> Vec<u32> {
        storage::get_template_schemas(&env, template_id)
    }

    pub fn set_template_gated(
        env: Env,
        admin: Address,
        template_id: u32,
        gated: bool,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        template_registry::set_template_gated(&env, &admin, template_id, gated)
    }

    pub fn allow_template_owner(
        env: Env,
        admin: Address,
        template_id: u32,
        owner: Address,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        template_registry::set_template_allowed(&env, &admin, template_id, &owner, true)
    }

    /// Removing an owner doesn't affect a selection or portfolio they already have.
    pub fn revoke_template_owner(
        env: Env,
        admin: Address,
        template_id: u32,
        owner: Address,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        template_registry::set_template_allowed(&env, &admin, template_id, &owner, false)
    }

    pub fn can_use_template(env: Env, template_id: u32, owner: Address) -> bool {
        template_registry::can_use_template(&env, template_id, &owner)
    }

    // ── Portfolio NFT ─────────────────────────────────────────────────────────

    pub fn mint_portfolio(
        env: Env,
        owner: Address,
        template_id: u32,
        ipfs_cid: String,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        nft_mint::mint_portfolio(&env, &owner, template_id, &ipfs_cid)
    }

    pub fn update_portfolio(
        env: Env,
        owner: Address,
        ipfs_cid: String,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        nft_mint::update_portfolio(&env, &owner, &ipfs_cid)
    }

    pub fn get_portfolio(env: Env, owner: Address) -> Result<PortfolioNFT, ContractError> {
        nft_mint::get_portfolio(&env, &owner)
    }

    pub fn verify_portfolio(env: Env, owner: Address) -> Result<bool, ContractError> {
        nft_mint::verify_portfolio(&env, &owner)
    }

    pub fn mint_portfolio_v2(
        env: Env,
        owner: Address,
        template_id: u32,
        ipfs_cid: String,
        content_schema: u32,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        nft_mint::mint_portfolio_v2(&env, &owner, template_id, &ipfs_cid, content_schema)
    }

    pub fn update_portfolio_v2(
        env: Env,
        owner: Address,
        ipfs_cid: String,
        content_schema: u32,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        nft_mint::update_portfolio_v2(&env, &owner, &ipfs_cid, content_schema)
    }

    /// Returns the content schema of the owner's slot-0 portfolio (1 by default).
    pub fn get_portfolio_schema(env: Env, owner: Address) -> u32 {
        storage::get_portfolio_schema(&env, &owner)
    }

    pub fn mint_portfolio_slot(
        env: Env,
        owner: Address,
        slot: u32,
        template_id: u32,
        ipfs_cid: String,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        nft_mint::mint_portfolio_slot(&env, &owner, slot, template_id, &ipfs_cid)
    }

    pub fn update_portfolio_slot(
        env: Env,
        owner: Address,
        slot: u32,
        ipfs_cid: String,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        nft_mint::update_portfolio_slot(&env, &owner, slot, &ipfs_cid)
    }

    pub fn get_portfolio_slot(
        env: Env,
        owner: Address,
        slot: u32,
    ) -> Result<PortfolioNFT, ContractError> {
        nft_mint::get_portfolio_slot(&env, &owner, slot)
    }

    pub fn list_portfolio_slots(env: Env, owner: Address) -> Vec<u32> {
        nft_mint::list_portfolio_slots(&env, &owner)
    }
}
