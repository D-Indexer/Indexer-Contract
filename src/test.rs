#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::{
        storage::{Instance as _, Persistent as _},
        Address as _, AuthorizedFunction, Events as _, Ledger as _,
    },
    Address, Env, Event as _, IntoVal, String, Val, Vec,
};

use crate::constants::{
    ADMIN_TIMELOCK_LEDGERS, DAY_IN_LEDGERS, INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD,
    MAX_ADMINS, MAX_TEMPLATES, PERSISTENT_BUMP_AMOUNT, PERSISTENT_LIFETIME_THRESHOLD,
};
use crate::errors::ContractError;
use crate::events::{
    AdminAdded, AdminChangeExecuted, AdminRemoved, ContractInitialized, TemplateRegistered,
};
use crate::types::{AdminOp, DataKey};
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

    /// Whether `key` is live in this contract's persistent storage.
    fn has_persistent(&self, key: &DataKey) -> bool {
        self.env.as_contract(&self.contract_id, || {
            self.env.storage().persistent().has(key)
        })
    }

    /// Writes `key` directly into this contract's persistent storage, as a
    /// restore of an archived entry would.
    fn set_persistent<V: IntoVal<Env, Val>>(&self, key: &DataKey, value: &V) {
        self.env.as_contract(&self.contract_id, || {
            self.env.storage().persistent().set(key, value)
        })
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

type TryResult = Result<
    Result<(), soroban_sdk::ConversionError>,
    Result<ContractError, soroban_sdk::InvokeError>,
>;

/// Queues an admin change as `admin`, waits out the timelock and executes it.
/// Admin membership only changes through the timelocked queue.
fn try_admin_change(t: &TestEnv, admin: &Address, op: AdminOp, target: &Address) -> TryResult {
    let op_id = match t.client().try_queue_admin_change(admin, &op, target) {
        Ok(Ok(id)) => id,
        Ok(Err(e)) => return Ok(Err(e)),
        Err(e) => return Err(e),
    };
    t.env
        .ledger()
        .set_sequence_number(t.env.ledger().sequence() + ADMIN_TIMELOCK_LEDGERS);
    t.client().try_execute_admin_change(admin, &op_id)
}

fn try_add_admin(t: &TestEnv, admin: &Address, new_admin: &Address) -> TryResult {
    try_admin_change(t, admin, AdminOp::Add, new_admin)
}

fn try_remove_admin(t: &TestEnv, admin: &Address, target: &Address) -> TryResult {
    try_admin_change(t, admin, AdminOp::Remove, target)
}

fn add_admin(t: &TestEnv, admin: &Address, new_admin: &Address) {
    try_add_admin(t, admin, new_admin).unwrap().unwrap();
}

fn remove_admin(t: &TestEnv, admin: &Address, target: &Address) {
    try_remove_admin(t, admin, target).unwrap().unwrap();
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
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia/template.json",
    );

    let res = t.client().try_register_template(&t.admin, &1u32, &uri);
    expect_contract_error(res, ContractError::NotInitialized);
}

#[test]
fn test_mint_requires_initialization() {
    let t = uninitialized_env();
    let user = Address::generate(&t.env);
    let cid = String::from_str(
        &t.env,
        "bafkreif3kqdiv2uf7kt6jb2tacbtm27jsyrzbl4cfzghd3y2zjydhsb6my",
    );

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
fn test_invalid_folder_characters_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "Bad_Name.folder");

    let res = t.client().try_claim_folder(&user, &name);
    expect_contract_error(res, ContractError::InvalidFolderName);
}

#[test]
fn test_folder_suffix_required() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "alice");

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

// ── Relayed submission (fee source != owner) ─────────────────────────────────

/// Builds the argument vector the contract sees for `claim_folder(owner, name)`.
fn claim_folder_args(env: &Env, owner: &Address, name: &String) -> Vec<Val> {
    (owner.clone(), name.clone()).into_val(env)
}

/// Builds the argument vector the contract sees for `mint_portfolio(owner, id, cid)`.
fn mint_portfolio_args(env: &Env, owner: &Address, template_id: u32, cid: &String) -> Vec<Val> {
    (owner.clone(), template_id, cid.clone()).into_val(env)
}

/// Authorizes `owner` for a single contract invocation, as the owner's wallet would
/// when signing a SorobanAuthorizationEntry for a relayer to submit.
///
/// `live_until_ledger` is the ledger sequence the signature stays valid through;
/// the test ledger is moved past it to model an expired entry.
fn authorize_owner(
    t: &TestEnv,
    owner: &Address,
    fn_name: &str,
    args: Vec<Val>,
    live_until_ledger: u32,
) {
    let mut info = t.env.ledger().get();
    info.base_reserve = 0;
    info.min_temp_entry_ttl = 1;
    info.min_persistent_entry_ttl = 1;
    info.max_entry_ttl = live_until_ledger;
    t.env.ledger().set(info);
    t.env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: owner,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &t.contract_id,
            fn_name,
            args,
            sub_invokes: &[],
        },
    }]);
}

#[test]
fn test_relayed_claim_folder_with_owner_auth_entry() {
    let t = TestEnv::new();
    let owner = Address::generate(&t.env);
    let relayer = Address::generate(&t.env);
    let name = String::from_str(&t.env, "relayed.folder");

    // Only the owner signs; the relayer pays the fee and submits.
    authorize_owner(
        &t,
        &owner,
        "claim_folder",
        claim_folder_args(&t.env, &owner, &name),
        100,
    );
    t.client().claim_folder(&owner, &name);

    assert_eq!(t.client().get_folder(&name), owner);
    assert_eq!(t.client().get_identity(&owner), name);
    // The relayer never gains ownership.
    assert_ne!(t.client().get_folder(&name), relayer);
}

#[test]
fn test_relayed_mint_portfolio_with_owner_auth_entry() {
    let t = TestEnv::new();
    let owner = Address::generate(&t.env);
    let name = String::from_str(&t.env, "relayed-mint.folder");
    let cid = String::from_str(
        &t.env,
        "bafkreif3kqdiv2uf7kt6jb2tacbtm27jsyrzbl4cfzghd3y2zjydhsb6my",
    );

    t.client()
        .register_template(&t.admin, &1u32, &template_uri(&t.env));
    t.client().claim_folder(&owner, &name);

    authorize_owner(
        &t,
        &owner,
        "mint_portfolio",
        mint_portfolio_args(&t.env, &owner, 1u32, &cid),
        100,
    );
    t.client().mint_portfolio(&owner, &1u32, &cid);

    let nft = t.client().get_portfolio(&owner);
    assert_eq!(nft.owner, owner);
    assert_eq!(nft.template_id, 1u32);
    assert_eq!(nft.ipfs_cid, cid);
}

