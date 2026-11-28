#![no_std]
// Contract code must fail with typed `ContractError`s, never panic (#238).
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects
    )
)]

mod access_control;
mod constants;
mod economics;
mod errors;
mod events;
mod identity_map;
mod nft_mint;
mod pricing;
mod recovery;
mod storage;
mod template_registry;
mod types;
mod validation;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, Address, BytesN, ContractExecutable, Env, String, Vec};

use crate::errors::ContractError;
use crate::types::ClaimFee;
use crate::types::Template;
use crate::types::{AdminOp, PortfolioNFT};

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

    pub fn add_admin(
        env: Env,
        admin: Address,
        new_admin: Address,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        access_control::add_admin(&env, &admin, &new_admin)
    }

    pub fn remove_admin(
        env: Env,
        admin: Address,
        target: Address,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        access_control::remove_admin(&env, &admin, &target)
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

    pub fn block_address(
        env: Env,
        admin: Address,
        addr: Address,
        reason_code: u32,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        access_control::require_admin(&env, &admin)?;
        if storage::is_admin(&env, &addr) {
            return Err(ContractError::CannotBlockAdmin);
        }
        storage::block_address(&env, &addr);
        events::address_blocked(&env, &admin, &addr, reason_code);
        Ok(())
    }

    pub fn unblock_address(
        env: Env,
        admin: Address,
        addr: Address,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        access_control::require_admin(&env, &admin)?;
        storage::unblock_address(&env, &addr);
        events::address_unblocked(&env, &admin, &addr);
        Ok(())
    }

    pub fn is_blocked(env: Env, addr: Address) -> bool {
        storage::is_blocked(&env, &addr)
    }

    pub fn set_portfolio_flag(
        env: Env,
        admin: Address,
        owner: Address,
        flag: u32,
        reason_code: u32,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        access_control::require_admin(&env, &admin)?;
        if flag > 2 {
            return Err(ContractError::InvalidPortfolioFlag);
        }
        storage::set_portfolio_flag(&env, &owner, flag);
        events::portfolio_flag_set(&env, &admin, &owner, flag, reason_code);
        Ok(())
    }

    pub fn get_portfolio_flag(env: Env, owner: Address) -> u32 {
        storage::get_portfolio_flag(&env, &owner)
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

    /// Admin-only, idempotent schema migration up to `CURRENT_STORAGE_VERSION`.
    /// Emits `migrated` with `[from, to]` only when the version actually moves.
    pub fn migrate(env: Env, admin: Address) -> Result<(), ContractError> {
        require_initialized(&env)?;
        access_control::require_admin(&env, &admin)?;
        let from = storage::get_storage_version(&env);
        let to = storage::run_migration_step(&env, from)?;
        if to != from {
            events::migrated(&env, from, to);
        }
        Ok(())
    }

    /// Returns the current storage schema version.
    pub fn get_storage_version(env: Env) -> u32 {
        storage::get_storage_version(&env)
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

    /// Admin-only removal of an abusive or impersonating name. The name is
    /// reserved, not reassigned. `reason_code`: 1 impersonation, 2 abuse,
    /// 3 legal.
    pub fn revoke_folder(
        env: Env,
        admin: Address,
        folder_name: String,
        reason_code: u32,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        identity_map::revoke_folder(&env, &admin, &folder_name, reason_code)
    }

    /// The revocation record of `folder_name`, if an admin revoked it.
    pub fn get_revocation(env: Env, folder_name: String) -> Option<Revocation> {
        storage::get_revocation(&env, &folder_name)
    }

    pub fn get_folder(env: Env, folder_name: String) -> Result<Address, ContractError> {
        identity_map::get_folder(&env, &folder_name)
    }

    pub fn get_identity(env: Env, owner: Address) -> Result<String, ContractError> {
        identity_map::get_identity(&env, &owner)
    }

    // ── Cross-contract resolver interface ─────────────────────────────────────

    /// Resolves a Folder name to its owner. Returns `None` for unknown names so
    /// that consumer contracts can branch without decoding an error.
    pub fn resolve(env: Env, folder_name: String) -> Option<Address> {
        storage::get_folder_owner(&env, &folder_name)
    }

    /// Reverse lookup: returns the Folder name owned by `owner`, or `None`.
    pub fn reverse(env: Env, owner: Address) -> Option<String> {
        storage::get_owner_folder(&env, &owner)
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

    pub fn submit_template(
        env: Env,
        designer: Address,
        metadata_uri: String,
    ) -> Result<u32, ContractError> {
        require_initialized(&env)?;
        template_registry::submit_template(&env, &designer, &metadata_uri)
    }

    pub fn approve_submission(
        env: Env,
        admin: Address,
        submission_id: u32,
        template_id: u32,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        template_registry::approve_submission(&env, &admin, submission_id, template_id)
    }

    pub fn reject_submission(
        env: Env,
        admin: Address,
        submission_id: u32,
        reason_code: u32,
    ) -> Result<(), ContractError> {
        require_initialized(&env)?;
        template_registry::reject_submission(&env, &admin, submission_id, reason_code)
    }

    pub fn get_template_designer(env: Env, template_id: u32) -> Option<Address> {
        storage::get_template_designer(&env, template_id)
    }

    pub fn get_template_usage(env: Env, template_id: u32) -> u32 {
        storage::get_template_usage(&env, template_id)
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

    // ── Config ────────────────────────────────────────────────────────────────

    /// Returns the client-facing validation rules and limits. Pure read: no
    /// state is written and no TTL is extended.
    pub fn get_limits(env: Env) -> Limits {
        Limits {
            min_folder_label_len: constants::MIN_FOLDER_LABEL_LEN,
            max_folder_name_len: constants::MAX_FOLDER_NAME_LEN,
            folder_suffix: String::from_bytes(&env, constants::FOLDER_SUFFIX),
            min_ipfs_cid_len: constants::MIN_IPFS_CID_LEN,
            max_ipfs_cid_len: constants::MAX_IPFS_CID_LEN,
            metadata_uri_scheme: String::from_bytes(&env, constants::METADATA_URI_SCHEME),
            max_metadata_uri_len: constants::MAX_METADATA_URI_LEN,
            max_templates: constants::MAX_TEMPLATES,
            max_admins: constants::MAX_ADMINS,
            day_in_ledgers: constants::DAY_IN_LEDGERS,
            persistent_bump_amount: constants::PERSISTENT_BUMP_AMOUNT,
            persistent_lifetime_threshold: constants::PERSISTENT_LIFETIME_THRESHOLD,
            instance_bump_amount: constants::INSTANCE_BUMP_AMOUNT,
            instance_lifetime_threshold: constants::INSTANCE_LIFETIME_THRESHOLD,
        }
    }

    // ── TTL maintenance ───────────────────────────────────────────────────────

    /// Extends the TTL of all persistent keys belonging to `owner`. Callable
    /// without auth; does not modify any data. Returns the number of keys
    /// extended (missing keys are skipped).
    pub fn extend_owner_ttl(env: Env, owner: Address) -> u32 {
        storage::extend_owner_ttl(&env, &owner)
    }

    /// Extends the TTL of the template entry and the template list. Callable
    /// without auth; does not modify any data. Returns the number of keys
    /// extended (missing keys are skipped).
    pub fn extend_template_ttl(env: Env, template_id: u32) -> u32 {
        storage::extend_template_ttl(&env, template_id)
    }
}
