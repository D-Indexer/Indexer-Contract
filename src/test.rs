#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::errors::ContractError;
use crate::{FolderContract, FolderContractClient};

struct TestEnv {
    env: Env,
    admin: Address,
    contract_id: Address,
}

impl TestEnv {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(FolderContract, ());
        let admin = Address::generate(&env);
        FolderContractClient::new(&env, &contract_id).initialize(&admin);
        TestEnv {
            env,
            admin,
            contract_id,
        }
    }

    fn client(&self) -> FolderContractClient<'_> {
        FolderContractClient::new(&self.env, &self.contract_id)
    }
}

fn uninitialized_env() -> TestEnv {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(FolderContract, ());
    let admin = Address::generate(&env);
    TestEnv {
        env,
        admin,
        contract_id,
    }
}

fn expect_contract_error<T>(
    result: Result<T, Result<ContractError, soroban_sdk::InvokeError>>,
    expected: ContractError,
) {
    assert_eq!(result.err(), Some(Ok(expected)));
}

#[test]
fn test_initialize() {
    let t = TestEnv::new();
    let res = t.client().try_initialize(&t.admin);
    expect_contract_error(res, ContractError::AlreadyInitialized);
}

#[test]
fn test_claim_requires_initialization() {
    let t = uninitialized_env();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "init.folder");

    let res = t.client().try_claim_folder(&user, &name);
    expect_contract_error(res, ContractError::NotInitialized);
}

#[test]
fn test_template_registration_requires_initialization() {
    let t = uninitialized_env();
    let uri = String::from_str(&t.env, "ipfs://bafybeig_template");

    let res = t.client().try_register_template(&t.admin, &1u32, &uri);
    expect_contract_error(res, ContractError::NotInitialized);
}

#[test]
fn test_mint_requires_initialization() {
    let t = uninitialized_env();
    let user = Address::generate(&t.env);
    let cid = String::from_str(&t.env, "bafybeig_init");

    let res = t.client().try_mint_portfolio(&user, &1u32, &cid);
    expect_contract_error(res, ContractError::NotInitialized);
}

#[test]
fn test_claim_folder_and_resolve() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "alice.folder");

    t.client().claim_folder(&user, &name);
    assert_eq!(t.client().get_folder(&name), user);
    assert_eq!(t.client().get_identity(&user), name);
}

#[test]
fn test_claim_folder_collision() {
    let t = TestEnv::new();
    let user1 = Address::generate(&t.env);
    let user2 = Address::generate(&t.env);
    let name = String::from_str(&t.env, "bob.folder");

    t.client().claim_folder(&user1, &name);
    let res = t.client().try_claim_folder(&user2, &name);
    expect_contract_error(res, ContractError::FolderNameTaken);
}

#[test]
fn test_owner_cannot_claim_second_folder() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let first = String::from_str(&t.env, "first.folder");
    let second = String::from_str(&t.env, "second.folder");

    t.client().claim_folder(&user, &first);
    let res = t.client().try_claim_folder(&user, &second);
    expect_contract_error(res, ContractError::FolderAlreadyClaimed);
}

#[test]
fn test_short_folder_name_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "ab");

    let res = t.client().try_claim_folder(&user, &name);
    expect_contract_error(res, ContractError::InvalidFolderName);
}

#[test]
fn test_long_folder_name_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(
        &t.env,
        "this-folder-name-is-intentionally-longer-than-sixty-four-characters.folder",
    );

    let res = t.client().try_claim_folder(&user, &name);
    expect_contract_error(res, ContractError::InvalidFolderName);
}

#[test]
fn test_get_missing_folder_rejected() {
    let t = TestEnv::new();
    let name = String::from_str(&t.env, "missing.folder");

    let res = t.client().try_get_folder(&name);
    expect_contract_error(res, ContractError::FolderNotFound);
}

#[test]
fn test_get_missing_identity_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);

    let res = t.client().try_get_identity(&user);
    expect_contract_error(res, ContractError::FolderNotFound);
}

#[test]
fn test_register_and_get_template() {
    let t = TestEnv::new();
    let uri = String::from_str(&t.env, "ipfs://bafybeig_template");

    t.client().register_template(&t.admin, &1u32, &uri);
    let tmpl = t.client().get_template(&1u32);
    assert_eq!(tmpl.metadata_uri, uri);
    assert_eq!(tmpl.template_id, 1u32);
}

