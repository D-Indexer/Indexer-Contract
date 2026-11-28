//! Spike (#249): the template registry as a standalone contract.
#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String, Vec};

const URI_SCHEME: &[u8] = b"ipfs://";
const MAX_URI_LEN: u32 = 256;
const MAX_TEMPLATES: u32 = 256;
const BUMP: u32 = 120 * 17_280;
const THRESHOLD: u32 = 90 * 17_280;

/// Codes match the monolith's `ContractError` values.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TemplatesError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    TemplateNotFound = 6,
    Unauthorized = 7,
    InvalidMetadataURI = 14,
    TemplateAlreadyExists = 15,
    InvalidTemplateID = 17,
    TemplateLimitReached = 18,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    pub template_id: u32,
    pub metadata_uri: String,
    pub creator: Address,
}

#[contracttype]
enum Key {
    Admin,
    Template(u32),
    TemplateList,
}

#[contract]
pub struct FolderTemplates;

#[contractimpl]
impl FolderTemplates {
    pub fn initialize(env: Env, admin: Address) -> Result<(), TemplatesError> {
        admin.require_auth();
        if env.storage().instance().has(&Key::Admin) {
            return Err(TemplatesError::AlreadyInitialized);
        }
        env.storage().instance().set(&Key::Admin, &admin);
        Ok(())
    }

    pub fn register_template(
        env: Env,
        admin: Address,
        template_id: u32,
        metadata_uri: String,
    ) -> Result<(), TemplatesError> {
        let stored: Address = env
            .storage()
            .instance()
            .get(&Key::Admin)
            .ok_or(TemplatesError::NotInitialized)?;
        admin.require_auth();
        if admin != stored {
            return Err(TemplatesError::Unauthorized);
        }
        if template_id == 0 {
            return Err(TemplatesError::InvalidTemplateID);
        }
        let len = metadata_uri.len();
        let mut buf = [0u8; MAX_URI_LEN as usize];
        if len <= URI_SCHEME.len() as u32 || len > MAX_URI_LEN {
            return Err(TemplatesError::InvalidMetadataURI);
        }
        metadata_uri.copy_into_slice(&mut buf[..len as usize]);
        if !buf.starts_with(URI_SCHEME) {
            return Err(TemplatesError::InvalidMetadataURI);
        }
        let storage = env.storage().persistent();
        let key = Key::Template(template_id);
        if storage.has(&key) {
            return Err(TemplatesError::TemplateAlreadyExists);
        }
        let mut list: Vec<u32> = storage.get(&Key::TemplateList).unwrap_or(Vec::new(&env));
        if list.len() >= MAX_TEMPLATES {
            return Err(TemplatesError::TemplateLimitReached);
        }
        list.push_back(template_id);
        let template = Template {
            template_id,
            metadata_uri,
            creator: admin,
        };
        storage.set(&key, &template);
        storage.set(&Key::TemplateList, &list);
        storage.extend_ttl(&key, THRESHOLD, BUMP);
        storage.extend_ttl(&Key::TemplateList, THRESHOLD, BUMP);
        Ok(())
    }

    pub fn get_template(env: Env, template_id: u32) -> Result<Template, TemplatesError> {
        if template_id == 0 {
            return Err(TemplatesError::InvalidTemplateID);
        }
        let key = Key::Template(template_id);
        let template = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(TemplatesError::TemplateNotFound)?;
        env.storage().persistent().extend_ttl(&key, THRESHOLD, BUMP);
        Ok(template)
    }

    pub fn list_templates(env: Env) -> Vec<u32> {
        env.storage()
            .persistent()
            .get(&Key::TemplateList)
            .unwrap_or(Vec::new(&env))
    }
}

#[cfg(test)]
mod test;
