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
    /// Appended field: the admin that registered the template.
    pub creator: Address,
}

#[contractevent(topics = ["tmpl_set"], data_format = "vec")]
pub struct TemplateAssigned {
    pub owner: Address,
    pub template_id: u32,
}

#[contractevent(topics = ["init"], data_format = "vec")]
pub struct ContractInitialized {
    pub admin: Address,
}

#[contractevent(topics = ["adm_add"], data_format = "vec")]
pub struct AdminAdded {
    pub admin: Address,
    pub new_admin: Address,
}

#[contractevent(topics = ["adm_rm"], data_format = "vec")]
pub struct AdminRemoved {
    pub admin: Address,
    pub target: Address,
}

pub fn contract_initialized(env: &Env, admin: &Address) {
    ContractInitialized {
        admin: admin.clone(),
    }
    .publish(env);
}

pub fn admin_added(env: &Env, admin: &Address, new_admin: &Address) {
    AdminAdded {
        admin: admin.clone(),
        new_admin: new_admin.clone(),
    }
    .publish(env);
}

pub fn admin_removed(env: &Env, admin: &Address, target: &Address) {
    AdminRemoved {
        admin: admin.clone(),
        target: target.clone(),
    }
    .publish(env);
}

#[contractevent(topics = ["migrated"], data_format = "vec")]
pub struct Migrated {
    pub from: u32,
    pub to: u32,
}

#[contractevent(topics = ["grd_set"], data_format = "vec")]
pub struct GuardianSet {
    pub guardian: Address,
}

#[contractevent(topics = ["grd_cancel"], data_format = "vec")]
pub struct GuardianCancelled {
    pub guardian: Address,
    pub change_id: u32,
}

#[contractevent(topics = ["grd_pause"], data_format = "vec")]
pub struct GuardianPaused {
    pub guardian: Address,
}

#[contractevent(topics = ["unpaused"], data_format = "vec")]
pub struct ContractUnpaused {
    pub admin: Address,
}

#[contractevent(topics = ["chg_queued"], data_format = "vec")]
pub struct ChangeQueued {
    pub change_id: u32,
    pub kind: u32,
    pub target: Address,
    pub ready_at: u32,
}

#[contractevent(topics = ["chg_exec"], data_format = "vec")]
pub struct ChangeExecuted {
    pub change_id: u32,
    pub kind: u32,
    pub target: Address,
}

pub fn guardian_set(env: &Env, guardian: &Address) {
    GuardianSet {
        guardian: guardian.clone(),
    }
    .publish(env);
}

pub fn guardian_cancelled(env: &Env, guardian: &Address, change_id: u32) {
    GuardianCancelled {
        guardian: guardian.clone(),
        change_id,
    }
    .publish(env);
}

pub fn guardian_paused(env: &Env, guardian: &Address) {
    GuardianPaused {
        guardian: guardian.clone(),
    }
    .publish(env);
}

pub fn contract_unpaused(env: &Env, admin: &Address) {
    ContractUnpaused {
        admin: admin.clone(),
    }
    .publish(env);
}

pub fn change_queued(env: &Env, change_id: u32, kind: u32, target: &Address, ready_at: u32) {
    ChangeQueued {
        change_id,
        kind,
        target: target.clone(),
        ready_at,
    }
    .publish(env);
}

pub fn change_executed(env: &Env, change_id: u32, kind: u32, target: &Address) {
    ChangeExecuted {
        change_id,
        kind,
        target: target.clone(),
    }
    .publish(env);
}

pub fn migrated(env: &Env, from: u32, to: u32) {
    Migrated { from, to }.publish(env);
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

pub fn template_registered(env: &Env, template_id: u32, metadata_uri: &String, creator: &Address) {
    TemplateRegistered {
        template_id,
        metadata_uri: metadata_uri.clone(),
        creator: creator.clone(),
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
