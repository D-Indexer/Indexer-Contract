use soroban_sdk::{Address, Env, String, Vec};

use crate::constants::{
    CLAIM_COMMIT_TTL, INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT,
    PERSISTENT_LIFETIME_THRESHOLD,
};
use crate::errors::ContractError;
use crate::types::{
    AdminChange, ClaimFee, DataKey, FeeConfig, LeaseConfig, PortfolioNFT, PriceTiers, Template,
    TemplateSubmission,
};

// ── Initialization ────────────────────────────────────────────────────────────

pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Initialized)
}

pub fn set_initialized(env: &Env) {
    env.storage().instance().set(&DataKey::Initialized, &true);
}

/// Extends the contract instance and code TTL. Instance storage (admins,
/// initialization flag) lives and dies with the instance entry.
pub fn extend_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

use crate::constants::{
    HARD_MAX_FOLDER_NAME_LEN, HARD_MAX_METADATA_URI_LEN, HARD_MAX_TEMPLATES, MAX_FOLDER_NAME_LEN,
    MAX_METADATA_URI_LEN, MAX_TEMPLATES,
};
use crate::errors::ContractError;
use crate::types::{Config, DataKey};

/// Returns the active config, falling back to the compile-time defaults when
/// no config has been stored yet.
pub fn get_config(env: &Env) -> Config {
    env.storage()
        .instance()
        .get(&DataKey::Config)
        .unwrap_or_else(default_config)
}

/// Compile-time defaults for the tunable limits.
pub fn default_config() -> Config {
    Config {
        max_folder_name_len: MAX_FOLDER_NAME_LEN,
        max_metadata_uri_len: MAX_METADATA_URI_LEN,
        max_templates: MAX_TEMPLATES,
    }
}

/// Validates `config` against the hard ceilings and the current state, then
/// stores it. Rejects values that would invalidate existing data.
pub fn set_config(env: &Env, config: &Config) -> Result<(), ContractError> {
    if config.max_folder_name_len > HARD_MAX_FOLDER_NAME_LEN
        || config.max_metadata_uri_len > HARD_MAX_METADATA_URI_LEN
        || config.max_templates > HARD_MAX_TEMPLATES
    {
        return Err(ContractError::InvalidConfig);
    }
    if config.max_templates < get_template_list(env).len() {
        return Err(ContractError::InvalidConfig);
    }
    env.storage().instance().set(&DataKey::Config, config);
    Ok(())
}

// ── Admin ─────────────────────────────────────────────────────────────────────

/// Returns the admin list. Membership is stored per admin under
/// `Admin(Address) -> slot` with the reverse `AdminSlot(slot) -> Address`, so
/// the list can be enumerated; the legacy `AdminList` vector is migrated on
/// first read.
pub fn get_admins(env: &Env) -> Vec<Address> {
    let count = admin_count(env);
    let mut admins = Vec::new(env);
    let mut i = 0u32;
    while i < count {
        if let Some(admin) = env.storage().instance().get(&DataKey::AdminIndex(i)) {
            admins.push_back(admin);
        }
    }
    admins
}

/// Number of admins, migrating the legacy `AdminList` first if needed.
pub fn admin_count(env: &Env) -> u32 {
    migrate_admin_list(env);
    env.storage()
        .instance()
        .get(&DataKey::AdminCount)
        .unwrap_or(0)
}

/// Constant-time membership check: one instance read, no vector scan.
pub fn is_admin(env: &Env, address: &Address) -> bool {
    migrate_admin_list(env);
    env.storage()
        .instance()
        .has(&DataKey::Admin(address.clone()))
}

/// Adds `address` as an admin. Returns `false` if it already was one.
pub fn add_admin_entry(env: &Env, address: &Address) -> bool {
    if is_admin(env, address) {
        return false;
    }
    let count = admin_count(env);
    env.storage()
        .instance()
        .set(&DataKey::Admin(address.clone()), &());
    env.storage()
        .instance()
        .set(&DataKey::AdminIndex(count), address);
    env.storage()
        .instance()
        .set(&DataKey::AdminCount, &(count + 1));
    true
}