#[test]
fn test_get_missing_template_rejected() {
    let t = TestEnv::new();

    let res = t.client().try_get_template(&404u32);
    expect_contract_error(res, ContractError::TemplateNotFound);
}

#[test]
fn test_duplicate_template_rejected() {
    let t = TestEnv::new();
    let uri = String::from_str(&t.env, "ipfs://bafybeig_template");

    t.client().register_template(&t.admin, &1u32, &uri);
    let res = t.client().try_register_template(&t.admin, &1u32, &uri);
    expect_contract_error(res, ContractError::TemplateAlreadyExists);
}

#[test]
fn test_short_template_metadata_rejected() {
    let t = TestEnv::new();
    let uri = String::from_str(&t.env, "ipfs://x");

    let res = t.client().try_register_template(&t.admin, &1u32, &uri);
    expect_contract_error(res, ContractError::InvalidMetadataURI);
}

#[test]
fn test_long_template_metadata_rejected() {
    let t = TestEnv::new();
    let uri = String::from_str(
        &t.env,
        "ipfs://bafybeig_template_metadata_uri_that_is_intentionally_long_for_contract_validation_bafybeig_template_metadata_uri_that_is_intentionally_long_for_contract_validation_bafybeig_template_metadata_uri_that_is_intentionally_long_for_contract_validation_bafybeig_template_metadata_uri_that_is_intentionally_long_for_contract_validation",
    );

    let res = t.client().try_register_template(&t.admin, &1u32, &uri);
    expect_contract_error(res, ContractError::InvalidMetadataURI);
}

#[test]
fn test_list_templates() {
    let t = TestEnv::new();
    let uri = String::from_str(&t.env, "ipfs://tmpl");
    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().register_template(&t.admin, &2u32, &uri);
    assert_eq!(t.client().list_templates().len(), 2);
}

#[test]
fn test_mint_and_get_portfolio() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(&t.env, "ipfs://tmpl");
    let cid = String::from_str(&t.env, "bafybeig_portfolio");
    let name = String::from_str(&t.env, "user.folder");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().claim_folder(&user, &name);
    t.client().mint_portfolio(&user, &1u32, &cid);

    let nft = t.client().get_portfolio(&user);
    assert_eq!(nft.owner, user);
    assert_eq!(nft.ipfs_cid, cid);
    assert_eq!(nft.template_id, 1u32);
}

#[test]
fn test_double_mint_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(&t.env, "ipfs://tmpl");
    let cid = String::from_str(&t.env, "bafybeig_v1");
    let name = String::from_str(&t.env, "double.folder");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().claim_folder(&user, &name);
    t.client().mint_portfolio(&user, &1u32, &cid);
    let res = t.client().try_mint_portfolio(&user, &1u32, &cid);
    expect_contract_error(res, ContractError::PortfolioAlreadyMinted);
}

#[test]
fn test_update_portfolio() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(&t.env, "ipfs://tmpl");
    let cid1 = String::from_str(&t.env, "bafybeig_v1");
    let cid2 = String::from_str(&t.env, "bafybeig_v2");
    let name = String::from_str(&t.env, "update.folder");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().claim_folder(&user, &name);
    t.client().mint_portfolio(&user, &1u32, &cid1);
    t.client().update_portfolio(&user, &cid2);

    assert_eq!(t.client().get_portfolio(&user).ipfs_cid, cid2);
}

#[test]
fn test_update_portfolio_invalid_cid_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(&t.env, "ipfs://tmpl");
    let cid = String::from_str(&t.env, "bafybeig_v1");
    let invalid = String::from_str(&t.env, "short");
    let name = String::from_str(&t.env, "bad-update.folder");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().claim_folder(&user, &name);
    t.client().mint_portfolio(&user, &1u32, &cid);

    let res = t.client().try_update_portfolio(&user, &invalid);
    expect_contract_error(res, ContractError::InvalidCID);
}

