use soroban_sdk::{Address, Env, String, Symbol, Vec};

use crate::constants::{
    INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT,
    PERSISTENT_LIFETIME_THRESHOLD, TEMPLATE_PAGE_SIZE,
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

/// Appends a new template ID. Only the counter and the last page (at most
/// `TEMPLATE_PAGE_SIZE` IDs) are written, so the cost does not grow with the
/// registry size. Callers must ensure the ID is not registered yet.
pub fn save_template(env: &Env, template: &Template) {
    let id = template.template_id;
    let key = DataKey::Template(id);
    env.storage().persistent().set(&key, template);
    extend_persistent(env, &key);

    let count = migrate_template_list(env);
    let page_key = DataKey::TemplatePage(count / TEMPLATE_PAGE_SIZE);
    let mut page: Vec<u32> = env
        .storage()
        .persistent()
        .get(&page_key)
        .unwrap_or(Vec::new(env));
    page.push_back(id);
    env.storage().persistent().set(&page_key, &page);
    extend_persistent(env, &page_key);
    env.storage()
        .persistent()
        .set(&DataKey::TemplateCount, &(count + 1));
    extend_persistent(env, &DataKey::TemplateCount);
}

fn get_legacy_template_list(env: &Env) -> Option<Vec<u32>> {
    env.storage().persistent().get(&DataKey::TemplateList)
}

fn get_template_count_entry(env: &Env) -> Option<u32> {
    let key = DataKey::TemplateCount;
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        extend_persistent(env, &key);
    }
    result
}

/// Moves the legacy `TemplateList` vector into `TemplateCount` and
/// `TemplatePage` entries, preserving order. Idempotent: once the counter
/// exists it is returned unchanged. Returns the template count.
fn migrate_template_list(env: &Env) -> u32 {
    if let Some(count) = get_template_count_entry(env) {
        return count;
    }
    let list = get_legacy_template_list(env).unwrap_or(Vec::new(env));
    let count = list.len();
    let mut start = 0u32;
    while start < count {
        let end = (start + TEMPLATE_PAGE_SIZE).min(count);
        let page_key = DataKey::TemplatePage(start / TEMPLATE_PAGE_SIZE);
        env.storage()
            .persistent()
            .set(&page_key, &list.slice(start..end));
        extend_persistent(env, &page_key);
        start = end;
    }
    env.storage()
        .persistent()
        .set(&DataKey::TemplateCount, &count);
    extend_persistent(env, &DataKey::TemplateCount);
    env.storage().persistent().remove(&DataKey::TemplateList);
    count
}

/// Number of registered templates, read from the counter or, before
/// migration, from the legacy list.
pub fn get_template_count(env: &Env) -> u32 {
    match get_template_count_entry(env) {
        Some(count) => count,
        None => get_legacy_template_list(env).map_or(0, |list| list.len()),
    }
}

/// Template IDs in registration order.
pub fn get_template_list(env: &Env) -> Vec<u32> {
    let Some(count) = get_template_count_entry(env) else {
        return get_legacy_template_list(env).unwrap_or(Vec::new(env));
    };
    let mut list = Vec::new(env);
    for page in 0..count.div_ceil(TEMPLATE_PAGE_SIZE) {
        let key = DataKey::TemplatePage(page);
        if let Some(ids) = env.storage().persistent().get::<_, Vec<u32>>(&key) {
            extend_persistent(env, &key);
            list.append(&ids);
        }
    }
    list
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

// ── Payment address ───────────────────────────────────────────────────────────

pub fn get_payment_address(env: &Env, folder_name: &String) -> Option<Address> {
    let key = DataKey::PaymentAddress(folder_name.clone());
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        extend_persistent(env, &key);
    }
    result
}

pub fn set_payment_address(env: &Env, folder_name: &String, payment: &Address) {
    let key = DataKey::PaymentAddress(folder_name.clone());
    env.storage().persistent().set(&key, payment);
    extend_persistent(env, &key);
}

pub fn remove_payment_address(env: &Env, folder_name: &String) {
    env.storage()
        .persistent()
        .remove(&DataKey::PaymentAddress(folder_name.clone()));
}

// ── Folder records ────────────────────────────────────────────────────────────

pub fn get_record_keys(env: &Env, folder_name: &String) -> Vec<Symbol> {
    let key = DataKey::FolderRecordKeys(folder_name.clone());
    match env.storage().persistent().get(&key) {
        Some(keys) => {
            extend_persistent(env, &key);
            keys
        }
        None => Vec::new(env),
    }
}

pub fn set_record_keys(env: &Env, folder_name: &String, keys: &Vec<Symbol>) {
    let key = DataKey::FolderRecordKeys(folder_name.clone());
    if keys.is_empty() {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, keys);
        extend_persistent(env, &key);
    }
}

pub fn get_record(env: &Env, folder_name: &String, record_key: &Symbol) -> Option<String> {
    let key = DataKey::FolderRecord(folder_name.clone(), record_key.clone());
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        extend_persistent(env, &key);
    }
    result
}

pub fn set_record(env: &Env, folder_name: &String, record_key: &Symbol, value: &String) {
    let key = DataKey::FolderRecord(folder_name.clone(), record_key.clone());
    env.storage().persistent().set(&key, value);
    extend_persistent(env, &key);
}

pub fn remove_record(env: &Env, folder_name: &String, record_key: &Symbol) {
    env.storage().persistent().remove(&DataKey::FolderRecord(
        folder_name.clone(),
        record_key.clone(),
    ));
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
