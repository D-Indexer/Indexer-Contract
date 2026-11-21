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

/// Moves the ledger past `ADMIN_TIMELOCK_LEDGERS`.
fn advance_past_timelock(t: &TestEnv) {
    let seq = t.env.ledger().sequence();
    t.env
        .ledger()
        .set_sequence_number(seq + ADMIN_TIMELOCK_LEDGERS);
}

/// Queues an admin change, waits out the timelock and executes it.
fn try_admin_change(
    t: &TestEnv,
    by: &Address,
    op: AdminOp,
    target: &Address,
) -> Result<(), Result<ContractError, soroban_sdk::InvokeError>> {
    let op_id = t.client().try_queue_admin_change(by, &op, target)?.unwrap();
    advance_past_timelock(t);
    t.client()
        .try_execute_admin_change(by, &op_id)
        .map(|r| r.unwrap())
}

fn add_admin(t: &TestEnv, by: &Address, target: &Address) {
    try_admin_change(t, by, AdminOp::Add, target).unwrap();
}

fn remove_admin(t: &TestEnv, by: &Address, target: &Address) {
    try_admin_change(t, by, AdminOp::Remove, target).unwrap();
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

/// Minimal custom account standing in for a wallet: accepts any signature, so
/// the host's own auth-entry checks (args, nonce, expiration ledger) are the
/// only thing under test.
mod relay_account {
    use soroban_sdk::{
        auth::{Context, CustomAccountInterface},
        contract, contractimpl,
        crypto::Hash,
        Env, Error, Vec,
    };

    #[contract]
    pub struct AlwaysAuth;

    #[contractimpl]
    impl CustomAccountInterface for AlwaysAuth {
        type Signature = ();
        type Error = Error;

        fn __check_auth(
            _env: Env,
            _signature_payload: Hash<32>,
            _signature: (),
            _auth_contexts: Vec<Context>,
        ) -> Result<(), Error> {
            Ok(())
        }
    }
}

/// Registers a custom-account owner for the relayed-submission tests.
fn relay_owner(t: &TestEnv) -> Address {
    t.env.register(relay_account::AlwaysAuth, ())
}

/// Installs the SorobanAuthorizationEntry an owner's wallet would sign for a
/// relayer to submit: a single contract invocation, valid through
/// `live_until_ledger`. Disables auth mocking for the next invocation.
fn authorize_owner(
    t: &TestEnv,
    owner: &Address,
    fn_name: &str,
    args: Vec<Val>,
    live_until_ledger: u32,
) {
    use soroban_sdk::xdr::{
        InvokeContractArgs, ScVal, SorobanAddressCredentials, SorobanAuthorizationEntry,
        SorobanAuthorizedFunction, SorobanAuthorizedInvocation, SorobanCredentials,
    };
    use soroban_sdk::TryFromVal;

    let args: std::vec::Vec<ScVal> = args
        .iter()
        .map(|v| ScVal::try_from_val(&t.env, &v).unwrap())
        .collect();
    let entry = SorobanAuthorizationEntry {
        credentials: SorobanCredentials::Address(SorobanAddressCredentials {
            address: owner.into(),
            nonce: t.env.ledger().sequence() as i64,
            signature_expiration_ledger: live_until_ledger,
            signature: ScVal::Void,
        }),
        root_invocation: SorobanAuthorizedInvocation {
            function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
                contract_address: (&t.contract_id).into(),
                function_name: fn_name.try_into().unwrap(),
                args: args.try_into().unwrap(),
            }),
            sub_invocations: Default::default(),
        },
    };
    t.env.set_auths(&[entry]);
}

#[test]
fn test_relayed_claim_folder_with_owner_auth_entry() {
    let t = TestEnv::new();
    let owner = relay_owner(&t);
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
    let owner = relay_owner(&t);
    let name = String::from_str(&t.env, "relayed-mint.folder");
    let cid = String::from_str(
        &t.env,
        "bafkreif3kqdiv2uf7kt6jb2tacbtm27jsyrzbl4cfzghd3y2zjydhsb6my",
    );

    t.client().claim_folder(&owner, &name);
    t.client()
        .register_template(&t.admin, &1u32, &template_uri(&t.env));

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
    let owner = relay_owner(&t);
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
    let owner = relay_owner(&t);
    let name = String::from_str(&t.env, "expired-mint.folder");
    let cid = String::from_str(
        &t.env,
        "bafkreif3kqdiv2uf7kt6jb2tacbtm27jsyrzbl4cfzghd3y2zjydhsb6my",
    );

    t.client().claim_folder(&owner, &name);
    t.client()
        .register_template(&t.admin, &1u32, &template_uri(&t.env));

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
    let owner = relay_owner(&t);
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
    let owner = relay_owner(&t);
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
    t.client()
        .register_template(&t.admin, &1u32, &template_uri(&t.env));

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
    let owner = relay_owner(&t);
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
//
// Since protocol 23 the network restores archived entries automatically when a
// transaction touches them, and SDK 23+ emulates that in tests (see
// `soroban_sdk::_migrating::v23_archived_testing`). These tests therefore pin
// the exact TTL each write sets and check that archival never frees a name,
// template or portfolio: once restored, every invariant still holds.

impl TestEnv {
    fn has_persistent(&self, key: &DataKey) -> bool {
        self.env.as_contract(&self.contract_id, || {
            self.env.storage().persistent().has(key)
        })
    }
}

/// Asserts the entry was written with the full persistent lifetime, then
/// moves the ledger past it so the next access is an automatic restore.
fn archive_persistent(t: &TestEnv, key: &DataKey) {
    assert!(t.has_persistent(key));
    assert_eq!(t.persistent_ttl(key), PERSISTENT_BUMP_AMOUNT);
    let seq = t.env.ledger().sequence();
    t.env
        .ledger()
        .set_sequence_number(seq + PERSISTENT_BUMP_AMOUNT + 1);
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

#[test]
fn test_persistent_ttl_below_max_entry_ttl() {
    // The host clamps `extend_ttl` to `max_entry_ttl`; staying below it keeps
    // the requested lifetime exact.
    const { assert!(PERSISTENT_BUMP_AMOUNT < 3_110_400) };
    const { assert!(PERSISTENT_BUMP_AMOUNT > PERSISTENT_LIFETIME_THRESHOLD) };
}

#[test]
fn test_folder_name_survives_archival() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let other = Address::generate(&t.env);
    let name = String::from_str(&t.env, "archive.folder");

    t.client().claim_folder(&user, &name);
    archive_persistent(&t, &DataKey::FolderOwner(name.clone()));

    // The restored claim still resolves and the name cannot be re-claimed.
    assert_eq!(t.client().get_folder(&name), user);
    let res = t.client().try_claim_folder(&other, &name);
    expect_contract_error(res, ContractError::FolderNameTaken);
}

#[test]
fn test_identity_survives_archival() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "identity.folder");

    t.client().claim_folder(&user, &name);
    archive_persistent(&t, &DataKey::OwnerFolder(user.clone()));

    assert_eq!(t.client().get_identity(&user), name);
    let res = t
        .client()
        .try_claim_folder(&user, &String::from_str(&t.env, "second.folder"));
    expect_contract_error(res, ContractError::FolderAlreadyClaimed);
}