#[test]
fn test_relayed_claim_folder_with_expired_auth_entry_fails() {
    let t = TestEnv::new();
    let owner = Address::generate(&t.env);
    let name = String::from_str(&t.env, "expired.folder");

    // Auth entry signed for a ledger window that has already passed.
    authorize_owner(
        &t,
        &owner,
        "claim_folder",
        claim_folder_args(&t.env, &owner, &name),
        10,
    );
    t.env.ledger().set_sequence_number(11);

    let res = t.client().try_claim_folder(&owner, &name);
    assert!(res.is_err(), "expired auth entry must not authorize");
    assert!(t.client().try_get_folder(&name).is_err());
}

#[test]
fn test_relayed_mint_portfolio_with_expired_auth_entry_fails() {
    let t = TestEnv::new();
    let owner = Address::generate(&t.env);
    let name = String::from_str(&t.env, "expired-mint.folder");
    let cid = String::from_str(
        &t.env,
        "bafkreif3kqdiv2uf7kt6jb2tacbtm27jsyrzbl4cfzghd3y2zjydhsb6my",
    );

    t.client().claim_folder(&owner, &name);

    authorize_owner(
        &t,
        &owner,
        "mint_portfolio",
        mint_portfolio_args(&t.env, &owner, 1u32, &cid),
        10,
    );
    t.env.ledger().set_sequence_number(11);

    let res = t.client().try_mint_portfolio(&owner, &1u32, &cid);
    assert!(res.is_err(), "expired auth entry must not authorize");
    assert!(t.client().try_get_portfolio(&owner).is_err());
}

#[test]
fn test_relayed_claim_folder_with_wrong_args_auth_entry_fails() {
    let t = TestEnv::new();
    let owner = Address::generate(&t.env);
    let signed_name = String::from_str(&t.env, "signed.folder");
    let submitted_name = String::from_str(&t.env, "swapped.folder");

    // Owner signed for `signed.folder`; the relayer submits a different name.
    authorize_owner(
        &t,
        &owner,
        "claim_folder",
        claim_folder_args(&t.env, &owner, &signed_name),
        100,
    );

    let res = t.client().try_claim_folder(&owner, &submitted_name);
    assert!(res.is_err(), "auth entry for other args must not authorize");
    assert!(t.client().try_get_folder(&submitted_name).is_err());
}

#[test]
fn test_relayed_mint_portfolio_with_wrong_args_auth_entry_fails() {
    let t = TestEnv::new();
    let owner = Address::generate(&t.env);
    let name = String::from_str(&t.env, "wrong-args-mint.folder");
    let signed_cid = String::from_str(
        &t.env,
        "bafkreif3kqdiv2uf7kt6jb2tacbtm27jsyrzbl4cfzghd3y2zjydhsb6my",
    );
    let submitted_cid = String::from_str(
        &t.env,
        "bafkreie33ewzcjvpo3gldpvqgevtoy4oaygkqri3cwxqr6o6lsqc4jqvwu",
    );

    t.client().claim_folder(&owner, &name);

    // Owner signed for `signed_cid`; the relayer submits a different CID.
    authorize_owner(
        &t,
        &owner,
        "mint_portfolio",
        mint_portfolio_args(&t.env, &owner, 1u32, &signed_cid),
        100,
    );

    let res = t.client().try_mint_portfolio(&owner, &1u32, &submitted_cid);
    assert!(res.is_err(), "auth entry for other args must not authorize");
    assert!(t.client().try_get_portfolio(&owner).is_err());
}

#[test]
fn test_relayed_claim_folder_without_owner_auth_fails() {
    let t = TestEnv::new();
    let owner = Address::generate(&t.env);
    let relayer = Address::generate(&t.env);
    let name = String::from_str(&t.env, "no-auth.folder");

    // Only the relayer signs; the owner never authorized the invocation.
    t.env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &relayer,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &t.contract_id,
            fn_name: "claim_folder",
            args: claim_folder_args(&t.env, &owner, &name),
            sub_invokes: &[],
        },
    }]);

    let res = t.client().try_claim_folder(&owner, &name);
    assert!(
        res.is_err(),
        "relayer signature must not stand in for the owner"
    );
    assert!(t.client().try_get_folder(&name).is_err());
}

#[test]
fn test_register_and_get_template() {
    let t = TestEnv::new();
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia/template.json",
    );

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
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia/template.json",
    );

    t.client().register_template(&t.admin, &1u32, &uri);
    let res = t.client().try_register_template(&t.admin, &1u32, &uri);
    expect_contract_error(res, ContractError::TemplateAlreadyExists);
    assert_eq!(t.client().list_templates().len(), 1);
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
        "ipfs://bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    );

    let res = t.client().try_register_template(&t.admin, &1u32, &uri);
    expect_contract_error(res, ContractError::InvalidMetadataURI);
}

#[test]
fn test_template_metadata_ipfs_scheme_required() {
    let t = TestEnv::new();
    let uri = String::from_str(
        &t.env,
        "https://bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia",
    );

    let res = t.client().try_register_template(&t.admin, &1u32, &uri);
    expect_contract_error(res, ContractError::InvalidMetadataURI);
}

#[test]
fn test_template_id_zero_rejected() {
    let t = TestEnv::new();
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia/template.json",
    );

    let res = t.client().try_register_template(&t.admin, &0u32, &uri);
    expect_contract_error(res, ContractError::InvalidTemplateID);
}

#[test]
fn test_list_templates() {
    let t = TestEnv::new();
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().register_template(&t.admin, &2u32, &uri);
    assert_eq!(t.client().list_templates().len(), 2);
}

#[test]
fn test_mint_and_get_portfolio() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    let cid = String::from_str(
        &t.env,
        "bafkreiaonkhaxbe63gygjrnclypokwjpij7d5ooskdsca2ooiykh2ahi2q",
    );
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
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    let cid = String::from_str(
        &t.env,
        "bafkreib37qtjlfhpmsjcr2nhjovqb4cc57er2wwmn67oggryf2anii4i7y",
    );
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
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    let cid1 = String::from_str(
        &t.env,
        "bafkreib37qtjlfhpmsjcr2nhjovqb4cc57er2wwmn67oggryf2anii4i7y",
    );
    let cid2 = String::from_str(
        &t.env,
        "bafkreih3atolnfyojq6rq466kh6vuugxxndlgobrcnqcmzodkdwebnpzsa",
    );
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
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    let cid = String::from_str(
        &t.env,
        "bafkreib37qtjlfhpmsjcr2nhjovqb4cc57er2wwmn67oggryf2anii4i7y",
    );
    let invalid = String::from_str(&t.env, "short");
    let name = String::from_str(&t.env, "bad-update.folder");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().claim_folder(&user, &name);
    t.client().mint_portfolio(&user, &1u32, &cid);

    let res = t.client().try_update_portfolio(&user, &invalid);
    expect_contract_error(res, ContractError::InvalidCID);
}

#[test]
fn test_update_missing_portfolio_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let cid = String::from_str(
        &t.env,
        "bafkreic7cazv5ta3crsj75lxe76qj2ewoigi3k6t4e4vntzvyr7kvpkjyy",
    );

    let res = t.client().try_update_portfolio(&user, &cid);
    expect_contract_error(res, ContractError::PortfolioNotFound);
}

