#![no_std]

mod access_control;
mod constants;
mod economics;
mod errors;
mod events;
mod identity_map;
mod nft_mint;
mod pricing;
mod storage;
mod template_registry;
mod types;
mod validation;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String, Vec};

use crate::errors::ContractError;
use crate::types::ClaimFee;
use crate::types::PortfolioNFT;
use crate::types::Template;
use crate::types::{LeaseConfig, PortfolioNFT, PriceTiers};

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

    // ── Economics ─────────────────────────────────────────────────────────────

    /// Sets the Folder claim fee. An `amount` of 0 disables the fee.
    pub fn set_claim_fee(
        env: Env,
        admin: Address,
        token: Address,
        amount: i128,
        treasury: Address,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        economics::set_claim_fee(&env, &admin, &token, amount, &treasury)
    }

    /// Returns the current claim fee configuration, if any.
    pub fn get_claim_fee(env: Env) -> Option<ClaimFee> {
        storage::get_claim_fee(&env)
    }

    /// Withdraws `amount` of `token` held by this contract to `to`, capped at
    /// `MAX_WITHDRAW_PER_CALL` per call.
    pub fn withdraw(
        env: Env,
        admin: Address,
        token: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        economics::withdraw(&env, &admin, &token, &to, amount)
    }

    /// Returns this contract's balance of `token`.
    pub fn treasury_balance(env: Env, token: Address) -> i128 {
        economics::treasury_balance(&env, &token)
    }

    /// Replaces the contract code with `new_wasm_hash`. Admin only. Storage is
    /// untouched: instance and persistent entries survive an upgrade.
    pub fn upgrade(
        env: Env,
        admin: Address,
        new_wasm_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        access_control::require_admin(&env, &admin)?;
        env.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());
        events::contract_upgraded(&env, &admin, &new_wasm_hash);
        Ok(())
    }

    /// Returns the contract version (`CONTRACT_VERSION`).
    pub fn version(_env: Env) -> u32 {
        constants::CONTRACT_VERSION
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

    /// Claims `folder_name` for `owner` with the fee paid by `sponsor`.
    /// Both must authorize.
    pub fn sponsored_claim(
        env: Env,
        sponsor: Address,
        owner: Address,
        folder_name: String,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        identity_map::sponsored_claim(&env, &sponsor, &owner, &folder_name)
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

    pub fn register_template_with_hash(
        env: Env,
        admin: Address,
        template_id: u32,
        metadata_uri: String,
        content_sha256: BytesN<32>,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        template_registry::register_template_with_hash(
            &env,
            &admin,
            template_id,
            &metadata_uri,
            &content_sha256,
        )
    }

    pub fn get_template(env: Env, template_id: u32) -> Result<Template, ContractError> {
        template_registry::get_template(&env, template_id)
    }

    pub fn get_template_hash(env: Env, template_id: u32) -> Option<BytesN<32>> {
        storage::get_template_hash(&env, template_id)
    }

    pub fn list_templates(env: Env) -> Vec<u32> {
        template_registry::list_templates(&env)
    }

    /// Returns up to 25 templates starting at the given index in registration order.
    pub fn list_templates_page(env: Env, start: u32, limit: u32) -> Vec<Template> {
        template_registry::list_templates_page(&env, start, limit)
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
