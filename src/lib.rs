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

use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};

use crate::errors::ContractError;
use crate::types::AdminOp;
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

    /// Queues an admin addition; executable after `ADMIN_TIMELOCK_LEDGERS`.
    pub fn queue_admin_change(
        env: Env,
        admin: Address,
        op: AdminOp,
        target: Address,
    ) -> Result<u32, ContractError> {
        require_initialized(&env)?;
        access_control::queue_admin_change(&env, &admin, op, &target)
    }

    /// Executes a queued admin change once its ETA ledger has passed.
    pub fn execute_admin_change(
        env: Env,
        admin: Address,
        op_id: u32,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        access_control::execute_admin_change(&env, &admin, op_id)
    }

    /// Cancels a queued admin change before it is executed.
    pub fn cancel_admin_change(
        env: Env,
        admin: Address,
        op_id: u32,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        access_control::cancel_admin_change(&env, &admin, op_id)
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
}