#[test]
fn test_template_survives_archival() {
    let t = TestEnv::new();
    let uri = template_uri(&t.env);

    t.client().register_template(&t.admin, &1u32, &uri);
    archive_persistent(&t, &DataKey::Template(1u32));

    assert_eq!(t.client().get_template(&1u32).metadata_uri, uri);
    let res = t.client().try_register_template(&t.admin, &1u32, &uri);
    expect_contract_error(res, ContractError::TemplateAlreadyExists);
}

#[test]
fn test_template_list_survives_archival() {
    let t = TestEnv::new();
    t.client()
        .register_template(&t.admin, &1u32, &template_uri(&t.env));
    archive_persistent(&t, &DataKey::TemplateList);

    assert_eq!(t.client().list_templates(), soroban_sdk::vec![&t.env, 1u32]);
}

#[test]
fn test_portfolio_survives_archival() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let cid = portfolio_cid(&t.env);

    t.client()
        .register_template(&t.admin, &1u32, &template_uri(&t.env));
    t.client()
        .claim_folder(&user, &String::from_str(&t.env, "nft.folder"));
    t.client().mint_portfolio(&user, &1u32, &cid);
    archive_persistent(&t, &DataKey::Portfolio(user.clone()));

    assert_eq!(t.client().get_portfolio(&user).ipfs_cid, cid);
    assert!(t.client().verify_portfolio(&user));
    let res = t.client().try_mint_portfolio(&user, &1u32, &cid);
    expect_contract_error(res, ContractError::PortfolioAlreadyMinted);
}

#[test]
fn test_user_template_survives_archival() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);

    t.client()
        .register_template(&t.admin, &1u32, &template_uri(&t.env));
    t.client()
        .claim_folder(&user, &String::from_str(&t.env, "assigned.folder"));
    t.client()
        .mint_portfolio(&user, &1u32, &portfolio_cid(&t.env));
    archive_persistent(&t, &DataKey::UserTemplate(user.clone()));

    assert_eq!(t.client().get_user_template(&user), Some(1u32));
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
    let res = try_admin_change(&t, &t.admin, AdminOp::Add, &new_admin);
    expect_contract_error(res, ContractError::AdminAlreadyExists);
}

#[test]
fn test_cannot_remove_last_admin() {
    let t = TestEnv::new();
    let res = try_admin_change(&t, &t.admin, AdminOp::Remove, &t.admin);
    expect_contract_error(res, ContractError::CannotRemoveLastAdmin);
}

#[test]
fn test_remove_unknown_admin_rejected() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);
    let unknown = Address::generate(&t.env);

    add_admin(&t, &t.admin, &new_admin);
    let res = try_admin_change(&t, &t.admin, AdminOp::Remove, &unknown);
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
            .to_xdr(&t.env, &t.contract_id)
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
            .to_xdr(&t.env, &t.contract_id)
        ]
    );
    assert_eq!(
        t.client().get_admins(),
        soroban_sdk::vec![&t.env, new_admin]
    );
}

#[test]
fn test_admin_change_timelock() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);
    let op_id = t
        .client()
        .queue_admin_change(&t.admin, &AdminOp::Add, &new_admin);

    let seq = t.env.ledger().sequence();
    t.env
        .ledger()
        .set_sequence_number(seq + ADMIN_TIMELOCK_LEDGERS - 1);
    let res = t.client().try_execute_admin_change(&t.admin, &op_id);
    expect_contract_error(res, ContractError::AdminChangeNotReady);

    t.client().cancel_admin_change(&t.admin, &op_id);
    advance_past_timelock(&t);
    let res = t.client().try_execute_admin_change(&t.admin, &op_id);
    expect_contract_error(res, ContractError::AdminChangeNotFound);
    assert_eq!(t.client().get_admins().len(), 1);
}

#[test]
fn test_admin_list_is_bounded() {
    let t = TestEnv::new();
    for _ in 1..MAX_ADMINS {
        add_admin(&t, &t.admin, &Address::generate(&t.env));
    }
    assert_eq!(t.client().get_admins().len(), MAX_ADMINS);

    let res = try_admin_change(&t, &t.admin, AdminOp::Add, &Address::generate(&t.env));
    expect_contract_error(res, ContractError::TooManyAdmins);
}

