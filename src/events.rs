use soroban_sdk::{contractevent, Address, BytesN, Env, String};

use crate::types::Config;

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

/// `folder_cl` with `owner` as `topics[1]`, so indexers can filter by owner.
#[contractevent(topics = ["folder_cl", "owner"], data_format = "vec")]
pub struct FolderClaimedByOwner {
    pub owner: Address,
    pub folder_name: String,
}

#[contractevent(topics = ["port_mint"], data_format = "vec")]
pub struct PortfolioMinted {
    pub owner: Address,
    pub template_id: u32,
    pub ipfs_cid: String,
    /// Appended field: the portfolio slot (0 for the legacy portfolio).
    pub slot: u32,
    pub seq: u64,
}

/// `port_mint` with `owner` as `topics[1]`, so indexers can filter by owner.
#[contractevent(topics = ["port_mint", "owner"], data_format = "vec")]
pub struct PortfolioMintedByOwner {
    pub owner: Address,
    pub template_id: u32,
    pub ipfs_cid: String,
}

#[contractevent(topics = ["port_upd"], data_format = "vec")]
pub struct PortfolioUpdated {
    pub owner: Address,
    pub ipfs_cid: String,
    /// Appended field: the CID that was replaced by this update.
    pub previous_cid: String,
}

/// `port_upd` with `owner` as `topics[1]`, so indexers can filter by owner.
#[contractevent(topics = ["port_upd", "owner"], data_format = "vec")]
pub struct PortfolioUpdatedByOwner {
    pub owner: Address,
    pub ipfs_cid: String,
}

#[contractevent(topics = ["tmpl_reg"], data_format = "vec")]
pub struct TemplateRegistered {
    pub template_id: u32,
    pub metadata_uri: String,
    /// Appended field: the admin that registered the template.
    pub creator: Address,
    pub seq: u64,
}

#[contractevent(topics = ["tmpl_set"], data_format = "vec")]
pub struct TemplateAssigned {
    pub owner: Address,
    pub template_id: u32,
    pub seq: u64,
}

/// `tmpl_set` with `owner` as `topics[1]`, so indexers can filter by owner.
#[contractevent(topics = ["tmpl_set", "owner"], data_format = "vec")]
pub struct TemplateAssignedByOwner {
    pub owner: Address,
    pub template_id: u32,
}

#[contractevent(topics = ["init"], data_format = "vec")]
pub struct ContractInitialized {
    pub admin: Address,
    pub seq: u64,
}

#[contractevent(topics = ["adm_add"], data_format = "vec")]
pub struct AdminAdded {
    pub admin: Address,
    pub new_admin: Address,
    pub seq: u64,
}

#[contractevent(topics = ["adm_rm"], data_format = "vec")]
pub struct AdminRemoved {
    pub admin: Address,
    pub target: Address,
    pub seq: u64,
}

/// Publishes `event` and folds it into the rolling event digest:
/// `digest = sha256(prev_digest || seq_be_u64 || xdr(topics) || xdr(data))`.
/// See `docs/EVENTS.md` for the exact encoding.
fn publish<E: Event>(env: &Env, seq: u64, event: E) {
    let mut preimage = Bytes::from_array(env, &storage::get_event_digest(env).to_array());
    preimage.extend_from_array(&seq.to_be_bytes());
    preimage.append(&event.topics(env).to_xdr(env));
    preimage.append(&event.data(env).to_xdr(env));
    let digest = env.crypto().sha256(&preimage).to_bytes();
    storage::set_event_cursor(env, seq, &digest);
    event.publish(env);
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
    let seq = storage::next_event_seq(env);
    publish(
        env,
        seq,
        ContractInitialized {
            admin: admin.clone(),
            seq,
        },
    );
}

pub fn admin_added(env: &Env, admin: &Address, new_admin: &Address) {
    let seq = storage::next_event_seq(env);
    publish(
        env,
        seq,
        AdminAdded {
            admin: admin.clone(),
            new_admin: new_admin.clone(),
            seq,
        },
    );
}

