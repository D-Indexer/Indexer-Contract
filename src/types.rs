use soroban_sdk::{contracttype, Address, BytesN, String};

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

/// Client-facing validation rules and limits. Every field mirrors a constant
/// in `constants.rs`; see `FolderContract::get_limits`.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Minimum length of the label before the folder suffix.
    pub min_folder_label_len: u32,
    /// Maximum accepted folder name length, suffix included.
    pub max_folder_name_len: u32,
    /// Required folder name suffix (e.g. `.folder`).
    pub folder_suffix: String,
    /// Minimum accepted IPFS CID length.
    pub min_ipfs_cid_len: u32,
    /// Maximum accepted IPFS CID length.
    pub max_ipfs_cid_len: u32,
    /// Required template metadata URI scheme (e.g. `ipfs://`).
    pub metadata_uri_scheme: String,
    /// Maximum accepted template metadata URI length.
    pub max_metadata_uri_len: u32,
    /// Maximum number of registered templates.
    pub max_templates: u32,
    /// Maximum number of admins.
    pub max_admins: u32,
    /// Ledgers per day used by the TTL policy.
    pub day_in_ledgers: u32,
    /// Persistent entry bump amount, in ledgers.
    pub persistent_bump_amount: u32,
    /// Persistent entry re-extension threshold, in ledgers.
    pub persistent_lifetime_threshold: u32,
    /// Instance bump amount, in ledgers.
    pub instance_bump_amount: u32,
    /// Instance re-extension threshold, in ledgers.
    pub instance_lifetime_threshold: u32,
}

#[contracttype]
pub enum DataKey {
    AdminList,
    Initialized,
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
}
