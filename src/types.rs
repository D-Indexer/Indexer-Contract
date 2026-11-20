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
    // events: last emitted event sequence number (instance)
    EventSeq,
    // events: rolling sha256 digest over all emitted events (instance)
    EventDigest,
    // portfolio content schema for the slot-0 portfolio: owner -> u32
    PortfolioSchema(Address),
    // content schemas a template supports: template_id -> Vec<u32>
    TemplateSchemas(u32),
    // extra portfolio slots: (owner, slot) -> PortfolioNFT (slot 0 is `Portfolio`)
    PortfolioSlot(Address, u32),
    // minted non-zero slots of an owner: owner -> Vec<u32>
    PortfolioSlots(Address),
    // gated template flag: template_id -> bool
    TemplateGated(u32),
    // gated template allowlist entry: (template_id, owner) -> ()
    TemplateAllow(u32, Address),
}