#[test]
fn test_removed_admin_loses_admin_rights() {
    let t = TestEnv::new();
    let other = Address::generate(&t.env);
    add_admin(&t, &t.admin, &other);
    remove_admin(&t, &t.admin, &t.admin);

    let res = try_admin_change(&t, &t.admin, AdminOp::Add, &Address::generate(&t.env));
    expect_contract_error(res, ContractError::Unauthorized);
    let res = try_admin_change(&t, &t.admin, AdminOp::Remove, &other);
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
    advance_past_timelock(&t);
    t.client().execute_admin_change(&t.admin, &op_id);
    assert_single_auth(&t, &t.admin, "execute_admin_change");
    let op_id = t
        .client()
        .queue_admin_change(&other_admin, &AdminOp::Remove, &t.admin);
    t.client().cancel_admin_change(&other_admin, &op_id);
    assert_single_auth(&t, &other_admin, "cancel_admin_change");

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
    assert!(try_admin_change(&t, &t.admin, AdminOp::Add, &Address::generate(&t.env)).is_err());
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
    let op_id = t
        .client()
        .queue_admin_change(&t.admin, &AdminOp::Add, &other);
    report(&t, "queue_admin_change");
    advance_past_timelock(&t);
    t.client().execute_admin_change(&t.admin, &op_id);
    report(&t, "execute_admin_change");
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
// ── Pricing (#246) ────────────────────────────────────────────────────────────

mod pricing_and_leases {
    use super::*;
    use crate::constants::SECONDS_PER_DAY;
    use crate::events::{ClaimSponsored, FolderClaimed};
    use crate::pricing::{fee_for_label_len, validate_tiers};
    use crate::types::PriceTiers;
    use soroban_sdk::{
        testutils::{MockAuth, MockAuthInvoke},
        token::{StellarAssetClient, TokenClient},
        vec, IntoVal, Vec,
    };

    const BASE_FEE: i128 = 10;

    fn tiers(env: &Env, by_len: &[(u32, i128)]) -> PriceTiers {
        let mut v = Vec::new(env);
        for t in by_len {
            v.push_back(*t);
        }
        PriceTiers { by_len: v }
    }

    struct FeeEnv {
        t: TestEnv,
        token: Address,
        treasury: Address,
    }

    impl FeeEnv {
        /// 3-char labels cost 1000, 4-char labels 100, 5+ pay BASE_FEE.
        fn new() -> Self {
            let t = TestEnv::new();
            let token = t
                .env
                .register_stellar_asset_contract_v2(t.admin.clone())
                .address();
            let treasury = Address::generate(&t.env);
            t.client()
                .set_claim_fee(&t.admin, &token, &BASE_FEE, &treasury);
            t.client()
                .set_price_tiers(&t.admin, &tiers(&t.env, &[(3, 1000), (4, 100)]));
            FeeEnv { t, token, treasury }
        }

        fn funded(&self, amount: i128) -> Address {
            let user = Address::generate(&self.t.env);
            StellarAssetClient::new(&self.t.env, &self.token).mint(&user, &amount);
            user
        }

        fn balance(&self, who: &Address) -> i128 {
            TokenClient::new(&self.t.env, &self.token).balance(who)
        }
    }

    #[test]
    fn fee_tier_boundaries() {
        let env = Env::default();
        let t = tiers(&env, &[(3, 1000), (4, 100)]);
        assert_eq!(fee_for_label_len(&t, BASE_FEE, 3), 1000);
        assert_eq!(fee_for_label_len(&t, BASE_FEE, 4), 100);
        assert_eq!(fee_for_label_len(&t, BASE_FEE, 5), BASE_FEE);
        assert_eq!(fee_for_label_len(&t, BASE_FEE, 57), BASE_FEE);
        assert_eq!(fee_for_label_len(&tiers(&env, &[]), BASE_FEE, 3), BASE_FEE);
    }

    #[test]
    fn invalid_tier_tables_rejected() {
        let env = Env::default();
        let bad = [
            tiers(&env, &[(4, 100), (3, 1000)]),
            tiers(&env, &[(3, 100), (3, 50)]),
            tiers(&env, &[(2, 100)]),
            tiers(&env, &[(3, -1)]),
            tiers(&env, &[(3, 1), (4, 1), (5, 1), (6, 1), (7, 1), (8, 1)]),
        ];
        for b in bad.iter() {
            assert_eq!(validate_tiers(b), Err(ContractError::InvalidPriceTiers));
        }
        assert_eq!(
            validate_tiers(&tiers(&env, &[(3, 1), (4, 1), (5, 1), (6, 1), (7, 1)])),
            Ok(())
        );

        let f = FeeEnv::new();
        let res =
            f.t.client()
                .try_set_price_tiers(&f.t.admin, &tiers(&f.t.env, &[(4, 1), (3, 1)]));
        expect_contract_error(res, ContractError::InvalidPriceTiers);
        let res =
            f.t.client()
                .try_set_claim_fee(&f.t.admin, &f.token, &-1, &f.treasury);
        expect_contract_error(res, ContractError::InvalidAmount);
        let outsider = Address::generate(&f.t.env);
        let res =
            f.t.client()
                .try_set_price_tiers(&outsider, &tiers(&f.t.env, &[]));
        expect_contract_error(res, ContractError::Unauthorized);
    }

    #[test]
    fn quote_equals_charge_for_every_tier() {
        let f = FeeEnv::new();
        for (name, expected) in [
            ("abc.folder", 1000),
            ("abcd.folder", 100),
            ("abcde.folder", BASE_FEE),
            ("abcdefghij.folder", BASE_FEE),
        ] {
            let name = String::from_str(&f.t.env, name);
            let quote = f.t.client().quote_claim(&name);
            assert_eq!(quote, expected);
            let user = f.funded(5000);
            let before = f.balance(&f.treasury);
            f.t.client().claim_folder(&user, &name);
            assert_eq!(f.balance(&user), 5000 - quote);
            assert_eq!(f.balance(&f.treasury) - before, quote);
        }
    }

    #[test]
    fn claims_are_free_without_fee_config() {
        let t = TestEnv::new();
        let name = String::from_str(&t.env, "abc.folder");
        assert_eq!(t.client().quote_claim(&name), 0);
        t.client().claim_folder(&Address::generate(&t.env), &name);
    }

    // ── Sponsored claims (#247) ───────────────────────────────────────────────

    #[test]
    fn sponsored_claim_charges_sponsor_and_records_both_signers() {
        let f = FeeEnv::new();
        let sponsor = f.funded(1000);
        let owner = f.funded(500);
        let name = String::from_str(&f.t.env, "abcd.folder");

        f.t.client().sponsored_claim(&sponsor, &owner, &name);
        let events = f.t.env.events().all();

        let auths = f.t.env.auths();
        assert_eq!(auths.len(), 2);
        assert_eq!(auths[0].0, sponsor);
        assert_eq!(auths[1].0, owner);
        for (_, invocation) in auths.iter() {
            assert_eq!(
                invocation.function,
                AuthorizedFunction::Contract((
                    f.t.contract_id.clone(),
                    soroban_sdk::Symbol::new(&f.t.env, "sponsored_claim"),
                    (sponsor.clone(), owner.clone(), name.clone()).into_val(&f.t.env),
                ))
            );
        }
        // Only the sponsor's authorization covers the fee transfer.
        assert_eq!(auths[0].1.sub_invocations.len(), 1);
        assert!(auths[1].1.sub_invocations.is_empty());

        assert_eq!(f.balance(&owner), 500);
        assert_eq!(f.balance(&sponsor), 900);
        assert_eq!(f.t.client().get_folder(&name), owner);
        assert_eq!(f.t.client().get_identity(&owner), name);
        assert!(f.t.client().try_get_identity(&sponsor).is_err());

        let n = events.events().len();
        assert_eq!(
            events.events()[n - 2..].to_vec(),
            std::vec![
                FolderClaimed {
                    owner: owner.clone(),
                    folder_name: name.clone(),
                    fee_paid: 100,
                }
                .to_xdr(&f.t.env, &f.t.contract_id),
                ClaimSponsored {
                    sponsor: sponsor.clone(),
                    owner: owner.clone(),
                    folder_name: name.clone(),
                }
                .to_xdr(&f.t.env, &f.t.contract_id),
            ]
        );
    }

    #[test]
    fn sponsored_claim_fails_on_insufficient_sponsor_balance() {
        let f = FeeEnv::new();
        let sponsor = f.funded(99);
        let owner = f.funded(1000);
        let name = String::from_str(&f.t.env, "abcd.folder");
        assert!(f
            .t
            .client()
            .try_sponsored_claim(&sponsor, &owner, &name)
            .is_err());
        assert_eq!(f.balance(&owner), 1000);
        assert!(f.t.client().try_get_folder(&name).is_err());
    }

    #[test]
    fn sponsored_claim_requires_both_signatures() {
        let t = TestEnv::new();
        let sponsor = Address::generate(&t.env);
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "abcde.folder");
        for signer in [&sponsor, &owner] {
            t.env.mock_auths(&[MockAuth {
                address: signer,
                invoke: &MockAuthInvoke {
                    contract: &t.contract_id,
                    fn_name: "sponsored_claim",
                    args: vec![
                        &t.env,
                        sponsor.into_val(&t.env),
                        owner.into_val(&t.env),
                        name.into_val(&t.env),
                    ],
                    sub_invokes: &[],
                },
            }]);
            assert!(t
                .client()
                .try_sponsored_claim(&sponsor, &owner, &name)
                .is_err());
        }
        t.env.mock_all_auths();
        assert!(t.client().try_get_folder(&name).is_err());
    }

    // ── Leases (#248) ─────────────────────────────────────────────────────────

    const PERIOD_DAYS: u32 = 30;
    const GRACE_DAYS: u32 = 7;
    const PERIOD: u64 = PERIOD_DAYS as u64 * SECONDS_PER_DAY;
    const GRACE: u64 = GRACE_DAYS as u64 * SECONDS_PER_DAY;

    fn leased_env() -> TestEnv {
        let t = TestEnv::new();
        t.env.ledger().set_timestamp(1_000);
        t.client()
            .set_lease_config(&t.admin, &PERIOD_DAYS, &GRACE_DAYS);
        t
    }

    #[test]
    fn permanent_mode_is_default() {
        let t = TestEnv::new();
        assert_eq!(t.client().get_lease_config(), None);
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "perm.folder");
        t.client().claim_folder(&owner, &name);
        assert_eq!(t.client().get_folder_expiry(&name), None);
        t.env.ledger().set_timestamp(u64::MAX / 2);
        assert_eq!(t.client().get_folder(&name), owner);
        expect_contract_error(
            t.client().try_renew_folder(&owner, &1),
            ContractError::LeasesDisabled,
        );
    }

    #[test]
    fn lease_lifecycle_across_expiry_and_grace() {
        let t = leased_env();
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "lease.folder");
        t.client().claim_folder(&owner, &name);
        let expiry = 1_000 + PERIOD;
        assert_eq!(t.client().get_folder_expiry(&name), Some(expiry));

        t.env.ledger().set_timestamp(expiry - 1);
        assert_eq!(t.client().get_folder(&name), owner);

        t.env.ledger().set_timestamp(expiry);
        expect_contract_error(
            t.client().try_get_folder(&name),
            ContractError::FolderExpired,
        );
        expect_contract_error(
            t.client().try_get_identity(&owner),
            ContractError::FolderExpired,
        );
        let other = Address::generate(&t.env);
        expect_contract_error(
            t.client().try_claim_folder(&other, &name),
            ContractError::FolderNameTaken,
        );

        t.env.ledger().set_timestamp(expiry + GRACE - 1);
        expect_contract_error(
            t.client().try_get_folder(&name),
            ContractError::FolderExpired,
        );

        t.env.ledger().set_timestamp(expiry + GRACE);
        expect_contract_error(
            t.client().try_get_folder(&name),
            ContractError::FolderNotFound,
        );
        expect_contract_error(
            t.client().try_renew_folder(&owner, &1),
            ContractError::FolderNotFound,
        );
    }

    #[test]
    fn renewal_before_and_after_expiry() {
        let t = leased_env();
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "renew.folder");
        t.client().claim_folder(&owner, &name);
        let expiry = 1_000 + PERIOD;

        expect_contract_error(
            t.client().try_renew_folder(&owner, &0),
            ContractError::InvalidRenewal,
        );
        assert_eq!(t.client().renew_folder(&owner, &2), expiry + 2 * PERIOD);

        let expiry = expiry + 2 * PERIOD;
        t.env.ledger().set_timestamp(expiry + 1);
        expect_contract_error(
            t.client().try_get_folder(&name),
            ContractError::FolderExpired,
        );
        assert_eq!(t.client().renew_folder(&owner, &1), expiry + PERIOD);
        assert_eq!(t.client().get_folder(&name), owner);
    }

    #[test]
    fn renewal_is_charged_per_period() {
        let f = FeeEnv::new();
        f.t.client()
            .set_lease_config(&f.t.admin, &PERIOD_DAYS, &GRACE_DAYS);
        let owner = f.funded(1000);
        let name = String::from_str(&f.t.env, "abcd.folder");
        f.t.client().claim_folder(&owner, &name);
        f.t.client().renew_folder(&owner, &3);
        assert_eq!(f.balance(&owner), 1000 - 100 - 3 * 100);
    }

    #[test]
    fn reclaim_by_new_owner_cleans_up_old_keys() {
        let t = leased_env();
        let old = Address::generate(&t.env);
        let new = Address::generate(&t.env);
        let name = String::from_str(&t.env, "stale.folder");
        t.client().claim_folder(&old, &name);

        t.env.ledger().set_timestamp(1_000 + PERIOD + GRACE);
        t.client().claim_folder(&new, &name);

        assert_eq!(t.client().get_folder(&name), new);
        assert_eq!(t.client().get_identity(&new), name);
        assert_eq!(
            t.client().get_folder_expiry(&name),
            Some(1_000 + PERIOD + GRACE + PERIOD)
        );
        t.env.as_contract(&t.contract_id, || {
            assert!(!t
                .env
                .storage()
                .persistent()
                .has(&DataKey::OwnerFolder(old.clone())));
        });
        // The previous owner is free to claim a new name.
        t.client()
            .claim_folder(&old, &String::from_str(&t.env, "fresh.folder"));
    }

    #[test]
    fn expired_name_blocks_new_portfolio_mint() {
        let t = leased_env();
        let owner = Address::generate(&t.env);
        t.client()
            .claim_folder(&owner, &String::from_str(&t.env, "port.folder"));
        t.client().register_template(
            &t.admin,
            &1,
            &String::from_str(
                &t.env,
                "ipfs://bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia/template.json",
            ),
        );
        t.env.ledger().set_timestamp(1_000 + PERIOD);
        let cid = String::from_str(&t.env, VALID_CIDV0);
        expect_contract_error(
            t.client().try_mint_portfolio(&owner, &1, &cid),
            ContractError::FolderExpired,
        );
    }
}