#[test]
fn test_verify_portfolio() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    let cid = String::from_str(
        &t.env,
        "bafkreifbfxj2p7jsaosff2zu3enjxyqfnhk6gn5dhbbuobuisxah6pqmli",
    );
    let name = String::from_str(&t.env, "verify.folder");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().claim_folder(&user, &name);
    t.client().mint_portfolio(&user, &1u32, &cid);
    assert!(t.client().verify_portfolio(&user));
}

#[test]
fn test_verify_missing_portfolio_returns_false() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);

    assert!(!t.client().verify_portfolio(&user));
}

#[test]
fn test_get_missing_portfolio_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);

    let res = t.client().try_get_portfolio(&user);
    expect_contract_error(res, ContractError::PortfolioNotFound);
}

// ── Archival and restore behaviour of persistent entries ─────────────────────

/// Advances the ledger far enough that every persistent entry written so far
/// has expired, then asserts the entry is gone from the live ledger.
fn archive_persistent(t: &TestEnv, key: &DataKey) {
    let live_until = t.env.ledger().sequence() + t.persistent_ttl(key);
    t.env.ledger().set_sequence_number(live_until + 1);
    assert!(
        !t.has_persistent(key),
        "persistent entry should be archived"
    );
}

fn template_uri(env: &Env) -> String {
    String::from_str(
        env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    )
}

fn portfolio_cid(env: &Env) -> String {
    String::from_str(
        env,
        "bafkreib37qtjlfhpmsjcr2nhjovqb4cc57er2wwmn67oggryf2anii4i7y",
    )
}

// The host clamps `extend_ttl` to `max_entry_ttl`; staying below it keeps
// the requested lifetime exact.
const _: () = assert!(PERSISTENT_BUMP_AMOUNT < 3_110_400);
const _: () = assert!(PERSISTENT_BUMP_AMOUNT > PERSISTENT_LIFETIME_THRESHOLD);

#[test]
#[ignore = "#182: the test host keeps expired persistent entries readable, so archival cannot be observed this way"]
fn test_folder_owner_archived_after_ttl() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "archive.folder");

    t.client().claim_folder(&user, &name);
    let key = DataKey::FolderOwner(name.clone());
    assert!(t.has_persistent(&key));

    archive_persistent(&t, &key);

    // The archived entry is not silently recreated: reads report it missing.
    let res = t.client().try_get_folder(&name);
    expect_contract_error(res, ContractError::FolderNotFound);
    assert!(!t.has_persistent(&key));
}

#[test]
#[ignore = "#182: the test host keeps expired persistent entries readable, so archival cannot be observed this way"]
fn test_folder_owner_restore_prevents_reclaim() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let other = Address::generate(&t.env);
    let name = String::from_str(&t.env, "restore.folder");

    t.client().claim_folder(&user, &name);
    let key = DataKey::FolderOwner(name.clone());
    archive_persistent(&t, &key);

    // Restoring the entry (as `stellar contract restore` does) brings the
    // claim back, so the name is still taken.
    t.set_persistent(&key, &user);
    assert_eq!(t.client().get_folder(&name), user);

    let res = t.client().try_claim_folder(&other, &name);
    expect_contract_error(res, ContractError::FolderNameTaken);
}

#[test]
#[ignore = "#182: the test host keeps expired persistent entries readable, so archival cannot be observed this way"]
fn test_owner_folder_archived_after_ttl() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "identity.folder");

    t.client().claim_folder(&user, &name);
    let key = DataKey::OwnerFolder(user.clone());
    assert!(t.has_persistent(&key));

    archive_persistent(&t, &key);

    let res = t.client().try_get_identity(&user);
    expect_contract_error(res, ContractError::FolderNotFound);
}

#[test]
#[ignore = "#182: the test host keeps expired persistent entries readable, so archival cannot be observed this way"]
fn test_template_archived_after_ttl() {
    let t = TestEnv::new();
    let uri = template_uri(&t.env);

    t.client().register_template(&t.admin, &1u32, &uri);
    let key = DataKey::Template(1u32);
    assert!(t.has_persistent(&key));

    archive_persistent(&t, &key);

    let res = t.client().try_get_template(&1u32);
    expect_contract_error(res, ContractError::TemplateNotFound);
}

#[test]
#[ignore = "#182: the test host keeps expired persistent entries readable, so archival cannot be observed this way"]
fn test_template_restore_keeps_registration() {
    let t = TestEnv::new();
    let uri = template_uri(&t.env);

    t.client().register_template(&t.admin, &1u32, &uri);
    let key = DataKey::Template(1u32);
    archive_persistent(&t, &key);

    t.set_persistent(
        &key,
        &crate::types::Template {
            template_id: 1u32,
            metadata_uri: uri.clone(),
            creator: t.admin.clone(),
        },
    );

    assert_eq!(t.client().get_template(&1u32).metadata_uri, uri);
    // Re-registering a restored template is still rejected.
    let res = t.client().try_register_template(&t.admin, &1u32, &uri);
    expect_contract_error(res, ContractError::TemplateAlreadyExists);
}

#[test]
#[ignore = "#182: the test host keeps expired persistent entries readable, so archival cannot be observed this way"]
fn test_template_list_archived_after_ttl() {
    let t = TestEnv::new();
    let uri = template_uri(&t.env);

    t.client().register_template(&t.admin, &1u32, &uri);
    let key = DataKey::TemplateList;
    assert!(t.has_persistent(&key));

    archive_persistent(&t, &key);

    // A missing list degrades to empty rather than trapping.
    assert_eq!(t.client().list_templates().len(), 0);
}

#[test]
#[ignore = "#182: the test host keeps expired persistent entries readable, so archival cannot be observed this way"]
fn test_portfolio_archived_after_ttl() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "nft.folder");

    t.client()
        .register_template(&t.admin, &1u32, &template_uri(&t.env));
    t.client().claim_folder(&user, &name);
    t.client()
        .mint_portfolio(&user, &1u32, &portfolio_cid(&t.env));

    let key = DataKey::Portfolio(user.clone());
    assert!(t.has_persistent(&key));

    archive_persistent(&t, &key);

    let res = t.client().try_get_portfolio(&user);
    expect_contract_error(res, ContractError::PortfolioNotFound);
    assert!(!t.client().verify_portfolio(&user));
}

#[test]
#[ignore = "#182: the test host keeps expired persistent entries readable, so archival cannot be observed this way"]
fn test_portfolio_restore_prevents_remint() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "remint.folder");
    let cid = portfolio_cid(&t.env);

    t.client()
        .register_template(&t.admin, &1u32, &template_uri(&t.env));
    t.client().claim_folder(&user, &name);
    t.client().mint_portfolio(&user, &1u32, &cid);

    let key = DataKey::Portfolio(user.clone());
    archive_persistent(&t, &key);

    t.set_persistent(
        &key,
        &crate::types::PortfolioNFT {
            owner: user.clone(),
            template_id: 1u32,
            ipfs_cid: cid.clone(),
            minted_at: 0,
        },
    );

    assert_eq!(t.client().get_portfolio(&user).ipfs_cid, cid);
    let res = t.client().try_mint_portfolio(&user, &1u32, &cid);
    expect_contract_error(res, ContractError::PortfolioAlreadyMinted);
}