#[test]
fn test_verify_portfolio() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(&t.env, "ipfs://tmpl");
    let cid = String::from_str(&t.env, "bafybeig_verify");
    let name = String::from_str(&t.env, "verify.folder");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().claim_folder(&user, &name);
    t.client().mint_portfolio(&user, &1u32, &cid);
    assert!(t.client().verify_portfolio(&user));
}

#[test]
fn test_get_missing_portfolio_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);

    let res = t.client().try_get_portfolio(&user);
    expect_contract_error(res, ContractError::PortfolioNotFound);
}

#[test]
fn test_add_and_remove_admin() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);
    let uri = String::from_str(&t.env, "ipfs://tmpl");

    t.client().add_admin(&t.admin, &new_admin);
    t.client().register_template(&new_admin, &1u32, &uri);

    t.client().remove_admin(&t.admin, &new_admin);
    let res = t.client().try_register_template(&new_admin, &2u32, &uri);
    expect_contract_error(res, ContractError::Unauthorized);
}

#[test]
fn test_duplicate_admin_add_rejected() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);

    t.client().add_admin(&t.admin, &new_admin);
    let res = t.client().try_add_admin(&t.admin, &new_admin);
    expect_contract_error(res, ContractError::AdminAlreadyExists);
}

#[test]
fn test_cannot_remove_last_admin() {
    let t = TestEnv::new();
    let res = t.client().try_remove_admin(&t.admin, &t.admin);
    expect_contract_error(res, ContractError::CannotRemoveLastAdmin);
}

#[test]
fn test_remove_unknown_admin_rejected() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);
    let unknown = Address::generate(&t.env);

    t.client().add_admin(&t.admin, &new_admin);
    let res = t.client().try_remove_admin(&t.admin, &unknown);
    expect_contract_error(res, ContractError::AdminNotFound);
}

#[test]
fn test_unauthorized_template_registration() {
    let t = TestEnv::new();
    let rando = Address::generate(&t.env);
    let uri = String::from_str(&t.env, "ipfs://tmpl");
    let res = t.client().try_register_template(&rando, &1u32, &uri);
    expect_contract_error(res, ContractError::Unauthorized);
}

#[test]
fn test_mint_without_folder_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(&t.env, "ipfs://tmpl");
    let cid = String::from_str(&t.env, "bafybeig_nofolder");

    t.client().register_template(&t.admin, &1u32, &uri);
    // user has not claimed a folder — must fail
    let res = t.client().try_mint_portfolio(&user, &1u32, &cid);
    expect_contract_error(res, ContractError::FolderNotFound);
}

#[test]
fn test_mint_unknown_template_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let cid = String::from_str(&t.env, "bafybeig_unknown_template");
    let name = String::from_str(&t.env, "unknown-template.folder");

    t.client().claim_folder(&user, &name);
    let res = t.client().try_mint_portfolio(&user, &404u32, &cid);
    expect_contract_error(res, ContractError::TemplateNotFound);
}

#[test]
fn test_short_ipfs_cid_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(&t.env, "ipfs://tmpl");
    let cid = String::from_str(&t.env, "short");
    let name = String::from_str(&t.env, "cid.folder");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().claim_folder(&user, &name);

    let res = t.client().try_mint_portfolio(&user, &1u32, &cid);
    expect_contract_error(res, ContractError::InvalidCID);
}

#[test]
fn test_long_ipfs_cid_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(&t.env, "ipfs://tmpl");
    let cid = String::from_str(
        &t.env,
        "bafybeig_portfolio_cid_that_is_intentionally_long_for_contract_validation_bafybeig_portfolio_cid_that_is_intentionally_long_for_contract_validation",
    );
    let name = String::from_str(&t.env, "long-cid.folder");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().claim_folder(&user, &name);

    let res = t.client().try_mint_portfolio(&user, &1u32, &cid);
    expect_contract_error(res, ContractError::InvalidCID);
}

#[test]
fn test_get_user_template() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(&t.env, "ipfs://tmpl");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().set_user_template(&user, &1u32);
    assert_eq!(t.client().get_user_template(&user), Some(1u32));
}

#[test]
fn test_set_unknown_user_template_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);

    let res = t.client().try_set_user_template(&user, &404u32);
    expect_contract_error(res, ContractError::TemplateNotFound);
}
