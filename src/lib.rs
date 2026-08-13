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
use crate::types::PortfolioNFT;
use crate::types::Template;

#[contract]
pub struct FolderContract;

#[contractimpl]
impl FolderContract {
    // ── Admin ─────────────────────────────────────────────────────────────────

    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        admin.require_auth();
        if storage::is_initialized(&env) {
            return Err(ContractError::AlreadyInitialized);
        }
        let mut admins = soroban_sdk::Vec::new(&env);
        admins.push_back(admin);
        storage::save_admins(&env, &admins);
        storage::set_initialized(&env);
        Ok(())
    }

    pub fn add_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), ContractError> {
        if !storage::is_initialized(&env) {
            return Err(ContractError::NotInitialized);
        }
        access_control::add_admin(&env, &admin, &new_admin)
    }

    pub fn remove_admin(env: Env, admin: Address, target: Address) -> Result<(), ContractError> {
        if !storage::is_initialized(&env) {
            return Err(ContractError::NotInitialized);
        }
        access_control::remove_admin(&env, &admin, &target)
    }

    // ── Identity ──────────────────────────────────────────────────────────────

    pub fn claim_folder(
        env: Env,
        owner: Address,
        folder_name: String,
    ) -> Result<(), ContractError> {
        if !storage::is_initialized(&env) {
            return Err(ContractError::NotInitialized);
        }
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
        if !storage::is_initialized(&env) {
            return Err(ContractError::NotInitialized);
        }
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
        if !storage::is_initialized(&env) {
            return Err(ContractError::NotInitialized);
        }
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
        if !storage::is_initialized(&env) {
            return Err(ContractError::NotInitialized);
        }
        nft_mint::mint_portfolio(&env, &owner, template_id, &ipfs_cid)
    }

    pub fn update_portfolio(
        env: Env,
        owner: Address,
        ipfs_cid: String,
    ) -> Result<(), ContractError> {
        if !storage::is_initialized(&env) {
            return Err(ContractError::NotInitialized);
        }
        nft_mint::update_portfolio(&env, &owner, &ipfs_cid)
    }

    pub fn get_portfolio(env: Env, owner: Address) -> Result<PortfolioNFT, ContractError> {
        nft_mint::get_portfolio(&env, &owner)
    }

    pub fn verify_portfolio(env: Env, owner: Address) -> Result<bool, ContractError> {
        nft_mint::verify_portfolio(&env, &owner)
    }
}