// ── Commit-reveal claims ────────────────────────────────────────────────────

mod commit_reveal {
    use super::*;
    use crate::constants::{CLAIM_COMMIT_TTL, MIN_REVEAL_DELAY, REVEAL_WINDOW};
    use soroban_sdk::{testutils::storage::Temporary as _, xdr::ToXdr, Bytes, BytesN};

    fn salt(env: &Env, byte: u8) -> BytesN<32> {
        BytesN::from_array(env, &[byte; 32])
    }

    fn advance(t: &TestEnv, ledgers: u32) {
        let seq = t.env.ledger().sequence();
        t.env.ledger().set_sequence_number(seq + ledgers);
    }

    /// Commits `name` for `owner` and returns the salt used.
    fn commit(t: &TestEnv, owner: &Address, name: &String) -> BytesN<32> {
        let s = salt(&t.env, 7);
        let c = t.client().claim_commitment(owner, name, &s);
        t.client().commit_claim(owner, &c);
        s
    }

    #[test]
    fn commitment_matches_documented_preimage() {
        let t = TestEnv::new();
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "hidden.folder");
        let s = salt(&t.env, 1);

        let mut preimage = Bytes::new(&t.env);
        preimage.append(&name.to_bytes());
        preimage.append(&Bytes::from(s.clone()));
        preimage.append(&owner.clone().to_xdr(&t.env));
        let expected: BytesN<32> = t.env.crypto().sha256(&preimage).into();

