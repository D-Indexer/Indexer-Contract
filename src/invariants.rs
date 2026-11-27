//! Stateful model-based tests: random operation sequences are applied to both
//! the contract and an in-memory model, and the results plus the full state are
//! compared after every step.

#![cfg(test)]
extern crate std;

use std::collections::BTreeMap;
use std::format;
use std::string::String as StdString;
use std::vec::Vec as StdVec;

use proptest::prelude::*;
use soroban_sdk::{Address, Env, String};

use crate::errors::ContractError;
use crate::types::PortfolioNFT;
use crate::{FolderContract, FolderContractClient};

const USERS: u32 = 4;
const ADMINS: u32 = 3;
const TEMPLATES: u32 = 4;
const FOLDERS: u32 = 5;
const CIDS: u32 = 3;

const CID_POOL: [&str; CIDS as usize] = [
    "bafkreif3kqdiv2uf7kt6jb2tacbtm27jsyrzbl4cfzghd3y2zjydhsb6my",
    "bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia",
    "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG",
];

const FOLDER_POOL: [&str; FOLDERS as usize] = [
    "alice.folder",
    "bob.folder",
    "carol.folder",
    "dave.folder",
    "erin.folder",
];

fn cid(env: &Env, index: u32) -> String {
    String::from_str(env, CID_POOL[index as usize % CID_POOL.len()])
}

fn folder_name(env: &Env, index: u32) -> String {
    String::from_str(env, FOLDER_POOL[index as usize % FOLDER_POOL.len()])
}

fn metadata_uri(env: &Env, index: u32) -> String {
    let cid = CID_POOL[index as usize % CID_POOL.len()];
    String::from_str(env, &format!("ipfs://{cid}/template.json"))
}

#[derive(Clone, Debug)]
enum Op {
    Claim { user: u32, folder: u32 },
    Mint { user: u32, template: u32, cid: u32 },
    Update { user: u32, cid: u32 },
    SetTemplate { user: u32, template: u32 },
    Register { admin: u32, template: u32, uri: u32 },
    AddAdmin { admin: u32, new_admin: u32 },
    RemoveAdmin { admin: u32, target: u32 },
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..USERS, 0..FOLDERS).prop_map(|(user, folder)| Op::Claim { user, folder }),
        (0..USERS, 0..TEMPLATES, 0..CIDS)
            .prop_map(|(user, template, cid)| Op::Mint { user, template, cid }),
        (0..USERS, 0..CIDS).prop_map(|(user, cid)| Op::Update { user, cid }),
        (0..USERS, 0..TEMPLATES).prop_map(|(user, template)| Op::SetTemplate { user, template }),
        (0..ADMINS, 0..TEMPLATES, 0..CIDS)
            .prop_map(|(admin, template, uri)| Op::Register { admin, template, uri }),
        (0..ADMINS, 0..ADMINS).prop_map(|(admin, new_admin)| Op::AddAdmin { admin, new_admin }),
        (0..ADMINS, 0..ADMINS).prop_map(|(admin, target)| Op::RemoveAdmin { admin, target }),
    ]
}

fn sequence_strategy() -> impl Strategy<Value = StdVec<Op>> {
    prop::collection::vec(op_strategy(), 0..50)
}

/// In-memory mirror of the contract state.
#[derive(Default)]
struct Model {
    admins: StdVec<u32>,
    folder_owner: BTreeMap<u32, u32>,
    owner_folder: BTreeMap<u32, u32>,
    templates: BTreeMap<u32, (u32, u32)>,
    template_list: StdVec<u32>,
    user_template: BTreeMap<u32, u32>,
    portfolios: BTreeMap<u32, (u32, u32)>,
}

impl Model {
    fn is_admin(&self, admin: u32) -> bool {
        self.admins.contains(&admin)
    }