#[test]
#[ignore = "#182: the test host keeps expired persistent entries readable, so archival cannot be observed this way"]
fn test_user_template_archived_after_ttl() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "assigned.folder");

    t.client()
        .register_template(&t.admin, &1u32, &template_uri(&t.env));
    t.client().claim_folder(&user, &name);
    t.client()
        .mint_portfolio(&user, &1u32, &portfolio_cid(&t.env));

    let key = DataKey::UserTemplate(user.clone());
    assert!(t.has_persistent(&key));

    archive_persistent(&t, &key);

    assert!(!t.has_persistent(&key));
}

#[test]
fn test_persistent_read_extends_ttl() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "ttl.folder");

    t.client().claim_folder(&user, &name);
    let key = DataKey::FolderOwner(name.clone());

    // Drop the entry close to expiry, then read it: the read must bump it back
    // out to the full persistent lifetime.
    t.env
        .ledger()
        .set_sequence_number(PERSISTENT_LIFETIME_THRESHOLD + 1);
    assert!(t.has_persistent(&key));

    assert_eq!(t.client().get_folder(&name), user);

    t.env
        .ledger()
        .set_sequence_number(PERSISTENT_LIFETIME_THRESHOLD + 1 + DAY_IN_LEDGERS);
    assert!(
        t.has_persistent(&key),
        "read should have extended the entry TTL"
    );
}

#[test]
fn test_add_and_remove_admin() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );

    add_admin(&t, &t.admin, &new_admin);
    t.client().register_template(&new_admin, &1u32, &uri);

    remove_admin(&t, &t.admin, &new_admin);
    let res = t.client().try_register_template(&new_admin, &2u32, &uri);
    expect_contract_error(res, ContractError::Unauthorized);
}

#[test]
fn test_duplicate_admin_add_rejected() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);

    add_admin(&t, &t.admin, &new_admin);
    let res = try_add_admin(&t, &t.admin, &new_admin);
    expect_contract_error(res, ContractError::AdminAlreadyExists);
}

#[test]
fn test_cannot_remove_last_admin() {
    let t = TestEnv::new();
    let res = try_remove_admin(&t, &t.admin, &t.admin);
    expect_contract_error(res, ContractError::CannotRemoveLastAdmin);
}

#[test]
fn test_remove_unknown_admin_rejected() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);
    let unknown = Address::generate(&t.env);

    add_admin(&t, &t.admin, &new_admin);
    let res = try_remove_admin(&t, &t.admin, &unknown);
    expect_contract_error(res, ContractError::AdminNotFound);
}

#[test]
fn test_unauthorized_template_registration() {
    let t = TestEnv::new();
    let rando = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    let res = t.client().try_register_template(&rando, &1u32, &uri);
    expect_contract_error(res, ContractError::Unauthorized);
}

#[test]
fn test_mint_without_folder_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    let cid = String::from_str(
        &t.env,
        "bafkreibmxd3f27mt5a7oxrw7oquokicti5bohljkssnxyotxg4qqghr664",
    );

    t.client().register_template(&t.admin, &1u32, &uri);
    // user has not claimed a folder — must fail
    let res = t.client().try_mint_portfolio(&user, &1u32, &cid);
    expect_contract_error(res, ContractError::FolderNotFound);
}

#[test]
fn test_mint_unknown_template_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let cid = String::from_str(
        &t.env,
        "bafkreicivmwnppyl74s6tw3357w3z7diihc73d2jph3qf26duumf6olkaq",
    );
    let name = String::from_str(&t.env, "unknown-template.folder");

    t.client().claim_folder(&user, &name);
    let res = t.client().try_mint_portfolio(&user, &404u32, &cid);
    expect_contract_error(res, ContractError::TemplateNotFound);
}

#[test]
fn test_short_ipfs_cid_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    let cid = String::from_str(&t.env, "short");
    let name = String::from_str(&t.env, "cid.folder");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().claim_folder(&user, &name);

    let res = t.client().try_mint_portfolio(&user, &1u32, &cid);
    expect_contract_error(res, ContractError::InvalidCID);
}

#[test]
fn test_invalid_ipfs_cid_prefix_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    let cid = String::from_str(&t.env, "invalidcidvalue");
    let name = String::from_str(&t.env, "bad-cid-prefix.folder");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().claim_folder(&user, &name);

    let res = t.client().try_mint_portfolio(&user, &1u32, &cid);
    expect_contract_error(res, ContractError::InvalidCID);
}

#[test]
fn test_long_ipfs_cid_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    let cid = String::from_str(
        &t.env,
        "bafkreiaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
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
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    let name = String::from_str(&t.env, "template-user.folder");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().claim_folder(&user, &name);
    t.client().set_user_template(&user, &1u32);
    assert_eq!(t.client().get_user_template(&user), Some(1u32));
}

#[test]
fn test_user_template_can_be_changed() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    let name = String::from_str(&t.env, "template-change.folder");

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().register_template(&t.admin, &2u32, &uri);
    t.client().claim_folder(&user, &name);
    t.client().set_user_template(&user, &1u32);
    t.client().set_user_template(&user, &2u32);

    assert_eq!(t.client().get_user_template(&user), Some(2u32));
}

#[test]
fn test_set_unknown_user_template_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "unknown-user-template.folder");

    t.client().claim_folder(&user, &name);
    let res = t.client().try_set_user_template(&user, &404u32);
    expect_contract_error(res, ContractError::TemplateNotFound);
}

#[test]
fn test_set_user_template_without_folder_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );

    t.client().register_template(&t.admin, &1u32, &uri);
    let res = t.client().try_set_user_template(&user, &1u32);
    expect_contract_error(res, ContractError::FolderNotFound);
}

// ── TTL policy ──────────────────────────────────────────────────────────────

/// Testnet/mainnet `max_entry_ttl` (~180 days).
const NETWORK_MAX_ENTRY_TTL: u32 = 3_110_400;
const _: () = assert!(PERSISTENT_BUMP_AMOUNT < NETWORK_MAX_ENTRY_TTL);
const _: () = assert!(INSTANCE_BUMP_AMOUNT < NETWORK_MAX_ENTRY_TTL);

impl TestEnv {
    fn instance_ttl(&self) -> u32 {
        self.env.as_contract(&self.contract_id, || {
            self.env.storage().instance().get_ttl()
        })
    }

    fn persistent_ttl(&self, key: &DataKey) -> u32 {
        self.env.as_contract(&self.contract_id, || {
            self.env.storage().persistent().get_ttl(key)
        })
    }

