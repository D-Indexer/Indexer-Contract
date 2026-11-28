#![no_std]

use soroban_sdk::{contract, contractclient, contracterror, contractimpl, token, Address, Env, String};

/// Minimal cross-contract interface exposed by the Indexer Folder contract.
#[contractclient(name = "FolderResolverClient")]
pub trait FolderResolver {
    fn resolve(env: Env, folder_name: String) -> Option<Address>;
    fn reverse(env: Env, owner: Address) -> Option<String>;
}

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FolderPayError {
    NameNotFound = 1,
}

#[contract]
pub struct FolderPay;

#[contractimpl]
impl FolderPay {
    /// Pays `amount` of the SAC `token` to the owner of `folder_name`.
    ///
    /// The payer must authorize the transfer; the recipient is resolved through
    /// the Folder contract's `resolve` entry point.
    pub fn pay(
        env: Env,
        resolver: Address,
        token: Address,
        payer: Address,
        folder_name: String,
        amount: i128,
    ) -> Result<Address, FolderPayError> {
        let recipient = FolderResolverClient::new(&env, &resolver)
            .resolve(&folder_name)
            .ok_or(FolderPayError::NameNotFound)?;
        token::Client::new(&env, &token).transfer(&payer, &recipient, &amount);
        Ok(recipient)
    }

    /// Resolves a Folder name to its owner without moving funds.
    pub fn resolve(env: Env, resolver: Address, folder_name: String) -> Option<Address> {
        FolderResolverClient::new(&env, &resolver).resolve(&folder_name)
    }

    /// Reverse lookup through the Folder contract.
    pub fn reverse(env: Env, resolver: Address, owner: Address) -> Option<String> {
        FolderResolverClient::new(&env, &resolver).reverse(&owner)
    }
}

#[cfg(test)]
mod test;
