use soroban_sdk::{contractevent, Address, BytesN, Env, String};

#[contractevent(topics = ["folder_cl"], data_format = "vec")]
pub struct FolderClaimed {
    pub owner: Address,
    pub folder_name: String,
    /// Appended field: the claim fee paid, 0 when the fee is disabled.
    pub fee_paid: i128,
}

#[contractevent(topics = ["fee_set"], data_format = "vec")]
pub struct ClaimFeeSet {
    pub admin: Address,
    pub token: Address,
    pub amount: i128,
    pub treasury: Address,
}

#[contractevent(topics = ["withdraw"], data_format = "vec")]
pub struct Withdrawn {
    pub admin: Address,
    pub token: Address,
    pub to: Address,
    pub amount: i128,
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

#[contractevent(topics = ["port_burn"], data_format = "vec")]
pub struct PortfolioBurned {
    pub owner: Address,
    /// The CID the portfolio pointed at when it was burned.
    pub last_ipfs_cid: String,
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

#[contractevent(topics = ["upgraded"], data_format = "vec")]
pub struct ContractUpgraded {
    pub admin: Address,
    pub new_wasm_hash: BytesN<32>,
}

pub fn contract_upgraded(env: &Env, admin: &Address, new_wasm_hash: &BytesN<32>) {
    ContractUpgraded {
        admin: admin.clone(),
        new_wasm_hash: new_wasm_hash.clone(),
    }
    .publish(env);
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

#[contractevent(topics = ["adm_q"], data_format = "vec")]
pub struct AdminChangeQueued {
    pub admin: Address,
    pub op_id: u32,
    pub target: Address,
    pub eta_ledger: u32,
}

#[contractevent(topics = ["adm_ex"], data_format = "vec")]
pub struct AdminChangeExecuted {
    pub admin: Address,
    pub op_id: u32,
    pub target: Address,
}

#[contractevent(topics = ["adm_cx"], data_format = "vec")]
pub struct AdminChangeCancelled {
    pub admin: Address,
    pub op_id: u32,
    pub target: Address,
}

pub fn admin_change_queued(
    env: &Env,
    admin: &Address,
    op_id: u32,
    target: &Address,
    eta_ledger: u32,
) {
    AdminChangeQueued {
        admin: admin.clone(),
        op_id,
        target: target.clone(),
        eta_ledger,
    }
    .publish(env);
}

pub fn admin_change_executed(env: &Env, admin: &Address, op_id: u32, target: &Address) {
    AdminChangeExecuted {
        admin: admin.clone(),
        op_id,
        target: target.clone(),
    }
    .publish(env);
}

pub fn admin_change_cancelled(env: &Env, admin: &Address, op_id: u32, target: &Address) {
    AdminChangeCancelled {
        admin: admin.clone(),
        op_id,
        target: target.clone(),
    }
    .publish(env);
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

pub fn folder_claimed(env: &Env, owner: &Address, folder_name: &String, fee_paid: i128) {
    FolderClaimed {
        owner: owner.clone(),
        folder_name: folder_name.clone(),
        fee_paid,
    }
    .publish(env);
}

pub fn claim_fee_set(env: &Env, admin: &Address, fee: &crate::types::ClaimFee) {
    ClaimFeeSet {
        admin: admin.clone(),
        token: fee.token.clone(),
        amount: fee.amount,
        treasury: fee.treasury.clone(),
    }
    .publish(env);
}

pub fn withdrawn(env: &Env, admin: &Address, token: &Address, to: &Address, amount: i128) {
    Withdrawn {
        admin: admin.clone(),
        token: token.clone(),
        to: to.clone(),
        amount,
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

pub fn portfolio_burned(env: &Env, owner: &Address, last_ipfs_cid: &String) {
    PortfolioBurned {
        owner: owner.clone(),
        last_ipfs_cid: last_ipfs_cid.clone(),
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

/// Emitted by `sponsored_claim` in addition to the unchanged `folder_cl`
/// event, so existing `folder_cl` consumers keep decoding the same payload.
#[contractevent(topics = ["claim_spon"], data_format = "vec")]
pub struct ClaimSponsored {
    pub sponsor: Address,
    pub owner: Address,
    pub folder_name: String,
}

#[contractevent(topics = ["folder_rn"], data_format = "vec")]
pub struct FolderRenewed {
    pub owner: Address,
    pub folder_name: String,
    pub expiry: u64,
}

pub fn claim_sponsored(env: &Env, sponsor: &Address, owner: &Address, folder_name: &String) {
    ClaimSponsored {
        sponsor: sponsor.clone(),
        owner: owner.clone(),
        folder_name: folder_name.clone(),
    }
    .publish(env);
}

pub fn folder_renewed(env: &Env, owner: &Address, folder_name: &String, expiry: u64) {
    FolderRenewed {
        owner: owner.clone(),
        folder_name: folder_name.clone(),
        expiry,
    }
    .publish(env);
}