    fn apply(&mut self, op: &Op) -> Result<(), ContractError> {
        match *op {
            Op::Claim { user, folder } => {
                if self.owner_folder.contains_key(&user) {
                    return Err(ContractError::FolderAlreadyClaimed);
                }
                if self.folder_owner.contains_key(&folder) {
                    return Err(ContractError::FolderNameTaken);
                }
                self.folder_owner.insert(folder, user);
                self.owner_folder.insert(user, folder);
                Ok(())
            }
            Op::Mint {
                user,
                template,
                cid,
            } => {
                if !self.owner_folder.contains_key(&user) {
                    return Err(ContractError::FolderNotFound);
                }
                if self.portfolios.contains_key(&user) {
                    return Err(ContractError::PortfolioAlreadyMinted);
                }
                if template == 0 {
                    return Err(ContractError::InvalidTemplateID);
                }
                if !self.templates.contains_key(&template) {
                    return Err(ContractError::TemplateNotFound);
                }
                self.portfolios.insert(user, (template, cid));
                self.user_template.insert(user, template);
                Ok(())
            }
            Op::Update { user, cid } => match self.portfolios.get_mut(&user) {
                Some(portfolio) => {
                    portfolio.1 = cid;
                    Ok(())
                }
                None => Err(ContractError::PortfolioNotFound),
            },
            Op::SetTemplate { user, template } => {
                if !self.owner_folder.contains_key(&user) {
                    return Err(ContractError::FolderNotFound);
                }
                if template == 0 {
                    return Err(ContractError::InvalidTemplateID);
                }
                if !self.templates.contains_key(&template) {
                    return Err(ContractError::TemplateNotFound);
                }
                self.user_template.insert(user, template);
                if let Some(portfolio) = self.portfolios.get_mut(&user) {
                    portfolio.0 = template;
                }
                Ok(())
            }
            Op::Register {
                admin,
                template,
                uri,
            } => {
                if !self.is_admin(admin) {
                    return Err(ContractError::Unauthorized);
                }
                if template == 0 {
                    return Err(ContractError::InvalidTemplateID);
                }
                if self.templates.contains_key(&template) {
                    return Err(ContractError::TemplateAlreadyExists);
                }
                self.templates.insert(template, (uri, admin));
                self.template_list.push(template);
                Ok(())
            }
            Op::AddAdmin { admin, new_admin } => {
                if !self.is_admin(admin) {
                    return Err(ContractError::Unauthorized);
                }
                if self.is_admin(new_admin) {
                    return Err(ContractError::AdminAlreadyExists);
                }
                self.admins.push(new_admin);
                Ok(())
            }
            Op::RemoveAdmin { admin, target } => {
                if !self.is_admin(admin) {
                    return Err(ContractError::Unauthorized);
                }
                match self.admins.iter().position(|a| *a == target) {
                    Some(index) => {
                        if self.admins.len() == 1 {
                            return Err(ContractError::CannotRemoveLastAdmin);
                        }
                        self.admins.remove(index);
                        Ok(())
                    }
                    None => Err(ContractError::AdminNotFound),
                }
            }
        }
    }
}

struct Harness {
    env: Env,
    client: FolderContractClient<'static>,
    users: StdVec<Address>,
    admins: StdVec<Address>,
}