    fn advance_ledgers(&self, ledgers: u32) {
        self.env.ledger().with_mut(|l| {
            l.sequence_number += ledgers;
        });
    }
}

#[test]
fn test_instance_ttl_extended_on_initialize_and_writes() {
    let t = TestEnv::new();
    assert_eq!(t.instance_ttl(), INSTANCE_BUMP_AMOUNT);

    // Let the instance TTL fall below the threshold, then perform a write.
    t.advance_ledgers(40 * DAY_IN_LEDGERS);
    assert!(t.instance_ttl() < INSTANCE_LIFETIME_THRESHOLD);

    let user = Address::generate(&t.env);
    t.client()
        .claim_folder(&user, &String::from_str(&t.env, "ttl.folder"));
    assert_eq!(t.instance_ttl(), INSTANCE_BUMP_AMOUNT);
}

#[test]
fn test_persistent_ttl_fits_network_max_entry_ttl() {
    let t = TestEnv::new();
    t.env.ledger().with_mut(|l| {
        l.max_entry_ttl = NETWORK_MAX_ENTRY_TTL;
    });
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "ttl-max.folder");
    t.client().claim_folder(&user, &name);

    // The requested extension is applied exactly, not clamped by the host.
    let ttl = t.persistent_ttl(&DataKey::FolderOwner(name));
    assert_eq!(ttl, PERSISTENT_BUMP_AMOUNT);
}

#[test]
fn test_persistent_ttl_not_rebumped_above_threshold() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "ttl-read.folder");
    t.client().claim_folder(&user, &name);

    t.advance_ledgers(10 * DAY_IN_LEDGERS);
    t.client().get_folder(&name);
    // Still above the threshold, so no rent is spent re-extending it.
    assert_eq!(
        t.persistent_ttl(&DataKey::FolderOwner(name)),
        PERSISTENT_BUMP_AMOUNT - 10 * DAY_IN_LEDGERS
    );
}

// ── Template storage ────────────────────────────────────────────────────────

#[test]
fn test_templates_stored_in_persistent_storage() {
    let t = TestEnv::new();
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    t.client().register_template(&t.admin, &1u32, &uri);

    t.env.as_contract(&t.contract_id, || {
        let instance = t.env.storage().instance();
        let persistent = t.env.storage().persistent();
        assert!(!instance.has(&DataKey::Template(1)));
        assert!(!instance.has(&DataKey::TemplateList));
        assert!(persistent.has(&DataKey::Template(1)));
        assert!(persistent.has(&DataKey::TemplateList));
    });
    assert_eq!(
        t.persistent_ttl(&DataKey::Template(1)),
        PERSISTENT_BUMP_AMOUNT
    );
}

#[test]
fn test_template_registry_is_bounded() {
    let env = Env::new_with_config(soroban_sdk::testutils::EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();
    let contract_id = env.register(FolderContract, ());
    let client = FolderContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let uri = String::from_str(
        &env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );

    for id in 1..=MAX_TEMPLATES {
        client.register_template(&admin, &id, &uri);
    }
    let res = client.try_register_template(&admin, &(MAX_TEMPLATES + 1), &uri);
    expect_contract_error(res, ContractError::TemplateLimitReached);
    assert_eq!(client.list_templates().len(), MAX_TEMPLATES);
}

// ── Admin management ────────────────────────────────────────────────────────

#[test]
fn test_initialize_emits_event_and_exposes_admins() {
    let t = uninitialized_env();
    assert_eq!(t.client().get_admins().len(), 0);

    t.client().initialize(&t.admin);
    assert_eq!(
        t.env.events().all().filter_by_contract(&t.contract_id),
        [ContractInitialized {
            admin: t.admin.clone(),
        }
        .to_xdr(&t.env, &t.contract_id)]
    );
    assert_eq!(
        t.client().get_admins(),
        soroban_sdk::vec![&t.env, t.admin.clone()]
    );
}

#[test]
fn test_admin_changes_emit_events() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);

    add_admin(&t, &t.admin, &new_admin);
    assert_eq!(
        t.env.events().all().filter_by_contract(&t.contract_id),
        [
            AdminAdded {
                admin: t.admin.clone(),
                new_admin: new_admin.clone(),
            }
            .to_xdr(&t.env, &t.contract_id),
            AdminChangeExecuted {
                admin: t.admin.clone(),
                op_id: 0,
                target: new_admin.clone(),
            }
            .to_xdr(&t.env, &t.contract_id),
        ]
    );

    remove_admin(&t, &new_admin, &t.admin);
    assert_eq!(
        t.env.events().all().filter_by_contract(&t.contract_id),
        [
            AdminRemoved {
                admin: new_admin.clone(),
                target: t.admin.clone(),
            }
            .to_xdr(&t.env, &t.contract_id),
            AdminChangeExecuted {
                admin: new_admin.clone(),
                op_id: 1,
                target: t.admin.clone(),
            }
            .to_xdr(&t.env, &t.contract_id),
        ]
    );
    assert_eq!(
        t.client().get_admins(),
        soroban_sdk::vec![&t.env, new_admin]
    );
}

#[test]
fn test_admin_list_is_bounded() {
    let t = TestEnv::new();
    for _ in 1..MAX_ADMINS {
        add_admin(&t, &t.admin, &Address::generate(&t.env));
    }
    assert_eq!(t.client().get_admins().len(), MAX_ADMINS);

    let res = try_add_admin(&t, &t.admin, &Address::generate(&t.env));
    expect_contract_error(res, ContractError::TooManyAdmins);
}

#[test]
fn test_removed_admin_loses_admin_rights() {
    let t = TestEnv::new();
    let other = Address::generate(&t.env);
    add_admin(&t, &t.admin, &other);
    remove_admin(&t, &t.admin, &t.admin);

    let res = try_add_admin(&t, &t.admin, &Address::generate(&t.env));
    expect_contract_error(res, ContractError::Unauthorized);
    let res = try_remove_admin(&t, &t.admin, &other);
    expect_contract_error(res, ContractError::Unauthorized);
}

#[test]
fn test_template_registered_event_includes_creator() {
    let t = TestEnv::new();
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    t.client().register_template(&t.admin, &7u32, &uri);

    assert_eq!(
        t.env.events().all().filter_by_contract(&t.contract_id),
        [TemplateRegistered {
            template_id: 7,
            metadata_uri: uri,
            creator: t.admin.clone(),
        }
        .to_xdr(&t.env, &t.contract_id)]
    );
}

// ── Template selection consistency ──────────────────────────────────────────

#[test]
fn test_mint_sets_user_template() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    t.client().register_template(&t.admin, &3u32, &uri);
    t.client()
        .claim_folder(&user, &String::from_str(&t.env, "consistent.folder"));
    assert_eq!(t.client().get_user_template(&user), None);

    t.client().mint_portfolio(
        &user,
        &3u32,
        &String::from_str(
            &t.env,
            "bafkreibannpmoq7uysdrxrc6vd4te4x6cywyugvk23diikm4zx4bggb4cq",
        ),
    );
    assert_eq!(t.client().get_user_template(&user), Some(3u32));
}

