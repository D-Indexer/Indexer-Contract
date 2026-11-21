use soroban_sdk::{contracttype, Address, String, Vec};

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
pub enum DataKey {
    AdminList,
    // admin membership: address -> its slot in `AdminAt`
    Admin(Address),
    // admin membership: slot -> address, dense in `0..AdminCount`
    AdminAt(u32),
    // admin membership: number of admins, kept in sync with `Admin(Address)`
    AdminCount,
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
    // economics: Folder claim fee configuration (instance storage)
    ClaimFee,
    // pricing: claim fee settings and length tiers (instance)
    FeeConfig,
    PriceTiers,
    // leases: lease settings (instance) and folder_name -> expiry timestamp
    LeaseConfig,
    FolderExpiry(String),
    // admin timelock: pending queue of admin changes (instance)
    PendingAdminChanges,
    // admin timelock: next op ID to hand out (instance)
    NextAdminChangeId,
}

/// Claim fee settings used by length-based pricing and lease renewals. The
/// fee is paid in `token` to `treasury`.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeConfig {
    pub token: Address,
    pub treasury: Address,
    /// Fee for labels longer than every configured tier.
    pub base_fee: i128,
}

/// Length-based premium pricing: `(max_label_len, amount)` pairs sorted by
/// strictly ascending `max_label_len`. A label uses the first tier whose
/// `max_label_len` is `>=` its length; longer labels pay `base_fee`.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriceTiers {
    pub by_len: Vec<(u32, i128)>,
}

/// Lease settings. Absent (the default) means names are permanent.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaseConfig {
    /// Length of one lease period in seconds.
    pub period_secs: u64,
    /// Grace period after expiry, in seconds, during which only the
    /// previous owner can renew.
    pub grace_secs: u64,
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