/// Removes `address` from the admins. Returns `false` if it was not one.
/// The last slot is moved into the freed one so `AdminAt` stays dense.
pub fn remove_admin_entry(env: &Env, address: &Address) -> bool {
    if !is_admin(env, address) {
        return false;
    }
    let count = admin_count(env);
    let instance = env.storage().instance();
    let slot: u32 = instance.get(&DataKey::Admin(address.clone())).unwrap_or(0);
    let last = count - 1;
    if slot != last {
        let moved: Address = instance.get(&DataKey::AdminAt(last)).unwrap();
        instance.set(&DataKey::AdminAt(slot), &moved);
        instance.set(&DataKey::Admin(moved), &slot);
    }
    instance.remove(&DataKey::AdminAt(last));
    instance.remove(&DataKey::Admin(address.clone()));
    instance.set(&DataKey::AdminCount, &last);
    true
}

/// Replaces the whole admin set (used by `initialize`).
pub fn save_admins(env: &Env, admins: &Vec<Address>) {
    let count = admin_count(env);
    let mut i = 0u32;
    while i < count {
        if let Some(admin) = env.storage().instance().get::<_, Address>(&DataKey::AdminIndex(i)) {
            env.storage().instance().remove(&DataKey::Admin(admin));
        }
        env.storage().instance().remove(&DataKey::AdminIndex(i));
        i += 1;
    }
    for admin in admins.iter() {
        add_admin_entry(env, &admin);
    }
}

/// Idempotently converts a legacy `AdminList` vector into per-admin keys.
/// Does nothing once the legacy entry is gone.
fn migrate_admin_list(env: &Env) {
    let legacy: Option<Vec<Address>> = env.storage().instance().get(&DataKey::AdminList);
    if let Some(admins) = legacy {
        // Remove first: `add_admin_entry` re-enters this function.
        env.storage().instance().remove(&DataKey::AdminList);
        for admin in admins.iter() {
            add_admin_entry(env, &admin);
        }
    }
}

// ── Admin Timelock ────────────────────────────────────────────────────────────

/// Pending admin changes live in instance storage so they are loaded with the
/// admin list; the queue is bounded by `MAX_PENDING_ADMIN_CHANGES`.
pub fn get_pending_admin_changes(env: &Env) -> Vec<(u32, AdminChange)> {
    env.storage()
        .instance()
        .get(&DataKey::PendingAdminChanges)
        .unwrap_or(Vec::new(env))
}

pub fn save_pending_admin_changes(env: &Env, pending: &Vec<(u32, AdminChange)>) {
    env.storage()
        .instance()
        .set(&DataKey::PendingAdminChanges, pending);
}

pub fn next_admin_change_id(env: &Env) -> u32 {
    let id: u32 = env
        .storage()
        .instance()
        .get(&DataKey::NextAdminChangeId)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::NextAdminChangeId, &(id + 1));
    id
}

// ── Identity Map ──────────────────────────────────────────────────────────────

/// Hashed key for a Folder name. `sha256` output is a fixed 32 bytes, so the
/// ledger key is the same size for every name length (3..=64 bytes).
pub fn folder_owner_key(env: &Env, folder_name: &String) -> DataKey {
    DataKey::FolderOwnerHash(env.crypto().sha256(folder_name).to_bytes())
}