#[test]
fn test_set_user_template_updates_minted_portfolio() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().register_template(&t.admin, &2u32, &uri);
    t.client()
        .claim_folder(&user, &String::from_str(&t.env, "switch.folder"));
    t.client().mint_portfolio(
        &user,
        &1u32,
        &String::from_str(
            &t.env,
            "bafkreidywsp3ftbnf3lmdo4dqoz5ez5txrrdvcqpwsx7jks5hw3uyo2jm4",
        ),
    );

    t.client().set_user_template(&user, &2u32);
    assert_eq!(t.client().get_portfolio(&user).template_id, 2u32);
    assert_eq!(t.client().get_user_template(&user), Some(2u32));
}

// ── Validation hardening ────────────────────────────────────────────────────

const VALID_CIDV1_RAW: &str = "bafkreicu2xn7aesolgx3umlcnw3tozt4xibpu42wa2i34dg3bz6kxgy7y4";
const VALID_CIDV1_DAG_PB: &str = "bafybeicu2xn7aesolgx3umlcnw3tozt4xibpu42wa2i34dg3bz6kxgy7y4";
const VALID_CIDV0: &str = "QmU3p6RJzZsp4ZTsi6JWeCDxD3JPinVgsZB2uRUS6APEce";

#[test]
fn test_folder_label_rules() {
    let t = TestEnv::new();
    for bad in [
        ".folder",
        "ab.folder",
        "-abc.folder",
        "abc-.folder",
        "a..folder",
        "a.b.folder",
        "abc.folder.folder",
        "ab c.folder",
        "abc.Folder",
    ] {
        let user = Address::generate(&t.env);
        let res = t
            .client()
            .try_claim_folder(&user, &String::from_str(&t.env, bad));
        expect_contract_error(res, ContractError::InvalidFolderName);
    }
    for good in [
        "abc.folder",
        "a-b.folder",
        "007.folder",
        "jane-doe-2.folder",
    ] {
        let user = Address::generate(&t.env);
        t.client()
            .claim_folder(&user, &String::from_str(&t.env, good));
    }
}

#[test]
fn test_folder_name_max_length_boundary() {
    let t = TestEnv::new();
    // 57-byte label + ".folder" = 64 bytes (accepted); one more byte is rejected.
    let ok = "a".repeat(57) + ".folder";
    let too_long = "a".repeat(58) + ".folder";
    t.client()
        .claim_folder(&Address::generate(&t.env), &String::from_str(&t.env, &ok));
    let res = t.client().try_claim_folder(
        &Address::generate(&t.env),
        &String::from_str(&t.env, &too_long),
    );
    expect_contract_error(res, ContractError::InvalidFolderName);
}

fn minted_env() -> (TestEnv, Address) {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        &(std::string::String::from("ipfs://") + VALID_CIDV1_RAW),
    );
    t.client().register_template(&t.admin, &1u32, &uri);
    t.client()
        .claim_folder(&user, &String::from_str(&t.env, "cidcheck.folder"));
    t.client()
        .mint_portfolio(&user, &1u32, &String::from_str(&t.env, VALID_CIDV1_RAW));
    (t, user)
}

#[test]
fn test_cid_format_rules() {
    let (t, user) = minted_env();
    let bad_charset = std::string::String::from("bafy") + &"_".repeat(55);
    let upper_v1 = VALID_CIDV1_RAW.to_uppercase();
    let short_v0 = &VALID_CIDV0[..45];
    let long_v0 = std::string::String::from(VALID_CIDV0) + "a";
    let v0_bad_alphabet = std::string::String::from("Qm") + &"0".repeat(44);
    let spaced = std::string::String::from(&VALID_CIDV1_RAW[..58]) + " ";
    for bad in [
        bad_charset.as_str(),
        upper_v1.as_str(),
        short_v0,
        long_v0.as_str(),
        v0_bad_alphabet.as_str(),
        spaced.as_str(),
        "bafy",
    ] {
        let res = t
            .client()
            .try_update_portfolio(&user, &String::from_str(&t.env, bad));
        expect_contract_error(res, ContractError::InvalidCID);
    }
    for good in [VALID_CIDV0, VALID_CIDV1_RAW, VALID_CIDV1_DAG_PB] {
        let cid = String::from_str(&t.env, good);
        t.client().update_portfolio(&user, &cid);
        assert_eq!(t.client().get_portfolio(&user).ipfs_cid, cid);
    }
}

#[test]
fn test_metadata_uri_format_rules() {
    let t = TestEnv::new();
    let with = |suffix: &str| std::string::String::from("ipfs://") + suffix;
    let bad = [
        with("tmpl"),
        with("bafy_not_a_cid"),
        with(&(std::string::String::from(VALID_CIDV1_RAW) + "/has space.json")),
        with(&(std::string::String::from(VALID_CIDV1_RAW) + "?q=1")),
        with(""),
        std::string::String::from("ipfs:/") + VALID_CIDV1_RAW,
    ];
    for (i, uri) in bad.iter().enumerate() {
        let res = t.client().try_register_template(
            &t.admin,
            &(i as u32 + 1),
            &String::from_str(&t.env, uri),
        );
        expect_contract_error(res, ContractError::InvalidMetadataURI);
    }
    let good = [
        with(VALID_CIDV1_RAW),
        with(VALID_CIDV0),
        with(&(std::string::String::from(VALID_CIDV1_DAG_PB) + "/templates/1.json")),
    ];
    for (i, uri) in good.iter().enumerate() {
        t.client()
            .register_template(&t.admin, &(i as u32 + 100), &String::from_str(&t.env, uri));
    }
}

// ── Authorization ───────────────────────────────────────────────────────────

/// Asserts the last invocation required exactly one signature, from `who`,
/// for `function` on this contract.
fn assert_single_auth(t: &TestEnv, who: &Address, function: &str) {
    let auths = t.env.auths();
    assert_eq!(auths.len(), 1, "expected exactly one authorizer");
    let (addr, invocation) = &auths[0];
    assert_eq!(addr, who);
    match &invocation.function {
        AuthorizedFunction::Contract((contract, name, _)) => {
            assert_eq!(contract, &t.contract_id);
            assert_eq!(name, &soroban_sdk::Symbol::new(&t.env, function));
        }
        _ => panic!("unexpected authorized function"),
    }
}