        assert_eq!(t.client().claim_commitment(&owner, &name, &s), expected);
    }

    #[test]
    fn commit_then_reveal_claims_the_name() {
        let t = TestEnv::new();
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "hidden.folder");
        let s = commit(&t, &owner, &name);

        advance(&t, MIN_REVEAL_DELAY);
        t.client().reveal_claim(&owner, &name, &s);

        // The reveal emits the ordinary claim event.
        assert_eq!(
            t.env.events().all().filter_by_contract(&t.contract_id),
            [crate::events::FolderClaimed {
                owner: owner.clone(),
                folder_name: name.clone(),
                fee_paid: 0,
            }
            .to_xdr(&t.env, &t.contract_id)]
        );
        assert_eq!(t.client().get_folder(&name), owner);
        assert_eq!(t.client().get_identity(&owner), name);
        // The commit is consumed.
        let res = t.client().try_reveal_claim(&owner, &name, &s);
        expect_contract_error(res, ContractError::CommitmentNotFound);
    }

    #[test]
    fn commit_does_not_reveal_or_reserve_the_name() {
        let t = TestEnv::new();
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "hidden.folder");
        commit(&t, &owner, &name);

        // Nothing about the name is written or emitted at commit time.
        assert!(t
            .env
            .events()
            .all()
            .filter_by_contract(&t.contract_id)
            .events()
            .is_empty());
        expect_contract_error(
            t.client().try_get_folder(&name),
            ContractError::FolderNotFound,
        );
    }

    #[test]
    fn commit_lives_in_temporary_storage_above_the_reveal_window() {
        let t = TestEnv::new();
        let owner = Address::generate(&t.env);
        commit(&t, &owner, &String::from_str(&t.env, "hidden.folder"));

        let key = DataKey::ClaimCommit(owner.clone());
        t.env.as_contract(&t.contract_id, || {
            assert!(t.env.storage().temporary().has(&key));
            assert!(!t.env.storage().persistent().has(&key));
            assert_eq!(t.env.storage().temporary().get_ttl(&key), CLAIM_COMMIT_TTL);
        });
        const { assert!(CLAIM_COMMIT_TTL > REVEAL_WINDOW) };
    }

    #[test]
    fn reveal_delay_boundary() {
        let t = TestEnv::new();
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "hidden.folder");
        let s = commit(&t, &owner, &name);

        let res = t.client().try_reveal_claim(&owner, &name, &s);
        expect_contract_error(res, ContractError::RevealTooEarly);
        advance(&t, MIN_REVEAL_DELAY - 1);
        let res = t.client().try_reveal_claim(&owner, &name, &s);
        expect_contract_error(res, ContractError::RevealTooEarly);

        advance(&t, 1);
        t.client().reveal_claim(&owner, &name, &s);
        assert_eq!(t.client().get_folder(&name), owner);
    }

    #[test]
    fn reveal_window_boundary() {
        let t = TestEnv::new();
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "late.folder");
        let s = commit(&t, &owner, &name);
        advance(&t, REVEAL_WINDOW);
        t.client().reveal_claim(&owner, &name, &s);

        let other = Address::generate(&t.env);
        let name = String::from_str(&t.env, "later.folder");
        let s = commit(&t, &other, &name);
        advance(&t, REVEAL_WINDOW + 1);
        let res = t.client().try_reveal_claim(&other, &name, &s);
        expect_contract_error(res, ContractError::CommitmentNotFound);
    }

    #[test]
    fn reveal_without_commit_fails() {
        let t = TestEnv::new();
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "hidden.folder");
        let res = t.client().try_reveal_claim(&owner, &name, &salt(&t.env, 7));
        expect_contract_error(res, ContractError::CommitmentNotFound);
    }

    #[test]
    fn reveal_with_wrong_name_salt_or_owner_fails() {
        let t = TestEnv::new();
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "hidden.folder");
        let s = commit(&t, &owner, &name);
        advance(&t, MIN_REVEAL_DELAY);

        let other_name = String::from_str(&t.env, "other.folder");
        let res = t.client().try_reveal_claim(&owner, &other_name, &s);
        expect_contract_error(res, ContractError::CommitmentMismatch);
        let res = t.client().try_reveal_claim(&owner, &name, &salt(&t.env, 8));
        expect_contract_error(res, ContractError::CommitmentMismatch);

        // A third party cannot reuse the owner's commitment: it is stored and
        // hashed per owner.
        let thief = Address::generate(&t.env);
        let stolen = t.client().claim_commitment(&owner, &name, &s);
        t.client().commit_claim(&thief, &stolen);
        advance(&t, MIN_REVEAL_DELAY);
        let res = t.client().try_reveal_claim(&thief, &name, &s);
        expect_contract_error(res, ContractError::CommitmentMismatch);

        // The owner's commit survived the failed attempts.
        t.client().reveal_claim(&owner, &name, &s);
        assert_eq!(t.client().get_folder(&name), owner);
    }

    #[test]
    fn observer_of_a_reveal_cannot_front_run_it() {
        let t = TestEnv::new();
        let owner = Address::generate(&t.env);
        let attacker = Address::generate(&t.env);
        let name = String::from_str(&t.env, "valuable.folder");
        let s = commit(&t, &owner, &name);
        advance(&t, MIN_REVEAL_DELAY);

        // The attacker sees the pending reveal and commits the same name in
        // the same ledger, but cannot reveal yet.
        let attacker_salt = commit(&t, &attacker, &name);
        let res = t
            .client()
            .try_reveal_claim(&attacker, &name, &attacker_salt);
        expect_contract_error(res, ContractError::RevealTooEarly);

        t.client().reveal_claim(&owner, &name, &s);
        advance(&t, MIN_REVEAL_DELAY);
        let res = t
            .client()
            .try_reveal_claim(&attacker, &name, &attacker_salt);
        expect_contract_error(res, ContractError::FolderNameTaken);
        assert_eq!(t.client().get_folder(&name), owner);
    }

    #[test]
    fn recommit_replaces_previous_commit() {
        let t = TestEnv::new();
        let owner = Address::generate(&t.env);
        let first = String::from_str(&t.env, "first.folder");
        let second = String::from_str(&t.env, "second.folder");
        let s = commit(&t, &owner, &first);
        advance(&t, MIN_REVEAL_DELAY);
        commit(&t, &owner, &second);

        let res = t.client().try_reveal_claim(&owner, &first, &s);
        expect_contract_error(res, ContractError::RevealTooEarly);
        advance(&t, MIN_REVEAL_DELAY);
        let res = t.client().try_reveal_claim(&owner, &first, &s);
        expect_contract_error(res, ContractError::CommitmentMismatch);
        t.client().reveal_claim(&owner, &second, &s);
        assert_eq!(t.client().get_identity(&owner), second);
    }

    #[test]
    fn failed_claim_keeps_the_commit() {
        let t = TestEnv::new();
        let holder = Address::generate(&t.env);
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "taken.folder");
        t.client().claim_folder(&holder, &name);

        let s = commit(&t, &owner, &name);
        advance(&t, MIN_REVEAL_DELAY);
        let res = t.client().try_reveal_claim(&owner, &name, &s);
        expect_contract_error(res, ContractError::FolderNameTaken);
        t.env.as_contract(&t.contract_id, || {
            assert!(t
                .env
                .storage()
                .temporary()
                .has(&DataKey::ClaimCommit(owner.clone())));
        });
    }

    #[test]
    fn commit_and_reveal_require_owner_auth() {
        let t = TestEnv::new();
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "hidden.folder");
        let s = commit(&t, &owner, &name);
        assert_single_auth(&t, &owner, "commit_claim");
        advance(&t, MIN_REVEAL_DELAY);

        t.env.set_auths(&[]);
        assert!(t.client().try_reveal_claim(&owner, &name, &s).is_err());
        assert!(t
            .client()
            .try_commit_claim(&owner, &BytesN::from_array(&t.env, &[0; 32]))
            .is_err());

        t.env.mock_all_auths();
        t.client().reveal_claim(&owner, &name, &s);
        assert_single_auth(&t, &owner, "reveal_claim");
    }

    #[test]
    fn commit_reveal_requires_initialization() {
        let t = uninitialized_env();
        let owner = Address::generate(&t.env);
        let res = t
            .client()
            .try_commit_claim(&owner, &BytesN::from_array(&t.env, &[0; 32]));
        expect_contract_error(res, ContractError::NotInitialized);
        let res = t.client().try_reveal_claim(
            &owner,
            &String::from_str(&t.env, "hidden.folder"),
            &salt(&t.env, 1),
        );
        expect_contract_error(res, ContractError::NotInitialized);
    }
}