/// Reads the owner of a Folder. Falls back to the legacy `FolderOwner(String)`
/// layout and lazily migrates the entry to the hashed layout on first read.
pub fn get_folder_owner(env: &Env, folder_name: &String) -> Option<Address> {
    let key = folder_owner_key(env, folder_name);
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        env.storage().persistent().extend_ttl(
            &key,
            PERSISTENT_LIFETIME_THRESHOLD,
            PERSISTENT_BUMP_AMOUNT,
        );
        return result;
    }

    let legacy_key = DataKey::FolderOwner(folder_name.clone());
    let legacy: Option<Address> = env.storage().persistent().get(&legacy_key);
    if let Some(owner) = legacy {
        env.storage().persistent().set(&key, &owner);
        env.storage().persistent().extend_ttl(
            &key,
            PERSISTENT_LIFETIME_THRESHOLD,
            PERSISTENT_BUMP_AMOUNT,
        );
        env.storage().persistent().remove(&legacy_key);
    }
    legacy
}

pub fn set_folder_owner(env: &Env, folder_name: &String, owner: &Address) {
    let folder_key = folder_owner_key(env, folder_name);
    let owner_key = DataKey::OwnerFolder(owner.clone());
    env.storage().persistent().set(&folder_key, owner);
    env.storage().persistent().set(&owner_key, folder_name);
    env.storage().persistent().extend_ttl(
        &folder_key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );
    env.storage().persistent().extend_ttl(
        &owner_key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );
}

/// Read-only getter: does not extend the TTL (see `get_folder_owner`).
pub fn get_owner_folder(env: &Env, owner: &Address) -> Option<String> {
    env.storage()
        .persistent()
        .get(&DataKey::OwnerFolder(owner.clone()))
}

// ── Template Registry ─────────────────────────────────────────────────────────

fn extend_persistent(env: &Env, key: &DataKey) {
    env.storage().persistent().extend_ttl(
        key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );
}

/// Templates live in persistent storage so the registry cannot grow the
/// contract instance entry, which is loaded on every invocation.
/// Read-only getter: does not extend the TTL (see `get_folder_owner`).
pub fn get_template(env: &Env, template_id: u32) -> Option<Template> {
    env.storage().persistent().get(&DataKey::Template(template_id))
}

/// Saves a template reusing an already-loaded registry list, avoiding a second
/// read of `TemplateList` in write paths that had to load it for validation.
pub fn save_template_with_list(env: &Env, template: &Template, mut list: Vec<u32>) {
    let id = template.template_id;
    let key = DataKey::Template(id);
    env.storage().persistent().set(&key, template);
    extend_persistent(env, &key);

    if !list.contains(id) {
        list.push_back(id);
        env.storage()
            .persistent()
            .set(&DataKey::TemplateList, &list);
    }
    extend_persistent(env, &DataKey::TemplateList);
}

/// Read-only getter: does not extend the TTL (see `get_folder_owner`).
pub fn get_template_list(env: &Env) -> Vec<u32> {
    env.storage()
        .persistent()
        .get(&DataKey::TemplateList)
        .unwrap_or(Vec::new(env))
}

/// Read-only getter: does not extend the TTL (see `get_folder_owner`).
pub fn get_user_template(env: &Env, owner: &Address) -> Option<u32> {
    env.storage()
        .persistent()
        .get(&DataKey::UserTemplate(owner.clone()))
}

pub fn set_user_template(env: &Env, owner: &Address, template_id: u32) {
    let key = DataKey::UserTemplate(owner.clone());
    env.storage().persistent().set(&key, &template_id);
    env.storage().persistent().extend_ttl(
        &key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );
}

// ── Public TTL extension ──────────────────────────────────────────────────────

/// Extends the TTL of `key` if it exists. Returns `true` when extended.
fn extend_if_present(env: &Env, key: &DataKey) -> bool {
    if env.storage().persistent().has(key) {
        extend_persistent(env, key);
        true
    } else {
        false
    }
}

