use soroban_sdk::{contractevent, Address, Env, String};

#[contractevent(topics = ["folder_cl"], data_format = "vec")]
pub struct FolderClaimed {
    pub owner: Address,
    pub folder_name: String,
}

#[contractevent(topics = ["port_mint"], data_format = "vec")]
pub struct PortfolioMinted {
    pub owner: Address,
    pub template_id: u32,
    pub ipfs_cid: String,
}

#[contractevent(topics = ["port_upd"], data_format = "vec")]
pub struct PortfolioUpdated {
    pub owner: Address,
    pub ipfs_cid: String,
}

#[contractevent(topics = ["tmpl_reg"], data_format = "vec")]
pub struct TemplateRegistered {
    pub template_id: u32,
    pub metadata_uri: String,
}

#[contractevent(topics = ["tmpl_set"], data_format = "vec")]
pub struct TemplateAssigned {
    pub owner: Address,
    pub template_id: u32,
}

pub fn folder_claimed(env: &Env, owner: &Address, folder_name: &String) {
    FolderClaimed {
        owner: owner.clone(),
        folder_name: folder_name.clone(),
    }
    .publish(env);
}

pub fn portfolio_minted(env: &Env, owner: &Address, template_id: u32, ipfs_cid: &String) {
    PortfolioMinted {
        owner: owner.clone(),
        template_id,
        ipfs_cid: ipfs_cid.clone(),
    }
    .publish(env);
}

pub fn portfolio_updated(env: &Env, owner: &Address, ipfs_cid: &String) {
    PortfolioUpdated {
        owner: owner.clone(),
        ipfs_cid: ipfs_cid.clone(),
    }
    .publish(env);
}

pub fn template_registered(env: &Env, template_id: u32, metadata_uri: &String) {
    TemplateRegistered {
        template_id,
        metadata_uri: metadata_uri.clone(),
    }
    .publish(env);
}

pub fn template_assigned(env: &Env, owner: &Address, template_id: u32) {
    TemplateAssigned {
        owner: owner.clone(),
        template_id,
    }
    .publish(env);
}