#[test]
fn test_each_write_requires_the_right_signer() {
    let t = uninitialized_env();
    t.client().initialize(&t.admin);
    assert_single_auth(&t, &t.admin, "initialize");

    let other_admin = Address::generate(&t.env);
    let op_id = t
        .client()
        .queue_admin_change(&t.admin, &AdminOp::Add, &other_admin);
    assert_single_auth(&t, &t.admin, "queue_admin_change");
    t.env
        .ledger()
        .set_sequence_number(t.env.ledger().sequence() + ADMIN_TIMELOCK_LEDGERS);
    t.client().execute_admin_change(&t.admin, &op_id);
    assert_single_auth(&t, &t.admin, "execute_admin_change");
    remove_admin(&t, &other_admin, &t.admin);
    assert_single_auth(&t, &other_admin, "execute_admin_change");
    let uri = String::from_str(
        &t.env,
        &(std::string::String::from("ipfs://") + VALID_CIDV1_RAW),
    );
    t.client().register_template(&other_admin, &1u32, &uri);
    assert_single_auth(&t, &other_admin, "register_template");

    let user = Address::generate(&t.env);
    t.client()
        .claim_folder(&user, &String::from_str(&t.env, "signer.folder"));
    assert_single_auth(&t, &user, "claim_folder");
    t.client().set_user_template(&user, &1u32);
    assert_single_auth(&t, &user, "set_user_template");
    t.client()
        .mint_portfolio(&user, &1u32, &String::from_str(&t.env, VALID_CIDV1_RAW));
    assert_single_auth(&t, &user, "mint_portfolio");
    t.client()
        .update_portfolio(&user, &String::from_str(&t.env, VALID_CIDV0));
    assert_single_auth(&t, &user, "update_portfolio");
}

#[test]
fn test_writes_fail_without_signature() {
    let (t, user) = minted_env();
    t.env.set_auths(&[]);

    let name = String::from_str(&t.env, "nosig.folder");
    let cid = String::from_str(&t.env, VALID_CIDV0);
    let uri = String::from_str(
        &t.env,
        &(std::string::String::from("ipfs://") + VALID_CIDV0),
    );
    assert!(t
        .client()
        .try_claim_folder(&Address::generate(&t.env), &name)
        .is_err());
    assert!(t.client().try_update_portfolio(&user, &cid).is_err());
    assert!(t.client().try_set_user_template(&user, &1u32).is_err());
    assert!(t
        .client()
        .try_register_template(&t.admin, &2u32, &uri)
        .is_err());
    assert!(try_add_admin(&t, &t.admin, &Address::generate(&t.env)).is_err());
    assert_eq!(
        t.client().get_portfolio(&user).ipfs_cid,
        String::from_str(&t.env, VALID_CIDV1_RAW)
    );
}

// ── Economics ───────────────────────────────────────────────────────────────

mod economics {
    use super::*;
    use crate::constants::{MAX_CLAIM_FEE, MAX_WITHDRAW_PER_CALL};
    use crate::events::{FolderClaimed, Withdrawn};
    use crate::types::ClaimFee;
    use soroban_sdk::testutils::{AuthorizedInvocation, MockAuth, MockAuthInvoke};
    use soroban_sdk::{token, IntoVal, Symbol};

    const FEE: i128 = 50_000_000;

    fn sac(t: &TestEnv) -> (Address, token::StellarAssetClient<'_>, token::Client<'_>) {
        let issuer = Address::generate(&t.env);
        let id = t.env.register_stellar_asset_contract_v2(issuer).address();
        (
            id.clone(),
            token::StellarAssetClient::new(&t.env, &id),
            token::Client::new(&t.env, &id),
        )
    }

    fn name(t: &TestEnv, s: &str) -> String {
        String::from_str(&t.env, s)
    }

    #[test]
    fn funded_claim_pays_fee_and_emits_it() {
        let t = TestEnv::new();
        let (tok, admin_tok, tok_client) = sac(&t);
        let treasury = Address::generate(&t.env);
        t.client().set_claim_fee(&t.admin, &tok, &FEE, &treasury);
        assert_eq!(
            t.client().get_claim_fee(),
            Some(ClaimFee {
                token: tok.clone(),
                amount: FEE,
                treasury: treasury.clone()
            })
        );

        let user = Address::generate(&t.env);
        admin_tok.mint(&user, &(FEE + 1));
        t.client().claim_folder(&user, &name(&t, "payer.folder"));
        assert_eq!(
            t.env.events().all().filter_by_contract(&t.contract_id),
            std::vec![FolderClaimed {
                owner: user.clone(),
                folder_name: name(&t, "payer.folder"),
                fee_paid: FEE,
            }
            .to_xdr(&t.env, &t.contract_id)]
        );
        assert_eq!(tok_client.balance(&user), 1);
        assert_eq!(tok_client.balance(&treasury), FEE);
    }