// ── Guardian recovery ───────────────────────────────────────────────────────

mod recovery {
    use super::*;
    use crate::constants::{MAX_GUARDIANS, RECOVERY_DELAY};
    use crate::events::{RecoveryApproved, RecoveryCancelled, RecoveryExecuted, RecoveryInitiated};
    use crate::types::GuardianConfig;

    struct RecEnv {
        t: TestEnv,
        owner: Address,
        name: String,
        guardians: std::vec::Vec<Address>,
        new_owner: Address,
    }

    /// An owner with a Folder, a minted portfolio and `n` guardians of which
    /// `threshold` must approve.
    fn rec_env(n: usize, threshold: u32) -> RecEnv {
        let t = TestEnv::new();
        let owner = Address::generate(&t.env);
        let name = String::from_str(&t.env, "lost-key.folder");
        t.client()
            .register_template(&t.admin, &1u32, &template_uri(&t.env));
        t.client().claim_folder(&owner, &name);
        t.client()
            .mint_portfolio(&owner, &1u32, &portfolio_cid(&t.env));
        let guardians: std::vec::Vec<Address> = (0..n).map(|_| Address::generate(&t.env)).collect();
        t.client()
            .set_guardians(&owner, &vec_of(&t.env, &guardians), &threshold);
        let new_owner = Address::generate(&t.env);
        RecEnv {
            t,
            owner,
            name,
            guardians,
            new_owner,
        }
    }

    fn vec_of(env: &Env, items: &[Address]) -> Vec<Address> {
        let mut v = Vec::new(env);
        for a in items {
            v.push_back(a.clone());
        }
        v
    }

    fn wait_delay(t: &TestEnv) {
        let seq = t.env.ledger().sequence();
        t.env.ledger().set_sequence_number(seq + RECOVERY_DELAY);
    }

    fn assert_no_keys_for(t: &TestEnv, who: &Address) {
        t.env.as_contract(&t.contract_id, || {
            let p = t.env.storage().persistent();
            for key in [
                DataKey::OwnerFolder(who.clone()),
                DataKey::Portfolio(who.clone()),
                DataKey::UserTemplate(who.clone()),
                DataKey::Guardians(who.clone()),
                DataKey::Recovery(who.clone()),
            ] {
                assert!(!p.has(&key), "stale key left for old owner");
            }
        });
    }

    #[test]
    fn full_lifecycle_threshold_one() {
        let r = rec_env(1, 1);
        let t = &r.t;
        let g = &r.guardians[0];

        t.client().initiate_recovery(g, &r.owner, &r.new_owner);
        assert_eq!(
            t.env.events().all().filter_by_contract(&t.contract_id),
            [RecoveryInitiated {
                guardian: g.clone(),
                owner: r.owner.clone(),
                new_owner: r.new_owner.clone(),
            }
            .to_xdr(&t.env, &t.contract_id)]
        );
        let pending = t.client().get_recovery(&r.owner).unwrap();
        assert_eq!(pending.new_owner, r.new_owner);
        assert_eq!(pending.approvals.len(), 1);

        wait_delay(t);
        assert_eq!(t.client().execute_recovery(&r.owner), r.new_owner);
        assert_eq!(
            t.env.events().all().filter_by_contract(&t.contract_id),
            [RecoveryExecuted {
                owner: r.owner.clone(),
                new_owner: r.new_owner.clone(),
                folder_name: r.name.clone(),
            }
            .to_xdr(&t.env, &t.contract_id)]
        );

        // The identity, portfolio, template and guardians moved.
        assert_eq!(t.client().get_folder(&r.name), r.new_owner);
        assert_eq!(t.client().get_identity(&r.new_owner), r.name);
        let nft = t.client().get_portfolio(&r.new_owner);
        assert_eq!(nft.owner, r.new_owner);
        assert_eq!(nft.ipfs_cid, portfolio_cid(&t.env));
        assert_eq!(t.client().get_user_template(&r.new_owner), Some(1u32));
        assert_eq!(
            t.client().get_guardians(&r.new_owner),
            Some(GuardianConfig {
                guardians: vec_of(&t.env, &r.guardians),
                threshold: 1,
            })
        );
        assert_eq!(t.client().get_recovery(&r.owner), None);

        // Old owner is gone everywhere.
        expect_contract_error(
            t.client().try_get_identity(&r.owner),
            ContractError::FolderNotFound,
        );
        expect_contract_error(
            t.client().try_get_portfolio(&r.owner),
            ContractError::PortfolioNotFound,
        );
        assert_eq!(t.client().get_user_template(&r.owner), None);
        assert_no_keys_for(t, &r.owner);

        // The new key controls the identity.
        let new_cid = String::from_str(&t.env, VALID_CIDV0);
        t.client().update_portfolio(&r.new_owner, &new_cid);
        expect_contract_error(
            t.client().try_update_portfolio(&r.owner, &new_cid),
            ContractError::PortfolioNotFound,
        );
        // The old address is free to start over.
        t.client()
            .claim_folder(&r.owner, &String::from_str(&t.env, "fresh.folder"));
    }

    #[test]
    fn threshold_three_needs_three_approvals() {
        let r = rec_env(5, 3);
        let t = &r.t;
        let g = &r.guardians;

        t.client().initiate_recovery(&g[0], &r.owner, &r.new_owner);
        t.client().approve_recovery(&g[1], &r.owner);
        assert_eq!(
            t.env.events().all().filter_by_contract(&t.contract_id),
            [RecoveryApproved {
                guardian: g[1].clone(),
                owner: r.owner.clone(),
                approvals: 2,
            }
            .to_xdr(&t.env, &t.contract_id)]
        );

        // Delay passed but only 2 of 3 approvals.
        wait_delay(t);
        expect_contract_error(
            t.client().try_execute_recovery(&r.owner),
            ContractError::RecoveryNotReady,
        );

        t.client().approve_recovery(&g[4], &r.owner);
        t.client().execute_recovery(&r.owner);
        assert_eq!(t.client().get_folder(&r.name), r.new_owner);
        assert_no_keys_for(t, &r.owner);
    }