/// Extends the TTL of every persistent key belonging to `owner`:
/// `OwnerFolder`, `FolderOwner(name)`, `UserTemplate` and `Portfolio`.
/// Missing keys are skipped. Returns the number of keys extended.
pub fn extend_owner_ttl(env: &Env, owner: &Address) -> u32 {
    let mut count = 0;

    let owner_key = DataKey::OwnerFolder(owner.clone());
    if let Some(folder_name) = env
        .storage()
        .persistent()
        .get::<DataKey, String>(&owner_key)
    {
        if extend_if_present(env, &owner_key) {
            count += 1;
        }
        if extend_if_present(env, &DataKey::FolderOwner(folder_name)) {
            count += 1;
        }
    }

    if extend_if_present(env, &DataKey::UserTemplate(owner.clone())) {
        count += 1;
    }
    if extend_if_present(env, &DataKey::Portfolio(owner.clone())) {
        count += 1;
    }

    count
}

/// Extends the TTL of `Template(template_id)` and `TemplateList` if present.
/// Missing keys are skipped. Returns the number of keys extended.
pub fn extend_template_ttl(env: &Env, template_id: u32) -> u32 {
    let mut count = 0;
    if extend_if_present(env, &DataKey::Template(template_id)) {
        count += 1;
    }
    if extend_if_present(env, &DataKey::TemplateList) {
        count += 1;
    }
    count
}

// ── Portfolio NFT ─────────────────────────────────────────────────────────────

/// Single-read existence check + `get_portfolio`: returns the
/// stored portfolio (if any) and extends its TTL, so write paths do not pay
/// for a `has` followed by a `get`.
pub fn load_portfolio(env: &Env, owner: &Address) -> Option<PortfolioNFT> {
    let key = DataKey::Portfolio(owner.clone());
    let result: Option<PortfolioNFT> = env.storage().persistent().get(&key);
    if result.is_some() {
        env.storage().persistent().extend_ttl(
            &key,
            PERSISTENT_LIFETIME_THRESHOLD,
            PERSISTENT_BUMP_AMOUNT,
        );
    }
    result
}

pub fn get_portfolio(env: &Env, owner: &Address) -> Result<PortfolioNFT, ContractError> {
    let key = DataKey::Portfolio(owner.clone());
    let result: Option<PortfolioNFT> = env.storage().persistent().get(&key);
    match result {
        Some(nft) => {
            env.storage().persistent().extend_ttl(
                &key,
                PERSISTENT_LIFETIME_THRESHOLD,
                PERSISTENT_BUMP_AMOUNT,
            );
            Ok(nft)
        }
        None => Err(ContractError::PortfolioNotFound),
    }
}

pub fn save_portfolio(env: &Env, portfolio: &PortfolioNFT) {
    let key = DataKey::Portfolio(portfolio.owner.clone());
    env.storage().persistent().set(&key, portfolio);
    env.storage().persistent().extend_ttl(
        &key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );
}

// ── Economics ─────────────────────────────────────────────────────────────────

pub fn get_claim_fee(env: &Env) -> Option<ClaimFee> {
    env.storage().instance().get(&DataKey::ClaimFee)
}

pub fn set_claim_fee(env: &Env, fee: &ClaimFee) {
    env.storage().instance().set(&DataKey::ClaimFee, fee);
}

// ── Blocklist ──────────────────────────────────────────────────────────────

pub fn is_blocked(env: &Env, address: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::BlockedAddress(address.clone()))
}

pub fn block_address(env: &Env, address: &Address) {
    let key = DataKey::BlockedAddress(address.clone());
    env.storage().persistent().set(&key, &true);
    extend_persistent(env, &key);
}

pub fn unblock_address(env: &Env, address: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::BlockedAddress(address.clone()));
}

// ── Portfolio flags ────────────────────────────────────────────────────────

