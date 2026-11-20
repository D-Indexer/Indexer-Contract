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

/// Optional price charged when an owner switches to a template. Paid in a
/// Stellar Asset Contract token and split between the template's designer
/// (`designer_bps` basis points) and the treasury (the remainder).
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplatePrice {
    pub token: Address,
    pub amount: i128,
    pub designer: Address,
    pub designer_bps: u32,
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
    // royalties: treasury receiving the non-designer share (instance)
    Treasury,
    // royalties: template_id -> TemplatePrice
    TemplatePrice(u32),
}
