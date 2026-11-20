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
    // storage: current storage schema version
    StorageVersion,
    // governance: current guardian address
    Guardian,
    // governance: contract paused flag
    Paused,
    // governance: queued admin-set change -> ready ledger
    PendingChange(u32),
    // governance: monotonically increasing pending change id
    PendingNonce,
}

/// Kinds of admin-set changes that must be queued and timelocked.
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    AddAdmin = 0,
    RemoveAdmin = 1,
    SetGuardian = 2,
}

/// A queued, timelocked admin-set change.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingChange {
    pub kind: ChangeKind,
    pub target: Address,
    pub ready_at: u32,
}
