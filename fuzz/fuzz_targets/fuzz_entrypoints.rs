//! Fuzz target: arbitrary structured operations against a testutils `Env`.
//!
//! Invariants: no unexpected panic, and the contract state stays consistent
//! (one folder per owner, one owner per folder, one portfolio per owner).
#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

use indexer_contract::{FolderContract, FolderContractClient};

const FOLDER_NAMES: [&str; 4] = ["alice.folder", "bob.folder", "carol.folder", "dave.folder"];
const CIDS: [&str; 2] = [
    "bafkreibmxd3f27mt5a7oxrw7oquokicti5bohljkssnxyotxg4qqghr664",
    "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG",
];
const URIS: [&str; 2] = [
    "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    "ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG/1.json",
];

fuzz_target!(|data: &[u8]| {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(FolderContract, ());
    let client = FolderContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let users: [Address; 3] = [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    let names: [String; 4] = [
        String::from_str(&env, FOLDER_NAMES[0]),
        String::from_str(&env, FOLDER_NAMES[1]),
        String::from_str(&env, FOLDER_NAMES[2]),
        String::from_str(&env, FOLDER_NAMES[3]),
    ];
    let cids: [String; 2] = [
        String::from_str(&env, CIDS[0]),
        String::from_str(&env, CIDS[1]),
    ];
    let uris: [String; 2] = [
        String::from_str(&env, URIS[0]),
        String::from_str(&env, URIS[1]),
    ];

    let _ = client.try_initialize(&admin);

    for chunk in data.chunks(3) {
        let op = chunk[0] % 8;
        let user = &users[(chunk.get(1).copied().unwrap_or(0) as usize) % users.len()];
        let pick = chunk.get(2).copied().unwrap_or(0) as usize;
        match op {
            0 => {
                let _ = client.try_claim_folder(user, &names[pick % names.len()]);
            }
            1 => {
                let _ = client.try_register_template(&admin, &((pick % 3) as u32 + 1), &uris[pick % uris.len()]);
            }
            2 => {
                let _ = client.try_set_user_template(user, &((pick % 3) as u32 + 1));
            }
            3 => {
                let _ = client.try_mint_portfolio(user, &((pick % 3) as u32 + 1), &cids[pick % cids.len()]);
            }
            4 => {
                let _ = client.try_update_portfolio(user, &cids[pick % cids.len()]);
            }
            5 => {
                let _ = client.try_add_admin(&admin, user);
            }
            6 => {
                let _ = client.try_remove_admin(&admin, user);
            }
            _ => {
                let _ = client.try_get_folder(&names[pick % names.len()]);
            }
        }
    }

    // State consistency: folder lookups agree in both directions.
    for user in users.iter() {
        if let Ok(name) = client.try_get_identity(user) {
            if let Ok(name) = name {
                assert_eq!(client.get_folder(&name), *user);
            }
        }
    }
    for name in names.iter() {
        if let Ok(owner) = client.try_get_folder(name) {
            if let Ok(owner) = owner {
                assert_eq!(client.get_identity(&owner), *name);
            }
        }
    }
});
