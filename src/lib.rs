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

use soroban_sdk::{contract, contractimpl, Address, BytesN, ContractExecutable, Env, String, Vec};

use crate::errors::ContractError;
use crate::types::{AdminOp, ClaimFee, LeaseConfig, PortfolioNFT, PriceTiers, Template};

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
    pub fn execute_admin_change(env: Env, admin: Address, op_id: u32) -> Result<(), ContractError> {
        require_initialized(&env)?;
        access_control::execute_admin_change(&env, &admin, op_id)
    }

    /// Cancels a queued admin change before it is executed.
    pub fn cancel_admin_change(env: Env, admin: Address, op_id: u32) -> Result<(), ContractError> {
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

    /// Replaces the length-based premium tiers (at most 5, sorted, validated).
    /// Labels longer than every tier pay the `ClaimFee` amount.
    pub fn set_price_tiers(
        env: Env,
        admin: Address,
        tiers: PriceTiers,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        pricing::set_price_tiers(&env, &admin, &tiers)
    }

    pub fn get_price_tiers(env: Env) -> PriceTiers {
        storage::get_price_tiers(&env)
    }

    /// Fee charged to claim `folder_name` (or to renew it for one period).
    pub fn quote_claim(env: Env, folder_name: String) -> Result<i128, ContractError> {
        pricing::quote_claim(&env, &folder_name)
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
            .update_current_contract(ContractExecutable::Wasm(new_wasm_hash.clone()));
        events::contract_upgraded(&env, &admin, &new_wasm_hash);
        Ok(())
    }

    /// Returns the contract version (`CONTRACT_VERSION`).
    pub fn version(_env: Env) -> u32 {
        constants::CONTRACT_VERSION
    }

    // ── Leases ────────────────────────────────────────────────────────────────

    /// Enables leases of `period_days` with a `grace_days` grace period for
    /// new claims. `period_days == 0` restores permanent mode (the default).
    pub fn set_lease_config(
        env: Env,
        admin: Address,
        period_days: u32,
        grace_days: u32,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        access_control::require_admin(&env, &admin)?;
        let config = (period_days > 0).then(|| LeaseConfig {
            period_secs: period_days as u64 * constants::SECONDS_PER_DAY,
            grace_secs: grace_days as u64 * constants::SECONDS_PER_DAY,
        });
        storage::set_lease_config(&env, &config);
        Ok(())
    }

    pub fn get_lease_config(env: Env) -> Option<LeaseConfig> {
        storage::get_lease_config(&env)
    }

    /// Extends the owner's lease by `periods`; returns the new expiry.
    pub fn renew_folder(env: Env, owner: Address, periods: u32) -> Result<u64, ContractError> {
        require_initialized(&env)?;
        identity_map::renew_folder(&env, &owner, periods)
    }

    /// Lease expiry timestamp, or `None` for a permanent / unknown name.
    pub fn get_folder_expiry(env: Env, folder_name: String) -> Option<u64> {
        identity_map::get_folder_expiry(&env, &folder_name)
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

    /// Phase 1 of a front-running-resistant claim. `commitment` is
    /// `sha256(folder_name_bytes || salt || owner_xdr)`; see `claim_commitment`.
    pub fn commit_claim(
        env: Env,
        owner: Address,
        commitment: BytesN<32>,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        identity_map::commit_claim(&env, &owner, &commitment)
    }

    /// Phase 2: reveals the committed name at least `MIN_REVEAL_DELAY` and at
    /// most `REVEAL_WINDOW` ledgers after `commit_claim`, then claims it.
    pub fn reveal_claim(
        env: Env,
        owner: Address,
        folder_name: String,
        salt: BytesN<32>,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        identity_map::reveal_claim(&env, &owner, &folder_name, &salt)
    }

    /// Computes the commitment `commit_claim` expects. Clients should compute
    /// it locally: simulating this call shows the name to the RPC node.
    pub fn claim_commitment(
        env: Env,
        owner: Address,
        folder_name: String,
        salt: BytesN<32>,
    ) -> BytesN<32> {
        identity_map::claim_commitment(&env, &owner, &folder_name, &salt)
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
