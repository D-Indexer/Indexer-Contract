use soroban_sdk::{contracttype, Address, BytesN, String, Vec};

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortfolioNFT {
    pub owner: Address,
    pub template_id: u32,
    pub ipfs_cid: String,
    pub minted_at: u64,
}

/// Per-portfolio change tracking, stored next to (not inside) `PortfolioNFT`
/// so the XDR layout of existing `Portfolio` entries does not change.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortfolioMeta {
    /// Number of state-changing calls since mint (0 right after mint).
    pub revision: u32,
    /// Ledger timestamp of the last change (`minted_at` right after mint).
    pub updated_at: u64,
}

/// An address allowed to update a portfolio's CID on the owner's behalf.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortfolioDelegate {
    pub delegate: Address,
    /// Last ledger sequence (inclusive) at which the delegate may act.
    /// 0 means no expiry.
    pub expires_ledger: u32,
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
    TemplateList,
    // user template selection: owner -> template_id
    UserTemplate(Address),
    // nft_mint: owner -> PortfolioNFT
    Portfolio(Address),
    // nft_mint: owner -> PortfolioMeta (absent for portfolios minted before it existed)
    PortfolioMeta(Address),
    // nft_mint: owner -> PortfolioDelegate (at most one per portfolio)
    PortfolioDelegate(Address),
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
