use soroban_sdk::{contractevent, xdr::ToXdr, Address, Bytes, Env, Event, String};

use crate::storage;

#[contractevent(topics = ["folder_cl"], data_format = "vec")]
pub struct FolderClaimed {
    pub owner: Address,
    pub folder_name: String,
    pub seq: u64,
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

#[contractevent(topics = ["port_upd"], data_format = "vec")]
pub struct PortfolioUpdated {
    pub owner: Address,
    pub ipfs_cid: String,
    /// Appended field: the portfolio slot (0 for the legacy portfolio).
    pub slot: u32,
    pub seq: u64,
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

pub fn folder_claimed(env: &Env, owner: &Address, folder_name: &String) {
    let seq = storage::next_event_seq(env);
    publish(
        env,
        seq,
        FolderClaimed {
            owner: owner.clone(),
            folder_name: folder_name.clone(),
            seq,
        },
    );
}

pub fn portfolio_minted(
    env: &Env,
    owner: &Address,
    template_id: u32,
    ipfs_cid: &String,
    slot: u32,
) {
    let seq = storage::next_event_seq(env);
    publish(
        env,
        seq,
        PortfolioMinted {
            owner: owner.clone(),
            template_id,
            ipfs_cid: ipfs_cid.clone(),
            slot,
            seq,
        },
    );
}

pub fn portfolio_updated(env: &Env, owner: &Address, ipfs_cid: &String, slot: u32) {
    let seq = storage::next_event_seq(env);
    publish(
        env,
        seq,
        PortfolioUpdated {
            owner: owner.clone(),
            ipfs_cid: ipfs_cid.clone(),
            slot,
            seq,
        },
    );
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
    let seq = storage::next_event_seq(env);
    publish(
        env,
        seq,
        TemplateAssigned {
            owner: owner.clone(),
            template_id,
            seq,
        },
    );
}