pub fn get_portfolio_flag(env: &Env, owner: &Address) -> u32 {
    let key = DataKey::PortfolioFlag(owner.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

pub fn set_portfolio_flag(env: &Env, owner: &Address, flag: u32) {
    let key = DataKey::PortfolioFlag(owner.clone());
    if flag == 0 {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &flag);
        extend_persistent(env, &key);
    }
}

// ── Template submissions ──────────────────────────────────────────────────

pub fn get_pending_template_submissions(env: &Env) -> Vec<u32> {
    env.storage()
        .instance()
        .get(&DataKey::PendingTemplateSubmissions)
        .unwrap_or(Vec::new(env))
}

pub fn save_pending_template_submissions(env: &Env, pending: &Vec<u32>) {
    env.storage()
        .instance()
        .set(&DataKey::PendingTemplateSubmissions, pending);
}

pub fn next_template_submission_id(env: &Env) -> u32 {
    let id: u32 = env
        .storage()
        .instance()
        .get(&DataKey::NextTemplateSubmissionId)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::NextTemplateSubmissionId, &(id + 1));
    id
}

pub fn get_template_submission(env: &Env, submission_id: u32) -> Option<TemplateSubmission> {
    let key = DataKey::TemplateSubmission(submission_id);
    env.storage().persistent().get(&key)
}

pub fn save_template_submission(
    env: &Env,
    submission_id: u32,
    submission: &TemplateSubmission,
) {
    let key = DataKey::TemplateSubmission(submission_id);
    env.storage().persistent().set(&key, submission);
    extend_persistent(env, &key);
}

pub fn remove_template_submission(env: &Env, submission_id: u32) {
    env.storage()
        .persistent()
        .remove(&DataKey::TemplateSubmission(submission_id));
}

pub fn get_template_designer(env: &Env, template_id: u32) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::TemplateDesigner(template_id))
}

pub fn set_template_designer(env: &Env, template_id: u32, designer: &Address) {
    let key = DataKey::TemplateDesigner(template_id);
    env.storage().persistent().set(&key, designer);
    extend_persistent(env, &key);
}

// ── Template usage ─────────────────────────────────────────────────────────

pub fn get_template_usage(env: &Env, template_id: u32) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::TemplateUsage(template_id))
        .unwrap_or(0)
}

pub fn set_template_usage(env: &Env, template_id: u32, usage: u32) {
    let key = DataKey::TemplateUsage(template_id);
    if usage == 0 {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &usage);
        extend_persistent(env, &key);
    }
}

// ── Pricing and leases (baseline storage) ──────────────────────────────────

pub fn get_fee_config(env: &Env) -> Option<FeeConfig> {
    env.storage().instance().get(&DataKey::FeeConfig)
}

pub fn set_fee_config(env: &Env, config: &FeeConfig) {
    env.storage().instance().set(&DataKey::FeeConfig, config);
}

pub fn get_price_tiers(env: &Env) -> PriceTiers {
    env.storage()
        .instance()
        .get(&DataKey::PriceTiers)
        .unwrap_or(PriceTiers {
            by_len: Vec::new(env),
        })
}

pub fn set_price_tiers(env: &Env, tiers: &PriceTiers) {
    env.storage().instance().set(&DataKey::PriceTiers, tiers);
}

pub fn get_lease_config(env: &Env) -> Option<LeaseConfig> {
    env.storage().instance().get(&DataKey::LeaseConfig)
}

pub fn set_lease_config(env: &Env, config: &LeaseConfig) {
    env.storage().instance().set(&DataKey::LeaseConfig, config);
}

pub fn get_folder_expiry(env: &Env, folder_name: &String) -> Option<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::FolderExpiry(folder_name.clone()))
}

pub fn set_folder_expiry(env: &Env, folder_name: &String, expiry: u64) {
    let key = DataKey::FolderExpiry(folder_name.clone());
    env.storage().persistent().set(&key, &expiry);
    extend_persistent(env, &key);
}

pub fn remove_folder(env: &Env, folder_name: &String, owner: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::FolderOwner(folder_name.clone()));
    env.storage()
        .persistent()
        .remove(&DataKey::OwnerFolder(owner.clone()));
    env.storage()
        .persistent()
        .remove(&DataKey::FolderExpiry(folder_name.clone()));
}
