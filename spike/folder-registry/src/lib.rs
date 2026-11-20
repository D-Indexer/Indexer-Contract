//! Spike (#249): the Folder name registry as a standalone contract.
#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, Env, String,
};

const SUFFIX: &[u8] = b".folder";
const MIN_LABEL_LEN: u32 = 3;
const MAX_NAME_LEN: u32 = 64;
const BUMP: u32 = 120 * 17_280;
const THRESHOLD: u32 = 90 * 17_280;

/// Codes match the monolith's `ContractError` values.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    FolderNameTaken = 3,
    FolderNotFound = 4,
    InvalidFolderName = 9,
    FolderAlreadyClaimed = 16,
}

/// Same topic and payload as the monolith's `folder_cl` event.
#[contractevent(topics = ["folder_cl"], data_format = "vec")]
pub struct FolderClaimed {
    pub owner: Address,
    pub folder_name: String,
}

#[contracttype]
enum Key {
    FolderOwner(String),
    OwnerFolder(Address),
}

fn valid_name(name: &String) -> bool {
    let len = name.len();
    if len > MAX_NAME_LEN || len < MIN_LABEL_LEN + SUFFIX.len() as u32 {
        return false;
    }
    let mut buf = [0u8; MAX_NAME_LEN as usize];
    let bytes = &mut buf[..len as usize];
    name.copy_into_slice(bytes);
    let (label, suffix) = bytes.split_at(bytes.len() - SUFFIX.len());
    suffix == SUFFIX
        && label
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        && label.first() != Some(&b'-')
        && label.last() != Some(&b'-')
}

fn get<V: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(env: &Env, key: &Key) -> Option<V> {
    let value = env.storage().persistent().get(key);
    if value.is_some() {
        env.storage().persistent().extend_ttl(key, THRESHOLD, BUMP);
    }
    value
}

#[contract]
pub struct FolderRegistry;

#[contractimpl]
impl FolderRegistry {
    pub fn claim_folder(
        env: Env,
        owner: Address,
        folder_name: String,
    ) -> Result<(), RegistryError> {
        owner.require_auth();
        if !valid_name(&folder_name) {
            return Err(RegistryError::InvalidFolderName);
        }
        let owner_key = Key::OwnerFolder(owner.clone());
        let folder_key = Key::FolderOwner(folder_name.clone());
        if get::<String>(&env, &owner_key).is_some() {
            return Err(RegistryError::FolderAlreadyClaimed);
        }
        if get::<Address>(&env, &folder_key).is_some() {
            return Err(RegistryError::FolderNameTaken);
        }
        let storage = env.storage().persistent();
        storage.set(&folder_key, &owner);
        storage.set(&owner_key, &folder_name);
        storage.extend_ttl(&folder_key, THRESHOLD, BUMP);
        storage.extend_ttl(&owner_key, THRESHOLD, BUMP);
        FolderClaimed { owner, folder_name }.publish(&env);
        Ok(())
    }

    pub fn get_folder(env: Env, folder_name: String) -> Result<Address, RegistryError> {
        get(&env, &Key::FolderOwner(folder_name)).ok_or(RegistryError::FolderNotFound)
    }

    pub fn get_identity(env: Env, owner: Address) -> Result<String, RegistryError> {
        get(&env, &Key::OwnerFolder(owner)).ok_or(RegistryError::FolderNotFound)
    }
}

#[cfg(test)]
mod test;
