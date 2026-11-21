use soroban_sdk::{Address, Env, String, Vec};

use crate::constants::{
    INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT,
    PERSISTENT_LIFETIME_THRESHOLD,
};
use crate::errors::ContractError;
use crate::types::{
    AdminChange, ClaimFee, DataKey, LeaseConfig, PortfolioDelegate, PortfolioMeta, PortfolioNFT,
    PriceTiers, Template,
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

// ── Admin ─────────────────────────────────────────────────────────────────────

/// Returns the admin list. Membership is stored per admin under
/// `Admin(Address)` (value: its slot) and `AdminAt(slot)` (value: the admin),
/// so membership checks are O(1) and the set stays enumerable. The legacy
/// `AdminList` vector is migrated on first read.
pub fn get_admins(env: &Env) -> Vec<Address> {
    let count = admin_count(env);
    let mut admins = Vec::new(env);
    for i in 0..count {
        if let Some(admin) = env.storage().instance().get(&DataKey::AdminAt(i)) {
            admins.push_back(admin);
        }
    }
    admins
}

/// Number of admins, migrating the legacy `AdminList` first if needed.
pub fn admin_count(env: &Env) -> u32 {
    migrate_admin_list(env);
    raw_admin_count(env)
}

fn raw_admin_count(env: &Env) -> u32 {
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
    insert_admin(env, address);
    true
}

fn insert_admin(env: &Env, address: &Address) {
    let store = env.storage().instance();
    let slot = raw_admin_count(env);
    store.set(&DataKey::Admin(address.clone()), &slot);
    store.set(&DataKey::AdminAt(slot), address);
    store.set(&DataKey::AdminCount, &(slot + 1));
}

/// Removes `address` from the admins. Returns `false` if it was not one.
/// The last slot is moved into the freed one, keeping slots dense.
pub fn remove_admin_entry(env: &Env, address: &Address) -> bool {
    if !is_admin(env, address) {
        return false;
    }
    let store = env.storage().instance();
    let slot: u32 = store.get(&DataKey::Admin(address.clone())).unwrap_or(0);
    let last = raw_admin_count(env) - 1;
    if slot != last {
        if let Some(moved) = store.get::<_, Address>(&DataKey::AdminAt(last)) {
            store.set(&DataKey::AdminAt(slot), &moved);
            store.set(&DataKey::Admin(moved), &slot);
        }
    }
    store.remove(&DataKey::AdminAt(last));
    store.remove(&DataKey::Admin(address.clone()));
    store.set(&DataKey::AdminCount, &last);
    true
}

/// Replaces the whole admin set (used by `initialize`).
pub fn save_admins(env: &Env, admins: &Vec<Address>) {
    for admin in get_admins(env).iter() {
        remove_admin_entry(env, &admin);
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
        // Remove first so the `is_admin` calls below do not recurse here.
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

pub fn get_folder_owner(env: &Env, folder_name: &String) -> Option<Address> {
    let key = DataKey::FolderOwner(folder_name.clone());
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        env.storage().persistent().extend_ttl(
            &key,
            PERSISTENT_LIFETIME_THRESHOLD,
            PERSISTENT_BUMP_AMOUNT,
        );
    }
    result
}

pub fn set_folder_owner(env: &Env, folder_name: &String, owner: &Address) {
    let folder_key = DataKey::FolderOwner(folder_name.clone());
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

pub fn get_owner_folder(env: &Env, owner: &Address) -> Option<String> {
    let key = DataKey::OwnerFolder(owner.clone());
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        env.storage().persistent().extend_ttl(
            &key,
            PERSISTENT_LIFETIME_THRESHOLD,
            PERSISTENT_BUMP_AMOUNT,
        );
    }
    result
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
pub fn get_template(env: &Env, template_id: u32) -> Option<Template> {
    let key = DataKey::Template(template_id);
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        extend_persistent(env, &key);
    }
    result
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

pub fn get_template_list(env: &Env) -> Vec<u32> {
    let key = DataKey::TemplateList;
    match env.storage().persistent().get(&key) {
        Some(list) => {
            extend_persistent(env, &key);
            list
        }
        None => Vec::new(env),
    }
}

pub fn get_user_template(env: &Env, owner: &Address) -> Option<u32> {
    let key = DataKey::UserTemplate(owner.clone());
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        env.storage().persistent().extend_ttl(
            &key,
            PERSISTENT_LIFETIME_THRESHOLD,
            PERSISTENT_BUMP_AMOUNT,
        );
    }
    result
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

// ── Portfolio NFT ─────────────────────────────────────────────────────────────

/// Single-read variant of `portfolio_exists` + `get_portfolio`: returns the
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

/// Returns the stored change-tracking entry, if any. Portfolios minted before
/// `PortfolioMeta` existed have none; see `nft_mint::portfolio_meta`.
pub fn get_portfolio_meta(env: &Env, owner: &Address) -> Option<PortfolioMeta> {
    let key = DataKey::PortfolioMeta(owner.clone());
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        extend_persistent(env, &key);
    }
    result
}

pub fn save_portfolio_meta(env: &Env, owner: &Address, meta: &PortfolioMeta) {
    let key = DataKey::PortfolioMeta(owner.clone());
    env.storage().persistent().set(&key, meta);
    extend_persistent(env, &key);
}

pub fn get_portfolio_delegate(env: &Env, owner: &Address) -> Option<PortfolioDelegate> {
    let key = DataKey::PortfolioDelegate(owner.clone());
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        extend_persistent(env, &key);
    }
    result
}

pub fn set_portfolio_delegate(env: &Env, owner: &Address, delegate: &PortfolioDelegate) {
    let key = DataKey::PortfolioDelegate(owner.clone());
    env.storage().persistent().set(&key, delegate);
    extend_persistent(env, &key);
}

pub fn remove_portfolio_delegate(env: &Env, owner: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::PortfolioDelegate(owner.clone()));
}

