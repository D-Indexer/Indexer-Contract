use soroban_sdk::{contracttype, Address, String};

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
    // admin timelock: pending queue of admin changes
    PendingAdminChanges,
    // admin timelock: next op ID to hand out
    NextAdminChangeId,
}

/// Kind of change queued through the admin timelock.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdminOp {
    Add,
    Remove,
}

/// A queued admin change, executable once `eta_ledger` is reached.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminChange {
    pub op: AdminOp,
    pub target: Address,
    pub eta_ledger: u32,
}
