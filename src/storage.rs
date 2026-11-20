use soroban_sdk::{Address, Env, String, Vec};

use crate::constants::{
    INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT,
    PERSISTENT_LIFETIME_THRESHOLD,
};
use crate::errors::ContractError;
use crate::types::{ClaimFee, DataKey, PortfolioNFT, Template};

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
/// `Admin(Address)`; the legacy `AdminList` vector is migrated on first read.
pub fn get_admins(env: &Env) -> Vec<Address> {
    migrate_admin_list(env);
    let count = admin_count(env);
    let mut admins = Vec::new(env);
    let mut i = 0u32;
    while i < count {
        if let Some(admin) = env.storage().instance().get(&DataKey::Admin(i)) {
            admins.push_back(admin);
        }
        i += 1;
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
        .set(&DataKey::AdminCount, &(count + 1));
    true
}

/// Removes `address` from the admins. Returns `false` if it was not one.
pub fn remove_admin_entry(env: &Env, address: &Address) -> bool {
    if !is_admin(env, address) {
        return false;
    }
    env.storage()
        .instance()
        .remove(&DataKey::Admin(address.clone()));
    let count = admin_count(env);
    env.storage()
        .instance()
        .set(&DataKey::AdminCount, &(count - 1));
    true
}

/// Replaces the whole admin set (used by `initialize`).
pub fn save_admins(env: &Env, admins: &Vec<Address>) {
    let count = admin_count(env);
    let mut i = 0u32;
    while i < count {
        if let Some(admin) = env.storage().instance().get::<_, Address>(&DataKey::Admin(i)) {
            env.storage().instance().remove(&DataKey::Admin(admin));
        }
        i += 1;
    }
    env.storage().instance().set(&DataKey::AdminCount, &0u32);
    for admin in admins.iter() {
        add_admin_entry(env, &admin);
    }
}

/// Idempotently converts a legacy `AdminList` vector into per-admin keys.
/// Does nothing once the legacy entry is gone.
fn migrate_admin_list(env: &Env) {
    let legacy: Option<Vec<Address>> = env.storage().instance().get(&DataKey::AdminList);
    if let Some(admins) = legacy {
        for admin in admins.iter() {
            add_admin_entry(env, &admin);
        }
        env.storage().instance().remove(&DataKey::AdminList);
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

pub fn save_template(env: &Env, template: &Template) {
    save_template_with_list(env, template, get_template_list(env));
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

pub fn portfolio_exists(env: &Env, owner: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Portfolio(owner.clone()))
}

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

// ── Economics ─────────────────────────────────────────────────────────────────

pub fn get_claim_fee(env: &Env) -> Option<ClaimFee> {
    env.storage().instance().get(&DataKey::ClaimFee)
}

pub fn set_claim_fee(env: &Env, fee: &ClaimFee) {
    env.storage().instance().set(&DataKey::ClaimFee, fee);
}
