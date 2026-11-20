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
    // Legacy template ID list, migrated lazily to TemplateCount/TemplatePage.
    TemplateList,
    // user template selection: owner -> template_id
    UserTemplate(Address),
    // nft_mint: owner -> PortfolioNFT
    Portfolio(Address),
    // template_registry: number of registered template IDs
    TemplateCount,
    // template_registry: page number -> up to TEMPLATE_PAGE_SIZE template IDs
    TemplatePage(u32),
    // identity_map: folder_name -> payment address
    PaymentAddress(String),
    // identity_map: (folder_name, key) -> record value
    FolderRecord(String, Symbol),
    // identity_map: folder_name -> record keys, in insertion order
    FolderRecordKeys(String),
}