pub fn admin_removed(env: &Env, admin: &Address, target: &Address) {
    let seq = storage::next_event_seq(env);
    publish(
        env,
        seq,
        AdminRemoved {
            admin: admin.clone(),
            target: target.clone(),
            seq,
        },
    );
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
    FolderClaimedByOwner {
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
    PortfolioMintedByOwner {
        owner: owner.clone(),
        template_id,
        ipfs_cid: ipfs_cid.clone(),
    }
    .publish(env);
}

pub fn portfolio_updated(env: &Env, owner: &Address, ipfs_cid: &String, previous_cid: &String) {
    PortfolioUpdated {
        owner: owner.clone(),
        ipfs_cid: ipfs_cid.clone(),
        previous_cid: previous_cid.clone(),
    }
    .publish(env);
    PortfolioUpdatedByOwner {
        owner: owner.clone(),
        ipfs_cid: ipfs_cid.clone(),
    }
    .publish(env);
}

pub fn template_registered(env: &Env, template_id: u32, metadata_uri: &String, creator: &Address) {
    let seq = storage::next_event_seq(env);
    publish(
        env,
        seq,
        TemplateRegistered {
            template_id,
            metadata_uri: metadata_uri.clone(),
            creator: creator.clone(),
            seq,
        },
    );
}

pub fn template_assigned(env: &Env, owner: &Address, template_id: u32) {
    TemplateAssigned {
        owner: owner.clone(),
        template_id,
    }
    .publish(env);
    TemplateAssignedByOwner {
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

#[contractevent(topics = ["blk_add"], data_format = "vec")]
pub struct AddressBlocked {
    pub admin: Address,
    pub address: Address,
    pub reason_code: u32,
}

#[contractevent(topics = ["blk_rm"], data_format = "vec")]
pub struct AddressUnblocked {
    pub admin: Address,
    pub address: Address,
}

pub fn address_blocked(env: &Env, admin: &Address, address: &Address, reason_code: u32) {
    AddressBlocked {
        admin: admin.clone(),
        address: address.clone(),
        reason_code,
    }
    .publish(env);
}

pub fn address_unblocked(env: &Env, admin: &Address, address: &Address) {
    AddressUnblocked {
        admin: admin.clone(),
        address: address.clone(),
    }
    .publish(env);
}

#[contractevent(topics = ["port_flag"], data_format = "vec")]
pub struct PortfolioFlagSet {
    pub admin: Address,
    pub owner: Address,
    pub flag: u32,
    pub reason_code: u32,
}

pub fn portfolio_flag_set(
    env: &Env,
    admin: &Address,
    owner: &Address,
    flag: u32,
    reason_code: u32,
) {
    PortfolioFlagSet {
        admin: admin.clone(),
        owner: owner.clone(),
        flag,
        reason_code,
    }
    .publish(env);
}

#[contractevent(topics = ["tmpl_sub"], data_format = "vec")]
pub struct TemplateSubmitted {
    pub submission_id: u32,
    pub designer: Address,
    pub metadata_uri: String,
}

#[contractevent(topics = ["tmpl_appr"], data_format = "vec")]
pub struct TemplateSubmissionApproved {
    pub admin: Address,
    pub submission_id: u32,
    pub template_id: u32,
}

#[contractevent(topics = ["tmpl_rej"], data_format = "vec")]
pub struct TemplateSubmissionRejected {
    pub admin: Address,
    pub submission_id: u32,
    pub reason_code: u32,
}

pub fn template_submitted(
    env: &Env,
    submission_id: u32,
    designer: &Address,
    metadata_uri: &String,
) {
    TemplateSubmitted {
        submission_id,
        designer: designer.clone(),
        metadata_uri: metadata_uri.clone(),
    }
    .publish(env);
}

pub fn template_submission_approved(
    env: &Env,
    admin: &Address,
    submission_id: u32,
    template_id: u32,
) {
    TemplateSubmissionApproved {
        admin: admin.clone(),
        submission_id,
        template_id,
    }
    .publish(env);
}

pub fn template_submission_rejected(
    env: &Env,
    admin: &Address,
    submission_id: u32,
    reason_code: u32,
) {
    TemplateSubmissionRejected {
        admin: admin.clone(),
        submission_id,
        reason_code,
    }
    .publish(env);
}

#[contractevent(topics = ["upgrade"], data_format = "vec")]
pub struct ContractUpgraded {
    pub admin: Address,
    pub wasm_hash: BytesN<32>,
}

pub fn contract_upgraded(env: &Env, admin: &Address, wasm_hash: &BytesN<32>) {
    ContractUpgraded {
        admin: admin.clone(),
        wasm_hash: wasm_hash.clone(),
    }
    .publish(env);
}