    #[test]
    fn claim_auth_tree_includes_token_transfer() {
        let t = TestEnv::new();
        let (tok, admin_tok, _) = sac(&t);
        let treasury = Address::generate(&t.env);
        t.client().set_claim_fee(&t.admin, &tok, &FEE, &treasury);
        let user = Address::generate(&t.env);
        admin_tok.mint(&user, &FEE);

        let folder = name(&t, "authed.folder");
        t.client().claim_folder(&user, &folder);
        assert_eq!(
            t.env.auths(),
            std::vec![(
                user.clone(),
                AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        t.contract_id.clone(),
                        Symbol::new(&t.env, "claim_folder"),
                        (user.clone(), folder.clone()).into_val(&t.env),
                    )),
                    sub_invocations: std::vec![AuthorizedInvocation {
                        function: AuthorizedFunction::Contract((
                            tok.clone(),
                            Symbol::new(&t.env, "transfer"),
                            (user.clone(), treasury.clone(), FEE).into_val(&t.env),
                        )),
                        sub_invocations: std::vec![],
                    }],
                }
            )]
        );
    }

    #[test]
    fn claim_without_transfer_auth_fails() {
        let t = TestEnv::new();
        let (tok, admin_tok, _) = sac(&t);
        let treasury = Address::generate(&t.env);
        t.client().set_claim_fee(&t.admin, &tok, &FEE, &treasury);
        let user = Address::generate(&t.env);
        admin_tok.mint(&user, &FEE);

        // Authorize only the root call, not the token sub-invocation.
        let folder = name(&t, "partial.folder");
        t.env.set_auths(&[]);
        let res = t
            .client()
            .mock_auths(&[MockAuth {
                address: &user,
                invoke: &MockAuthInvoke {
                    contract: &t.contract_id,
                    fn_name: "claim_folder",
                    args: (user.clone(), folder.clone()).into_val(&t.env),
                    sub_invokes: &[],
                },
            }])
            .try_claim_folder(&user, &folder);
        assert!(res.is_err());
        expect_contract_error(
            t.client().try_get_folder(&folder),
            ContractError::FolderNotFound,
        );
    }

    #[test]
    fn insufficient_balance_claim_fails_atomically() {
        let t = TestEnv::new();
        let (tok, admin_tok, tok_client) = sac(&t);
        let treasury = Address::generate(&t.env);
        t.client().set_claim_fee(&t.admin, &tok, &FEE, &treasury);
        let user = Address::generate(&t.env);
        admin_tok.mint(&user, &(FEE - 1));

        let folder = name(&t, "broke.folder");
        assert!(t.client().try_claim_folder(&user, &folder).is_err());
        expect_contract_error(
            t.client().try_get_folder(&folder),
            ContractError::FolderNotFound,
        );
        expect_contract_error(
            t.client().try_get_identity(&user),
            ContractError::FolderNotFound,
        );
        assert_eq!(tok_client.balance(&user), FEE - 1);
        assert_eq!(tok_client.balance(&treasury), 0);
    }

    #[test]
    fn fee_disabled_and_changed() {
        let t = TestEnv::new();
        let (tok, admin_tok, tok_client) = sac(&t);
        let treasury = Address::generate(&t.env);

        // Disabled: unfunded user claims for free, event reports 0.
        t.client().set_claim_fee(&t.admin, &tok, &0, &treasury);
        let free = Address::generate(&t.env);
        t.client().claim_folder(&free, &name(&t, "free.folder"));
        assert_eq!(
            t.env.events().all().filter_by_contract(&t.contract_id),
            std::vec![FolderClaimed {
                owner: free.clone(),
                folder_name: name(&t, "free.folder"),
                fee_paid: 0,
            }
            .to_xdr(&t.env, &t.contract_id)]
        );
        assert_single_auth(&t, &free, "claim_folder");

        // Changed: the new amount is charged.
        t.client()
            .set_claim_fee(&t.admin, &tok, &(FEE * 2), &treasury);
        let payer = Address::generate(&t.env);
        admin_tok.mint(&payer, &(FEE * 2));
        t.client().claim_folder(&payer, &name(&t, "paid.folder"));
        assert_eq!(tok_client.balance(&treasury), FEE * 2);
    }

    #[test]
    fn set_claim_fee_validation_and_auth() {
        let t = TestEnv::new();
        let (tok, _, _) = sac(&t);
        let treasury = Address::generate(&t.env);
        expect_contract_error(
            t.client().try_set_claim_fee(&t.admin, &tok, &-1, &treasury),
            ContractError::InvalidAmount,
        );
        expect_contract_error(
            t.client()
                .try_set_claim_fee(&t.admin, &tok, &(MAX_CLAIM_FEE + 1), &treasury),
            ContractError::InvalidAmount,
        );
        expect_contract_error(
            t.client()
                .try_set_claim_fee(&Address::generate(&t.env), &tok, &FEE, &treasury),
            ContractError::Unauthorized,
        );
        assert_eq!(t.client().get_claim_fee(), None);
    }

    /// Fee paid into the contract itself, so it can be withdrawn.
    fn funded_treasury(t: &TestEnv) -> (Address, token::Client<'_>) {
        let (tok, admin_tok, tok_client) = sac(t);
        t.client()
            .set_claim_fee(&t.admin, &tok, &FEE, &t.contract_id);
        let user = Address::generate(&t.env);
        admin_tok.mint(&user, &FEE);
        t.client().claim_folder(&user, &name(t, "fund.folder"));
        (tok, tok_client)
    }

    #[test]
    fn withdraw_happy_path_reconciles_and_emits() {
        let t = TestEnv::new();
        let (tok, tok_client) = funded_treasury(&t);
        assert_eq!(t.client().treasury_balance(&tok), FEE);
        assert_eq!(
            t.client().treasury_balance(&tok),
            tok_client.balance(&t.contract_id)
        );

        let to = Address::generate(&t.env);
        t.client().withdraw(&t.admin, &tok, &to, &(FEE - 10));
        assert_single_auth(&t, &t.admin, "withdraw");
        assert_eq!(
            t.env.events().all().filter_by_contract(&t.contract_id),
            std::vec![Withdrawn {
                admin: t.admin.clone(),
                token: tok.clone(),
                to: to.clone(),
                amount: FEE - 10,
            }
            .to_xdr(&t.env, &t.contract_id)]
        );
        assert_eq!(t.client().treasury_balance(&tok), 10);
        assert_eq!(tok_client.balance(&t.contract_id), 10);
        assert_eq!(tok_client.balance(&to), FEE - 10);
    }

    #[test]
    fn withdraw_rejections() {
        let t = TestEnv::new();
        let (tok, _) = funded_treasury(&t);
        let to = Address::generate(&t.env);
        expect_contract_error(
            t.client().try_withdraw(&t.admin, &tok, &to, &(FEE + 1)),
            ContractError::InsufficientBalance,
        );
        expect_contract_error(
            t.client().try_withdraw(&t.admin, &tok, &to, &-1),
            ContractError::InvalidAmount,
        );
        expect_contract_error(
            t.client().try_withdraw(&t.admin, &tok, &to, &0),
            ContractError::InvalidAmount,
        );
        expect_contract_error(
            t.client()
                .try_withdraw(&Address::generate(&t.env), &tok, &to, &1),
            ContractError::Unauthorized,
        );
        assert_eq!(t.client().treasury_balance(&tok), FEE);
    }

    #[test]
    fn withdraw_cap_enforced() {
        let t = TestEnv::new();
        let (tok, admin_tok, _) = sac(&t);
        admin_tok.mint(&t.contract_id, &(MAX_WITHDRAW_PER_CALL + 1));
        let to = Address::generate(&t.env);
        expect_contract_error(
            t.client()
                .try_withdraw(&t.admin, &tok, &to, &(MAX_WITHDRAW_PER_CALL + 1)),
            ContractError::InvalidAmount,
        );
        t.client()
            .withdraw(&t.admin, &tok, &to, &MAX_WITHDRAW_PER_CALL);
        assert_eq!(t.client().treasury_balance(&tok), 1);
    }
}

// ── Budgets (used by scripts/sdk-compat.sh) ─────────────────────────────────

/// Prints CPU instructions and memory bytes per entry point. Run with
/// `cargo test budget_report -- --ignored --nocapture`.
#[test]
#[ignore]
fn budget_report() {
    fn report(t: &TestEnv, entry: &str) {
        let b = t.env.cost_estimate().budget();
        std::println!(
            "BUDGET {entry} cpu={} mem={}",
            b.cpu_instruction_cost(),
            b.memory_bytes_cost()
        );
    }
    let t = uninitialized_env();
    t.client().initialize(&t.admin);
    report(&t, "initialize");
    let other = Address::generate(&t.env);
    add_admin(&t, &t.admin, &other);
    report(&t, "add_admin");
    remove_admin(&t, &t.admin, &other);
    report(&t, "remove_admin");
    let uri = String::from_str(
        &t.env,
        &(std::string::String::from("ipfs://") + VALID_CIDV0),
    );
    t.client().register_template(&t.admin, &1u32, &uri);
    report(&t, "register_template");
    let user = Address::generate(&t.env);
    t.client()
        .claim_folder(&user, &String::from_str(&t.env, "budget.folder"));
    report(&t, "claim_folder");
    t.client().set_user_template(&user, &1u32);
    report(&t, "set_user_template");
    t.client()
        .mint_portfolio(&user, &1u32, &String::from_str(&t.env, VALID_CIDV0));
    report(&t, "mint_portfolio");
    t.client()
        .update_portfolio(&user, &String::from_str(&t.env, VALID_CIDV1_RAW));
    report(&t, "update_portfolio");
}
