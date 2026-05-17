use soroban_sdk::{symbol_short, Address, Env, String};

pub fn folder_claimed(env: &Env, owner: &Address, folder_name: &String) {
    env.events()
        .publish((symbol_short!("folder_cl"),), (owner.clone(), folder_name.clone()));
}

pub fn portfolio_minted(env: &Env, owner: &Address, template_id: u32, ipfs_cid: &String) {
    env.events()
        .publish((symbol_short!("port_mint"),), (owner.clone(), template_id, ipfs_cid.clone()));
}

pub fn portfolio_updated(env: &Env, owner: &Address, ipfs_cid: &String) {
    env.events()
        .publish((symbol_short!("port_upd"),), (owner.clone(), ipfs_cid.clone()));
}

pub fn template_registered(env: &Env, template_id: u32, metadata_uri: &String) {
    env.events()
        .publish((symbol_short!("tmpl_reg"),), (template_id, metadata_uri.clone()));
}

pub fn template_assigned(env: &Env, owner: &Address, template_id: u32) {
    env.events()
        .publish((symbol_short!("tmpl_set"),), (owner.clone(), template_id));
}
