use soroban_sdk::{Address, BytesN, Env, String, Vec};

use crate::constants::{
    DEFAULT_CONTENT_SCHEMA, INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD,
    PERSISTENT_BUMP_AMOUNT, PERSISTENT_LIFETIME_THRESHOLD,
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

/// Reads the portfolio stored at `slot`; slot 0 is the legacy `Portfolio` key.
pub fn get_portfolio_at(
    env: &Env,
    owner: &Address,
    slot: u32,
) -> Result<PortfolioNFT, ContractError> {
    if slot == 0 {
        return get_portfolio(env, owner);
    }
    let key = DataKey::PortfolioSlot(owner.clone(), slot);
    let result: Option<PortfolioNFT> = env.storage().persistent().get(&key);
    match result {
        Some(nft) => {
            extend_persistent(env, &key);
            Ok(nft)
        }
        None => Err(ContractError::PortfolioNotFound),
    }
}

pub fn portfolio_exists_at(env: &Env, owner: &Address, slot: u32) -> bool {
    if slot == 0 {
        return portfolio_exists(env, owner);
    }
    env.storage()
        .persistent()
        .has(&DataKey::PortfolioSlot(owner.clone(), slot))
}

pub fn save_portfolio_at(env: &Env, slot: u32, portfolio: &PortfolioNFT) {
    if slot == 0 {
        return save_portfolio(env, portfolio);
    }
    let key = DataKey::PortfolioSlot(portfolio.owner.clone(), slot);
    env.storage().persistent().set(&key, portfolio);
    extend_persistent(env, &key);
}

/// Non-zero slots minted by `owner`, in mint order.
pub fn get_extra_slots(env: &Env, owner: &Address) -> Vec<u32> {
    let key = DataKey::PortfolioSlots(owner.clone());
    match env.storage().persistent().get(&key) {
        Some(list) => {
            extend_persistent(env, &key);
            list
        }
        None => Vec::new(env),
    }
}

pub fn save_extra_slots(env: &Env, owner: &Address, slots: &Vec<u32>) {
    let key = DataKey::PortfolioSlots(owner.clone());
    env.storage().persistent().set(&key, slots);
    extend_persistent(env, &key);
}

// ── Content schemas ───────────────────────────────────────────────────────────

pub fn get_portfolio_schema(env: &Env, owner: &Address) -> u32 {
    let key = DataKey::PortfolioSchema(owner.clone());
    match env.storage().persistent().get(&key) {
        Some(schema) => {
            extend_persistent(env, &key);
            schema
        }
        None => DEFAULT_CONTENT_SCHEMA,
    }
}

/// The default schema is stored as an absent key, so legacy calls write nothing.
pub fn set_portfolio_schema(env: &Env, owner: &Address, schema: u32) {
    let key = DataKey::PortfolioSchema(owner.clone());
    if schema == DEFAULT_CONTENT_SCHEMA {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &schema);
        extend_persistent(env, &key);
    }
}

pub fn get_template_schemas(env: &Env, template_id: u32) -> Vec<u32> {
    let key = DataKey::TemplateSchemas(template_id);
    match env.storage().persistent().get(&key) {
        Some(list) => {
            extend_persistent(env, &key);
            list
        }
        None => Vec::from_array(env, [DEFAULT_CONTENT_SCHEMA]),
    }
}

pub fn set_template_schemas(env: &Env, template_id: u32, schemas: &Vec<u32>) {
    let key = DataKey::TemplateSchemas(template_id);
    env.storage().persistent().set(&key, schemas);
    extend_persistent(env, &key);
}

// ── Template gating ───────────────────────────────────────────────────────────

pub fn is_template_gated(env: &Env, template_id: u32) -> bool {
    let key = DataKey::TemplateGated(template_id);
    match env.storage().persistent().get(&key) {
        Some(gated) => {
            extend_persistent(env, &key);
            gated
        }
        None => false,
    }
}

pub fn set_template_gated(env: &Env, template_id: u32, gated: bool) {
    let key = DataKey::TemplateGated(template_id);
    if gated {
        env.storage().persistent().set(&key, &true);
        extend_persistent(env, &key);
    } else {
        env.storage().persistent().remove(&key);
    }
}

pub fn is_template_allowed(env: &Env, template_id: u32, owner: &Address) -> bool {
    let key = DataKey::TemplateAllow(template_id, owner.clone());
    let allowed = env.storage().persistent().has(&key);
    if allowed {
        extend_persistent(env, &key);
    }
    allowed
}

pub fn set_template_allowed(env: &Env, template_id: u32, owner: &Address, allowed: bool) {
    let key = DataKey::TemplateAllow(template_id, owner.clone());
    if allowed {
        env.storage().persistent().set(&key, &());
        extend_persistent(env, &key);
    } else {
        env.storage().persistent().remove(&key);
    }
}

// ── Event cursor ──────────────────────────────────────────────────────────────

/// Returns the next (gapless) event sequence number, starting at 1.
pub fn next_event_seq(env: &Env) -> u64 {
    get_event_seq(env) + 1
}

pub fn get_event_seq(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::EventSeq)
        .unwrap_or(0)
}

/// Rolling event digest; 32 zero bytes before the first event.
pub fn get_event_digest(env: &Env) -> BytesN<32> {
    env.storage()
        .instance()
        .get(&DataKey::EventDigest)
        .unwrap_or(BytesN::from_array(env, &[0; 32]))
}

pub fn set_event_cursor(env: &Env, seq: u64, digest: &BytesN<32>) {
    env.storage().instance().set(&DataKey::EventSeq, &seq);
    env.storage().instance().set(&DataKey::EventDigest, digest);
}