    #[test]
    fn delay_boundary() {
        let r = rec_env(1, 1);
        let t = &r.t;
        t.env.ledger().set_sequence_number(500);
        t.client()
            .initiate_recovery(&r.guardians[0], &r.owner, &r.new_owner);

        t.env.ledger().set_sequence_number(500 + RECOVERY_DELAY - 1);
        expect_contract_error(
            t.client().try_execute_recovery(&r.owner),
            ContractError::RecoveryNotReady,
        );
        t.env.ledger().set_sequence_number(500 + RECOVERY_DELAY);
        t.client().execute_recovery(&r.owner);
        assert_eq!(t.client().get_folder(&r.name), r.new_owner);
    }

    #[test]
    fn owner_cancel_race() {
        let r = rec_env(3, 2);
        let t = &r.t;
        let g = &r.guardians;
        t.client().initiate_recovery(&g[0], &r.owner, &r.new_owner);
        t.client().approve_recovery(&g[1], &r.owner);

        // The owner still has their key and cancels in the last ledger
        // before the recovery becomes executable.
        let seq = t.env.ledger().sequence();
        t.env.ledger().set_sequence_number(seq + RECOVERY_DELAY - 1);
        t.client().cancel_recovery(&r.owner);
        assert_single_auth(t, &r.owner, "cancel_recovery");
        assert_eq!(
            t.env.events().all().filter_by_contract(&t.contract_id),
            [RecoveryCancelled {
                owner: r.owner.clone(),
                new_owner: r.new_owner.clone(),
            }
            .to_xdr(&t.env, &t.contract_id)]
        );
        assert_eq!(t.client().get_recovery(&r.owner), None);

        wait_delay(t);
        expect_contract_error(
            t.client().try_execute_recovery(&r.owner),
            ContractError::RecoveryNotFound,
        );
        expect_contract_error(
            t.client().try_approve_recovery(&g[2], &r.owner),
            ContractError::RecoveryNotFound,
        );
        assert_eq!(t.client().get_folder(&r.name), r.owner);

        // Approvals do not carry over to a new recovery.
        t.client().initiate_recovery(&g[2], &r.owner, &r.new_owner);
        wait_delay(t);
        expect_contract_error(
            t.client().try_execute_recovery(&r.owner),
            ContractError::RecoveryNotReady,
        );
    }

    #[test]
    fn cancel_after_execution_fails() {
        let r = rec_env(1, 1);
        let t = &r.t;
        t.client()
            .initiate_recovery(&r.guardians[0], &r.owner, &r.new_owner);
        wait_delay(t);
        t.client().execute_recovery(&r.owner);
        expect_contract_error(
            t.client().try_cancel_recovery(&r.owner),
            ContractError::RecoveryNotFound,
        );
        assert_eq!(t.client().get_folder(&r.name), r.new_owner);
    }

    #[test]
    fn duplicate_approval_rejected() {
        let r = rec_env(3, 2);
        let t = &r.t;
        let g = &r.guardians;
        t.client().initiate_recovery(&g[0], &r.owner, &r.new_owner);

        // The initiator's approval is already counted.
        expect_contract_error(
            t.client().try_approve_recovery(&g[0], &r.owner),
            ContractError::AlreadyApproved,
        );
        t.client().approve_recovery(&g[1], &r.owner);
        expect_contract_error(
            t.client().try_approve_recovery(&g[1], &r.owner),
            ContractError::AlreadyApproved,
        );
        assert_eq!(
            t.client().get_recovery(&r.owner).unwrap().approvals.len(),
            2
        );
    }

    #[test]
    fn non_guardians_cannot_act() {
        let r = rec_env(2, 1);
        let t = &r.t;
        let stranger = Address::generate(&t.env);
        expect_contract_error(
            t.client()
                .try_initiate_recovery(&stranger, &r.owner, &r.new_owner),
            ContractError::NotGuardian,
        );
        t.client()
            .initiate_recovery(&r.guardians[0], &r.owner, &r.new_owner);
        expect_contract_error(
            t.client().try_approve_recovery(&stranger, &r.owner),
            ContractError::NotGuardian,
        );
        // An owner without guardians cannot be recovered.
        let other = Address::generate(&t.env);
        t.client()
            .claim_folder(&other, &String::from_str(&t.env, "other.folder"));
        expect_contract_error(
            t.client()
                .try_initiate_recovery(&r.guardians[0], &other, &r.new_owner),
            ContractError::NotGuardian,
        );
    }

    #[test]
    fn only_one_recovery_at_a_time() {
        let r = rec_env(2, 1);
        let t = &r.t;
        t.client()
            .initiate_recovery(&r.guardians[0], &r.owner, &r.new_owner);
        expect_contract_error(
            t.client()
                .try_initiate_recovery(&r.guardians[1], &r.owner, &Address::generate(&t.env)),
            ContractError::RecoveryInProgress,
        );
        // Guardians cannot be swapped while a recovery is pending.
        expect_contract_error(
            t.client().try_set_guardians(
                &r.owner,
                &vec_of(&t.env, &[Address::generate(&t.env)]),
                &1,
            ),
            ContractError::RecoveryInProgress,
        );
    }

    #[test]
    fn new_owner_must_not_hold_a_folder() {
        let r = rec_env(1, 1);
        let t = &r.t;
        let g = &r.guardians[0];
        let holder = Address::generate(&t.env);
        t.client()
            .claim_folder(&holder, &String::from_str(&t.env, "holder.folder"));
        expect_contract_error(
            t.client().try_initiate_recovery(g, &r.owner, &holder),
            ContractError::FolderAlreadyClaimed,
        );
        expect_contract_error(
            t.client().try_initiate_recovery(g, &r.owner, &r.owner),
            ContractError::FolderAlreadyClaimed,
        );

        // new_owner claims a Folder after initiation: execution is refused.
        t.client().initiate_recovery(g, &r.owner, &r.new_owner);
        t.client()
            .claim_folder(&r.new_owner, &String::from_str(&t.env, "sneaky.folder"));
        wait_delay(t);
        expect_contract_error(
            t.client().try_execute_recovery(&r.owner),
            ContractError::FolderAlreadyClaimed,
        );
        assert_eq!(t.client().get_folder(&r.name), r.owner);
    }

    #[test]
    fn set_guardians_validation() {
        let t = TestEnv::new();
        let owner = Address::generate(&t.env);
        let gs: std::vec::Vec<Address> = (0..=MAX_GUARDIANS)
            .map(|_| Address::generate(&t.env))
            .collect();
        let five = vec_of(&t.env, &gs[..5]);

        expect_contract_error(
            t.client().try_set_guardians(&owner, &five, &1),
            ContractError::FolderNotFound,
        );
        t.client()
            .claim_folder(&owner, &String::from_str(&t.env, "guarded.folder"));

        for (guardians, threshold) in [
            (vec_of(&t.env, &gs), 1),
            (five.clone(), 0),
            (five.clone(), 6),
            (Vec::new(&t.env), 0),
            (vec_of(&t.env, &[gs[0].clone(), gs[0].clone()]), 1),
            (vec_of(&t.env, &[gs[0].clone(), owner.clone()]), 1),
        ] {
            expect_contract_error(
                t.client().try_set_guardians(&owner, &guardians, &threshold),
                ContractError::InvalidGuardians,
            );
        }

        t.client().set_guardians(&owner, &five, &5);
        assert_single_auth(&t, &owner, "set_guardians");
        t.client()
            .set_guardians(&owner, &vec_of(&t.env, &gs[..1]), &1);
        assert_eq!(t.client().get_guardians(&owner).unwrap().guardians.len(), 1);
    }

