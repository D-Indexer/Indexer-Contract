use soroban_sdk::{Address, Env, String, Vec};

use crate::constants::{
    INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT,
    PERSISTENT_LIFETIME_THRESHOLD,
};
use crate::errors::ContractError;
use crate::types::{DataKey, PortfolioNFT, Template};

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

pub fn get_admins(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::AdminList)
        .unwrap_or(Vec::new(env))
}

pub fn save_admins(env: &Env, admins: &Vec<Address>) {
    env.storage().instance().set(&DataKey::AdminList, admins);
}

pub fn is_admin(env: &Env, address: &Address) -> bool {
    get_admins(env).contains(address)
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
    let id = template.template_id;
    let key = DataKey::Template(id);
    env.storage().persistent().set(&key, template);
    extend_persistent(env, &key);

    let mut list = get_template_list(env);
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

pub fn portfolio_exists(env: &Env, owner: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Portfolio(owner.clone()))
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