/// Removes every portfolio-scoped key of `owner`. The identity mapping and
/// `UserTemplate` are owner-scoped and deliberately left in place.
pub fn remove_portfolio(env: &Env, owner: &Address) {
    let persistent = env.storage().persistent();
    persistent.remove(&DataKey::Portfolio(owner.clone()));
    persistent.remove(&DataKey::PortfolioMeta(owner.clone()));
    persistent.remove(&DataKey::PortfolioDelegate(owner.clone()));
}

// ── Economics ─────────────────────────────────────────────────────────────────

pub fn get_claim_fee(env: &Env) -> Option<ClaimFee> {
    env.storage().instance().get(&DataKey::ClaimFee)
}

pub fn set_claim_fee(env: &Env, fee: &ClaimFee) {
    env.storage().instance().set(&DataKey::ClaimFee, fee);
}

// ── Pricing ───────────────────────────────────────────────────────────────────

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

// ── Leases ────────────────────────────────────────────────────────────────────

pub fn get_lease_config(env: &Env) -> Option<LeaseConfig> {
    env.storage().instance().get(&DataKey::LeaseConfig)
}

pub fn set_lease_config(env: &Env, config: &Option<LeaseConfig>) {
    match config {
        Some(c) => env.storage().instance().set(&DataKey::LeaseConfig, c),
        None => env.storage().instance().remove(&DataKey::LeaseConfig),
    }
}

pub fn get_folder_expiry(env: &Env, folder_name: &String) -> Option<u64> {
    let key = DataKey::FolderExpiry(folder_name.clone());
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        extend_persistent(env, &key);
    }
    result
}

pub fn set_folder_expiry(env: &Env, folder_name: &String, expiry: u64) {
    let key = DataKey::FolderExpiry(folder_name.clone());
    env.storage().persistent().set(&key, &expiry);
    extend_persistent(env, &key);
}

/// Removes every key of a released Folder: both directions of the identity
/// mapping (the reverse one only if it still points at this name) and the
/// expiry entry.
pub fn remove_folder(env: &Env, folder_name: &String, owner: &Address) {
    let persistent = env.storage().persistent();
    persistent.remove(&DataKey::FolderOwner(folder_name.clone()));
    persistent.remove(&DataKey::FolderExpiry(folder_name.clone()));
    let owner_key = DataKey::OwnerFolder(owner.clone());
    if persistent.get::<_, String>(&owner_key).as_ref() == Some(folder_name) {
        persistent.remove(&owner_key);
    }
}