impl Harness {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(FolderContract, ());
        let client = FolderContractClient::new(&env, &contract_id);
        let users = (0..USERS).map(|_| Address::generate(&env)).collect();
        let admins = (0..ADMINS).map(|_| Address::generate(&env)).collect();
        client.initialize(&admins[0]);
        for admin in admins.iter().skip(1) {
            client.add_admin(&admins[0], admin);
        }
        Harness {
            env,
            client,
            users,
            admins,
        }
    }

    fn user(&self, index: u32) -> Address {
        self.users[index as usize % self.users.len()].clone()
    }

    fn admin(&self, index: u32) -> Address {
        self.admins[index as usize % self.admins.len()].clone()
    }

    fn apply(&self, op: &Op) -> Result<(), ContractError> {
        let env = &self.env;
        let client = &self.client;
        match *op {
            Op::Claim { user, folder } => client
                .try_claim_folder(&self.user(user), &folder_name(env, folder))
                .map(|_| ())
                .map_err(|e| e.unwrap()),
            Op::Mint {
                user,
                template,
                cid: cid_index,
            } => client
                .try_mint_portfolio(&self.user(user), &template, &cid(env, cid_index))
                .map(|_| ())
                .map_err(|e| e.unwrap()),
            Op::Update { user, cid: cid_index } => client
                .try_update_portfolio(&self.user(user), &cid(env, cid_index))
                .map(|_| ())
                .map_err(|e| e.unwrap()),
            Op::SetTemplate { user, template } => client
                .try_set_user_template(&self.user(user), &template)
                .map(|_| ())
                .map_err(|e| e.unwrap()),
            Op::Register {
                admin,
                template,
                uri,
            } => client
                .try_register_template(&self.admin(admin), &template, &metadata_uri(env, uri))
                .map(|_| ())
                .map_err(|e| e.unwrap()),
            Op::AddAdmin { admin, new_admin } => client
                .try_add_admin(&self.admin(admin), &self.admin(new_admin))
                .map(|_| ())
                .map_err(|e| e.unwrap()),
            Op::RemoveAdmin { admin, target } => client
                .try_remove_admin(&self.admin(admin), &self.admin(target))
                .map(|_| ())
                .map_err(|e| e.unwrap()),
        }
    }

    /// Reads the whole contract state and compares it with the model.
    fn assert_state(&self, model: &Model) {
        let env = &self.env;
        let client = &self.client;

        let admins: StdVec<Address> = client.get_admins().iter().collect();
        let expected_admins: StdVec<Address> =
            model.admins.iter().map(|a| self.admin(*a)).collect();
        assert_eq!(admins, expected_admins, "admin list mismatch");

        for folder in 0..FOLDERS {
            let name = folder_name(env, folder);
            let actual = client.try_get_folder(&name).ok();
            let expected = model.folder_owner.get(&folder).map(|u| self.user(*u));
            assert_eq!(actual, expected, "folder owner mismatch for {folder}");
        }

        for user in 0..USERS {
            let address = self.user(user);
            let actual = client.try_get_identity(&address).ok();
            let expected = model.owner_folder.get(&user).map(|f| folder_name(env, *f));
            assert_eq!(actual, expected, "identity mismatch for user {user}");

            let actual = client.try_get_portfolio(&address).ok();
            let expected = model.portfolios.get(&user).map(|(template, cid_index)| {
                PortfolioNFT {
                    owner: address.clone(),
                    template_id: *template,
                    ipfs_cid: cid(env, *cid_index),
                    minted_at: 0,
                }
            });
            match (actual, expected) {
                (Some(actual), Some(expected)) => {
                    assert_eq!(actual.owner, expected.owner, "portfolio owner mismatch");
                    assert_eq!(
                        actual.template_id, expected.template_id,
                        "portfolio template mismatch for user {user}"
                    );
                    assert_eq!(
                        actual.ipfs_cid, expected.ipfs_cid,
                        "portfolio cid mismatch for user {user}"
                    );
                }
                (None, None) => {}
                (actual, expected) => {
                    panic!("portfolio presence mismatch for user {user}: {actual:?} vs {expected:?}")
                }
            }

            let actual = client.get_user_template(&address);
            assert_eq!(
                actual,
                model.user_template.get(&user).copied(),
                "user template mismatch for user {user}"
            );
        }

        for template in 0..TEMPLATES {
            let actual = client.try_get_template(&template).ok();
            let expected = model.templates.get(&template).map(|(uri, admin)| {
                (metadata_uri(env, *uri), self.admin(*admin))
            });
            match (actual, expected) {
                (Some(actual), Some((uri, creator))) => {
                    assert_eq!(actual.template_id, template, "template id mismatch");
                    assert_eq!(actual.metadata_uri, uri, "template uri mismatch");
                    assert_eq!(actual.creator, creator, "template creator mismatch");
                }
                (None, None) => {}
                (actual, expected) => {
                    panic!("template presence mismatch for {template}: {actual:?} vs {expected:?}")
                }
            }
        }

        let actual: StdVec<u32> = client.list_templates().iter().collect();
        assert_eq!(actual, model.template_list, "template list mismatch");
    }
}

fn run_sequence(ops: &[Op]) {
    let harness = Harness::new();
    let mut model = Model::default();
    model.admins = (0..ADMINS).collect();

    for (step, op) in ops.iter().enumerate() {
        let expected = model.apply(op);
        let actual = harness.apply(op);
        assert_eq!(
            actual, expected,
            "step {step} ({op:?}) returned a different result"
        );
        harness.assert_state(&model);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        max_shrink_iters: 4096,
        ..ProptestConfig::default()
    })]

    #[test]
    fn model_matches_contract(ops in sequence_strategy()) {
        run_sequence(&ops);
    }
}

/// Historical regression: a minted portfolio follows the owner's template
/// selection, and a second mint is rejected without touching state.
#[test]
fn regression_template_switch_after_mint() {
    let ops = std::vec![
        Op::Register {
            admin: 0,
            template: 1,
            uri: 0,
        },
        Op::Register {
            admin: 0,
            template: 2,
            uri: 1,
        },
        Op::Claim {
            user: 0,
            folder: 0,
        },
        Op::Mint {
            user: 0,
            template: 1,
            cid: 0,
        },
        Op::SetTemplate {
            user: 0,
            template: 2,
        },
        Op::Mint {
            user: 0,
            template: 2,
            cid: 1,
        },
        Op::Update {
            user: 0,
            cid: 2,
        },
        Op::RemoveAdmin {
            admin: 0,
            target: 0,
        },
    ];
    run_sequence(&ops);
}
