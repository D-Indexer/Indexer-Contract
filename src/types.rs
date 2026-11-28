use soroban_sdk::{contracttype, Address, BytesN, String, Vec};

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

/// Fee charged on `claim_folder`, paid in a Stellar Asset Contract token.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimFee {
    pub token: Address,
    pub amount: i128,
    pub treasury: Address,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeConfig {
    pub token: Address,
    pub treasury: Address,
    pub base_fee: i128,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriceTiers {
    pub by_len: Vec<(u32, i128)>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaseConfig {
    pub period_secs: u64,
    pub grace_secs: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateSubmission {
    pub designer: Address,
    pub metadata_uri: String,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdminOp {
    Add,
    Remove,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminChange {
    pub op: AdminOp,
    pub target: Address,
    pub eta_ledger: u32,
}

#[contracttype]
pub enum DataKey {
    AdminList,
    // admin membership: admin -> its slot index in `AdminAt`
    Admin(Address),
    AdminIndex(u32),
    // admin membership: number of admins, kept in sync with `Admin(Address)`
    AdminCount,
    Initialized,
    // config: admin-tunable protocol limits
    Config,
    // identity_map: sha256(folder_name) -> owner (current layout)
    FolderOwnerHash(BytesN<32>),
    // identity_map: folder_name -> owner (legacy layout, read-only migration path)
    FolderOwner(String),
    // identity_map: owner -> folder_name
    OwnerFolder(Address),
    // template_registry: template_id -> Template
    Template(u32),
    // template_registry: template_id -> approved metadata JSON sha256
    TemplateHash(u32),
    TemplateList,
    // user template selection: owner -> template_id
    UserTemplate(Address),
    // nft_mint: owner -> PortfolioNFT
    Portfolio(Address),
    // economics: Folder claim fee configuration (instance storage)
    ClaimFee,
    FeeConfig,
    PriceTiers,
    LeaseConfig,
    FolderExpiry(String),
    // admin timelock
    PendingAdminChanges,
    NextAdminChangeId,
    // address blocklist
    BlockedAddress(Address),
    // portfolio moderation
    PortfolioFlag(Address),
    // template submission queue
    TemplateSubmission(u32),
    PendingTemplateSubmissions,
    NextTemplateSubmissionId,
    TemplateDesigner(u32),
    // template usage
    TemplateUsage(u32),
}
