//! Spike (#249): portfolio records as a standalone contract that checks
//! Folder ownership with `folder-registry` and templates with
//! `folder-templates` through cross-contract calls.
#![no_std]

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, Address, Env, String,
};

const MIN_CID_LEN: u32 = 46;
const MAX_CID_LEN: u32 = 128;
const BUMP: u32 = 120 * 17_280;
const THRESHOLD: u32 = 90 * 17_280;

/// Interface of `folder-registry` used by this contract.
#[contractclient(name = "RegistryClient")]
pub trait Registry {
    fn get_identity(env: Env, owner: Address) -> String;
}

/// Interface of `folder-templates` used by this contract. The template is
/// returned as an opaque value: only its existence matters here.
#[contractclient(name = "TemplatesClient")]
pub trait Templates {
    fn get_template(env: Env, template_id: u32) -> soroban_sdk::Val;
}

/// Codes match the monolith's `ContractError` values.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PortfolioError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    FolderNotFound = 4,
    PortfolioNotFound = 5,
    TemplateNotFound = 6,
    InvalidCID = 8,
    PortfolioAlreadyMinted = 13,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortfolioNFT {
    pub owner: Address,
    pub template_id: u32,
    pub ipfs_cid: String,
    pub minted_at: u64,
}

#[contracttype]
enum Key {
    Registry,
    Templates,
    Portfolio(Address),
}

fn require_folder(env: &Env, owner: &Address) -> Result<(), PortfolioError> {
    let registry: Address = env
        .storage()
        .instance()
        .get(&Key::Registry)
        .ok_or(PortfolioError::NotInitialized)?;
    match RegistryClient::new(env, &registry).try_get_identity(owner) {
        Ok(Ok(_)) => Ok(()),
        _ => Err(PortfolioError::FolderNotFound),
    }
}

fn require_template(env: &Env, template_id: u32) -> Result<(), PortfolioError> {
    let templates: Address = env
        .storage()
        .instance()
        .get(&Key::Templates)
        .ok_or(PortfolioError::NotInitialized)?;
    match TemplatesClient::new(env, &templates).try_get_template(&template_id) {
        Ok(Ok(_)) => Ok(()),
        _ => Err(PortfolioError::TemplateNotFound),
    }
}

fn require_valid_cid(cid: &String) -> Result<(), PortfolioError> {
    if cid.len() < MIN_CID_LEN || cid.len() > MAX_CID_LEN {
        return Err(PortfolioError::InvalidCID);
    }
    Ok(())
}

fn load(env: &Env, owner: &Address) -> Result<PortfolioNFT, PortfolioError> {
    let key = Key::Portfolio(owner.clone());
    let nft = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(PortfolioError::PortfolioNotFound)?;
    env.storage().persistent().extend_ttl(&key, THRESHOLD, BUMP);
    Ok(nft)
}

fn save(env: &Env, nft: &PortfolioNFT) {
    let key = Key::Portfolio(nft.owner.clone());
    env.storage().persistent().set(&key, nft);
    env.storage().persistent().extend_ttl(&key, THRESHOLD, BUMP);
}

#[contract]
pub struct FolderPortfolio;

#[contractimpl]
impl FolderPortfolio {
    pub fn initialize(
        env: Env,
        admin: Address,
        registry: Address,
        templates: Address,
    ) -> Result<(), PortfolioError> {
        admin.require_auth();
        if env.storage().instance().has(&Key::Registry) {
            return Err(PortfolioError::AlreadyInitialized);
        }
        env.storage().instance().set(&Key::Registry, &registry);
        env.storage().instance().set(&Key::Templates, &templates);
        Ok(())
    }

    pub fn mint_portfolio(
        env: Env,
        owner: Address,
        template_id: u32,
        ipfs_cid: String,
    ) -> Result<(), PortfolioError> {
        owner.require_auth();
        require_folder(&env, &owner)?;
        if env
            .storage()
            .persistent()
            .has(&Key::Portfolio(owner.clone()))
        {
            return Err(PortfolioError::PortfolioAlreadyMinted);
        }
        require_valid_cid(&ipfs_cid)?;
        require_template(&env, template_id)?;
        save(
            &env,
            &PortfolioNFT {
                owner,
                template_id,
                ipfs_cid,
                minted_at: env.ledger().timestamp(),
            },
        );
        Ok(())
    }

    pub fn update_portfolio(
        env: Env,
        owner: Address,
        ipfs_cid: String,
    ) -> Result<(), PortfolioError> {
        owner.require_auth();
        require_valid_cid(&ipfs_cid)?;
        let mut nft = load(&env, &owner)?;
        nft.ipfs_cid = ipfs_cid;
        save(&env, &nft);
        Ok(())
    }

    pub fn set_user_template(
        env: Env,
        owner: Address,
        template_id: u32,
    ) -> Result<(), PortfolioError> {
        owner.require_auth();
        require_folder(&env, &owner)?;
        require_template(&env, template_id)?;
        let mut nft = load(&env, &owner)?;
        nft.template_id = template_id;
        save(&env, &nft);
        Ok(())
    }

    pub fn get_portfolio(env: Env, owner: Address) -> Result<PortfolioNFT, PortfolioError> {
        load(&env, &owner)
    }

    pub fn verify_portfolio(env: Env, owner: Address) -> bool {
        env.storage().persistent().has(&Key::Portfolio(owner))
    }
}

#[cfg(test)]
mod test;
