use soroban_sdk::{contracttype, Address, String, Symbol};

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortfolioNFT {
    pub owner: Address,
    pub template_id: u32,
    pub ipfs_cid: String,
    pub minted_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    pub template_id: u32,
    pub metadata_uri: String,
    pub creator: Address,
}

#[contracttype]
pub enum DataKey {
    AdminList,
    Initialized,
    // identity_map: folder_name -> owner
    FolderOwner(String),
    // identity_map: owner -> folder_name
    OwnerFolder(Address),
    // template_registry: template_id -> Template
    Template(u32),
    TemplateList,
    // user template selection: owner -> template_id
    UserTemplate(Address),
    // nft_mint: owner -> PortfolioNFT
    Portfolio(Address),
    // admin audit log ring buffer: slot -> AdminLogEntry
    AdminLog(u32),
    // total number of admin log entries ever written (next seq)
    AdminLogHead,
}

/// What an admin action acted on.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdminSubject {
    Address(Address),
    Template(u32),
}

/// One entry of the bounded on-chain admin audit log.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminLogEntry {
    pub seq: u32,
    pub actor: Address,
    pub action: Symbol,
    pub subject: AdminSubject,
    pub ledger: u32,
}