    #[test]
    fn recovery_auth() {
        let r = rec_env(1, 1);
        let t = &r.t;
        let g = &r.guardians[0];

        t.client().initiate_recovery(g, &r.owner, &r.new_owner);
        assert_single_auth(t, g, "initiate_recovery");

        t.env.set_auths(&[]);
        assert!(t.client().try_cancel_recovery(&r.owner).is_err());
        assert!(t
            .client()
            .try_set_guardians(&r.owner, &vec_of(&t.env, core::slice::from_ref(g)), &1)
            .is_err());
        assert!(t.client().try_approve_recovery(g, &r.owner).is_err());
        // Execution needs no signature at all.
        wait_delay(t);
        t.client().execute_recovery(&r.owner);
        assert!(t.env.auths().is_empty());
        assert_eq!(t.client().get_folder(&r.name), r.new_owner);
    }
}

// ── Portfolio CID verification ──────────────────────────────────────────────

mod verify_cid {
    use super::*;

    const OTHER_CID: &str = "bafkreie33ewzcjvpo3gldpvqgevtoy4oaygkqri3cwxqr6o6lsqc4jqvwu";

    fn s(env: &Env, v: &str) -> String {
        String::from_str(env, v)
    }

    /// `minted_env` owner holds `cidcheck.folder` with VALID_CIDV1_RAW.
    fn name(env: &Env) -> String {
        s(env, "cidcheck.folder")
    }

    #[test]
    fn exact_match_is_true() {
        let (t, user) = minted_env();
        let cid = s(&t.env, VALID_CIDV1_RAW);
        assert!(t.client().verify_portfolio_cid(&user, &cid));
        assert!(t.client().verify_folder_portfolio(&name(&t.env), &cid));
    }

    #[test]
    fn different_cid_is_false() {
        let (t, user) = minted_env();
        let other = s(&t.env, OTHER_CID);
        assert!(!t.client().verify_portfolio_cid(&user, &other));
        assert!(!t.client().verify_folder_portfolio(&name(&t.env), &other));
        // Byte-exact: a different but valid CIDv0 of the same length class
        // does not match either.
        let v0 = s(&t.env, VALID_CIDV0);
        assert!(!t.client().verify_portfolio_cid(&user, &v0));
    }

    #[test]
    fn old_cid_is_false_after_update() {
        let (t, user) = minted_env();
        let old = s(&t.env, VALID_CIDV1_RAW);
        let new = s(&t.env, VALID_CIDV0);
        t.client().update_portfolio(&user, &new);

        assert!(!t.client().verify_portfolio_cid(&user, &old));
        assert!(!t.client().verify_folder_portfolio(&name(&t.env), &old));
        assert!(t.client().verify_portfolio_cid(&user, &new));
        assert!(t.client().verify_folder_portfolio(&name(&t.env), &new));
    }

    #[test]
    fn missing_owner_or_portfolio_is_false() {
        let (t, _) = minted_env();
        let cid = s(&t.env, VALID_CIDV1_RAW);
        let stranger = Address::generate(&t.env);
        assert_eq!(
            t.client().try_verify_portfolio_cid(&stranger, &cid),
            Ok(Ok(false))
        );

        // Claimed a Folder but never minted.
        let unminted = Address::generate(&t.env);
        let unminted_name = s(&t.env, "unminted.folder");
        t.client().claim_folder(&unminted, &unminted_name);
        assert!(!t.client().verify_portfolio_cid(&unminted, &cid));
        assert!(!t.client().verify_folder_portfolio(&unminted_name, &cid));
    }

    #[test]
    fn missing_name_is_false() {
        let (t, _) = minted_env();
        let cid = s(&t.env, VALID_CIDV1_RAW);
        assert_eq!(
            t.client()
                .try_verify_folder_portfolio(&s(&t.env, "nobody.folder"), &cid),
            Ok(Ok(false))
        );
    }

    #[test]
    fn expired_lease_name_is_false() {
        let t = TestEnv::new();
        t.client().set_lease_config(&t.admin, &30, &7);
        let user = Address::generate(&t.env);
        let leased = s(&t.env, "leased.folder");
        let cid = s(&t.env, VALID_CIDV0);
        t.client()
            .register_template(&t.admin, &1u32, &template_uri(&t.env));
        t.client().claim_folder(&user, &leased);
        t.client().mint_portfolio(&user, &1u32, &cid);
        assert!(t.client().verify_folder_portfolio(&leased, &cid));

        t.env
            .ledger()
            .set_timestamp(30 * crate::constants::SECONDS_PER_DAY);
        assert_eq!(
            t.client().try_verify_folder_portfolio(&leased, &cid),
            Ok(Ok(false))
        );
    }

    #[test]
    fn malformed_cid_is_an_error() {
        let (t, user) = minted_env();
        for bad in [
            "",
            "not-a-cid",
            "Qm123",
            "bafy!!!!",
            "QmU3p6RJzZsp4ZTsi6JWeCDxD3JPinVgsZB2uRUS6APEc0",
        ] {
            let bad = s(&t.env, bad);
            expect_contract_error(
                t.client().try_verify_portfolio_cid(&user, &bad),
                ContractError::InvalidCID,
            );
            expect_contract_error(
                t.client().try_verify_folder_portfolio(&name(&t.env), &bad),
                ContractError::InvalidCID,
            );
            // The CID is checked first, even when the owner or name is missing.
            expect_contract_error(
                t.client()
                    .try_verify_portfolio_cid(&Address::generate(&t.env), &bad),
                ContractError::InvalidCID,
            );
            expect_contract_error(
                t.client()
                    .try_verify_folder_portfolio(&s(&t.env, "nobody.folder"), &bad),
                ContractError::InvalidCID,
            );
        }
    }

    #[test]
    fn malformed_folder_name_is_an_error() {
        let (t, _) = minted_env();
        expect_contract_error(
            t.client()
                .try_verify_folder_portfolio(&s(&t.env, "Bad Name"), &s(&t.env, VALID_CIDV0)),
            ContractError::InvalidFolderName,
        );
    }

    #[test]
    fn verification_is_read_only() {
        let (t, user) = minted_env();
        let cid = s(&t.env, VALID_CIDV1_RAW);
        t.env.set_auths(&[]);
        assert!(t.client().verify_portfolio_cid(&user, &cid));
        assert!(t.client().verify_folder_portfolio(&name(&t.env), &cid));
        assert!(t.env.auths().is_empty());
        assert!(t
            .env
            .events()
            .all()
            .filter_by_contract(&t.contract_id)
            .events()
            .is_empty());
    }
}
