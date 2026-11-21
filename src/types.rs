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
pub enum DataKey {
    AdminList,
    // admin membership: admin -> its slot index in `AdminAt`
    Admin(Address),
    // admin membership: slot index -> admin, dense in `0..AdminCount`
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
    // admin timelock: pending queue of admin changes
    PendingAdminChanges,
    // admin timelock: next op ID to hand out
    NextAdminChangeId,
    // pricing: length-based premium tiers (instance storage)
    PriceTiers,
    // leases: lease configuration, absent in permanent mode (instance storage)
    LeaseConfig,
    // leases: folder_name -> expiry timestamp
    FolderExpiry(String),
    // commit-reveal: owner -> pending ClaimCommitment (temporary storage)
    ClaimCommit(Address),
    // recovery: owner -> GuardianConfig
    Guardians(Address),
    // recovery: owner -> pending Recovery
    Recovery(Address),
}

/// Guardians allowed to move an owner's identity, and how many must approve.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardianConfig {
    pub guardians: Vec<Address>,
    pub threshold: u32,
}

/// A pending identity recovery to `new_owner`.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovery {
    pub new_owner: Address,
    pub approvals: Vec<Address>,
    pub initiated_ledger: u32,
}

/// A hidden Folder claim made by `commit_claim`: `commitment` is
/// `sha256(folder_name || salt || owner_xdr)`, `ledger` the commit ledger.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimCommitment {
    pub commitment: BytesN<32>,
    pub ledger: u32,
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

/// Length-based premium pricing: `(max_label_len, amount)` pairs sorted by
/// ascending `max_label_len`. Labels longer than every tier pay the base fee
/// (`ClaimFee.amount`).
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriceTiers {
    pub by_len: Vec<(u32, i128)>,
}

/// Lease mode: names expire `period_secs` after claim (or renewal) and can be
/// renewed by their owner for `grace_secs` after expiry.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaseConfig {
    pub period_secs: u64,
    pub grace_secs: u64,
}
