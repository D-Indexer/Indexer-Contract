#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::{
        storage::{Instance as _, Persistent as _},
        Address as _, AuthorizedFunction, Events as _, Ledger as _,
    },
    Address, BytesN, Env, Event as _, IntoVal, String, Val, Vec,
};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::xdr::ToXdr;

use crate::constants::{
    DAY_IN_LEDGERS, INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, MAX_ADMINS, MAX_TEMPLATES,
    PERSISTENT_BUMP_AMOUNT,
};use crate::errors::ContractError;
use crate::events::{AdminAdded, AdminRemoved, ContractInitialized, TemplateRegistered};
use crate::storage;
use crate::types::DataKey;
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
    let name = String::from_str(&t.env, "ab.folder");

    let res = t.client().try_claim_folder(&user, &name);
    expect_contract_error(res, ContractError::FolderNameTooShort);
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
    expect_contract_error(res, ContractError::FolderNameTooLong);
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
fn test_punycode_folder_name_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "xn--abc.folder");

    let res = t.client().try_claim_folder(&user, &name);
    expect_contract_error(res, ContractError::InvalidFolderName);
}

#[test]
fn test_double_hyphen_folder_name_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "ab--c.folder");

    let res = t.client().try_claim_folder(&user, &name);
    expect_contract_error(res, ContractError::InvalidFolderName);
}

#[test]
fn test_single_hyphen_folder_name_accepted() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "a-b-c.folder");

    t.client().claim_folder(&user, &name);
    assert_eq!(t.client().get_folder(&name), user);
}

#[test]
fn test_numeric_only_folder_name_rejected() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "007.folder");

    let res = t.client().try_claim_folder(&user, &name);
    expect_contract_error(res, ContractError::InvalidFolderName);
}

#[test]
fn test_alphanumeric_folder_name_accepted() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "agent007.folder");

    t.client().claim_folder(&user, &name);
    assert_eq!(t.client().get_folder(&name), user);
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
    }]);
    t.env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: t.env.ledger().timestamp(),
        protocol_version: t.env.ledger().protocol_version(),
        sequence_number: t.env.ledger().sequence(),
        network_id: soroban_sdk::BytesN::from_array(&t.env, &[0u8; 32]).into(),
        base_reserve: 0,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: live_until_ledger,
    });
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
fn test_template_hash_registration_and_lookup() {
    let t = TestEnv::new();
    let uri = template_uri(&t.env);
    let digest = BytesN::from_array(&t.env, &[42; 32]);

    t.client()
        .register_template_with_hash(&t.admin, &1, &uri, &digest);

    assert_eq!(t.client().get_template_hash(&1), Some(digest.clone()));
    assert_eq!(t.client().get_template_hash(&2), None);
    assert_eq!(
        t.env.events().all().filter_by_contract(&t.contract_id),
        [
            TemplateRegistered {
                template_id: 1,
                metadata_uri: uri,
                creator: t.admin.clone(),
            }
            .to_xdr(&t.env, &t.contract_id),
            TemplateHashRegistered {
                template_id: 1,
                content_sha256: digest,
            }
            .to_xdr(&t.env, &t.contract_id),
        ]
    );
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
fn test_list_templates_page_covers_registry_without_overlap() {
    let t = TestEnv::new();
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    for id in 1..=60 {
        t.client().register_template(&t.admin, &id, &uri);
    }

    let first = t.client().list_templates_page(&0, &25);
    let second = t.client().list_templates_page(&25, &25);
    let third = t.client().list_templates_page(&50, &25);

    assert_eq!(first.len(), 25);
    assert_eq!(second.len(), 25);
    assert_eq!(third.len(), 10);
    for (offset, template) in first.iter().chain(second.iter()).chain(third.iter()).enumerate() {
        assert_eq!(template.template_id, offset as u32 + 1);
    }
}

#[test]
fn test_list_templates_page_boundaries_and_limits() {
    let t = TestEnv::new();
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );
    for id in 1..=3 {
        t.client().register_template(&t.admin, &id, &uri);
    }

    assert_eq!(t.client().list_templates_page(&3, &1).len(), 0);
    assert_eq!(t.client().list_templates_page(&4, &1).len(), 0);
    assert_eq!(t.client().list_templates_page(&0, &0).len(), 0);
    assert_eq!(t.client().list_templates_page(&0, &u32::MAX).len(), 3);
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
    let key = DataKey::Template(1u32);
    archive_persistent(&t, &key);

    t.env.storage().persistent().set(
        &key,
        &crate::types::Template {
            template_id: 1u32,
            metadata_uri: uri.clone(),
            creator: t.admin.clone(),
        },
    );

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

    let key = DataKey::Portfolio(user.clone());
    archive_persistent(&t, &key);

    t.env.storage().persistent().set(
        &key,
        &crate::types::PortfolioNFT {
            owner: user.clone(),
            template_id: 1u32,
            ipfs_cid: cid.clone(),
            minted_at: 0,
        },
    );

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
    let ttl = t.persistent_ttl(&storage::folder_owner_key(&t.env, &name));
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
        t.persistent_ttl(&storage::folder_owner_key(&t.env, &name)),
        PERSISTENT_BUMP_AMOUNT - 10 * DAY_IN_LEDGERS
    );
}

#[test]
fn test_extend_owner_ttl_extends_all_owner_keys() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    let name = String::from_str(&t.env, "ttl-owner.folder");
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37y42yh43acwyegety",
    );
    let cid = String::from_str(
        &t.env,
        "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG",
    );

    t.client().register_template(&t.admin, &1u32, &uri);
    t.client().claim_folder(&user, &name);
    t.client().set_user_template(&user, &1u32);
    t.client().mint_portfolio(&user, &1u32, &cid);

    let keys = [
        DataKey::OwnerFolder(user.clone()),
        DataKey::FolderOwner(name.clone()),
        DataKey::UserTemplate(user.clone()),
        DataKey::Portfolio(user.clone()),
    ];

    t.advance_ledgers(40 * DAY_IN_LEDGERS);
    for key in keys.iter() {
        assert!(t.persistent_ttl(key) < PERSISTENT_BUMP_AMOUNT);
    }

    let extended = t.client().extend_owner_ttl(&user);
    assert_eq!(extended, 4);
    for key in keys.iter() {
        assert_eq!(t.persistent_ttl(key), PERSISTENT_BUMP_AMOUNT);
    }

    // Data is untouched.
    assert_eq!(t.client().get_identity(&user), name);
    assert_eq!(t.client().get_user_template(&user), Some(1u32));
    assert_eq!(t.client().get_portfolio(&user).ipfs_cid, cid);
}

#[test]
fn test_extend_owner_ttl_skips_missing_keys() {
    let t = TestEnv::new();
    let user = Address::generate(&t.env);
    assert_eq!(t.client().extend_owner_ttl(&user), 0);

    let name = String::from_str(&t.env, "ttl-partial.folder");
    t.client().claim_folder(&user, &name);
    t.advance_ledgers(40 * DAY_IN_LEDGERS);
    assert_eq!(t.client().extend_owner_ttl(&user), 2);
    assert_eq!(
        t.persistent_ttl(&DataKey::OwnerFolder(user.clone())),
        PERSISTENT_BUMP_AMOUNT
    );
    assert_eq!(
        t.persistent_ttl(&DataKey::FolderOwner(name)),
        PERSISTENT_BUMP_AMOUNT
    );
}

#[test]
fn test_extend_template_ttl_extends_template_keys() {
    let t = TestEnv::new();
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37y42yh43acwyegety",
    );
    t.client().register_template(&t.admin, &1u32, &uri);

    t.advance_ledgers(40 * DAY_IN_LEDGERS);
    assert!(t.persistent_ttl(&DataKey::Template(1)) < PERSISTENT_BUMP_AMOUNT);

    assert_eq!(t.client().extend_template_ttl(&1u32), 2);
    assert_eq!(
        t.persistent_ttl(&DataKey::Template(1)),
        PERSISTENT_BUMP_AMOUNT
    );
    assert_eq!(
        t.persistent_ttl(&DataKey::TemplateList),
        PERSISTENT_BUMP_AMOUNT
    );

    // Missing template is skipped without an error.
    assert_eq!(t.client().extend_template_ttl(&404u32), 1);
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

// ── Golden event fixtures ───────────────────────────────────────────────────

/// Fixed addresses keep the golden XDR fixtures deterministic across runs.
const GOLDEN_ADMIN: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
const GOLDEN_OWNER: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";
const GOLDEN_NEW_ADMIN: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACZ7M";

fn golden_env() -> (Env, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(FolderContract, ());
    let admin = Address::from_str(&env, GOLDEN_ADMIN);
    let owner = Address::from_str(&env, GOLDEN_OWNER);
    let new_admin = Address::from_str(&env, GOLDEN_NEW_ADMIN);
    (env, contract_id, admin, owner, new_admin)
}

fn golden_cid(env: &Env) -> String {
    String::from_str(
        env,
        "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG",
    )
}

fn golden_uri(env: &Env) -> String {
    String::from_str(
        env,
        "ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG/1.json",
    )
}

/// Triggers every contract event and returns `(topic, topics_xdr_b64, data_xdr_b64)`.
fn golden_events() -> std::vec::Vec<(std::string::String, std::string::String, std::string::String)> {
    let (env, contract_id, admin, owner, new_admin) = golden_env();
    let client = FolderContractClient::new(&env, &contract_id);
    let cid = golden_cid(&env);
    let uri = golden_uri(&env);

    client.initialize(&admin);
    client.add_admin(&admin, &new_admin);
    client.remove_admin(&admin, &new_admin);
    client.claim_folder(&owner, &String::from_str(&env, "golden.folder"));
    client.register_template(&admin, &1, &uri);
    client.set_user_template(&owner, &1);
    client.mint_portfolio(&owner, &1, &cid);
    client.update_portfolio(&owner, &cid);

    let mut out = std::vec::Vec::new();
    for (topics, data) in env.events().all().filter_by_contract(&contract_id) {
        let topic = topics.get(0).unwrap();
        let topic_str = match topic {
            soroban_sdk::xdr::ScVal::Symbol(sym) => {
                std::string::String::from_utf8(sym.0.to_vec()).unwrap()
            }
            _ => panic!("unexpected topic type"),
        };
        out.push((
            topic_str,
            base64_encode(&topics.to_xdr(&env)),
            base64_encode(&data.to_xdr(&env)),
        ));
    }
    out
}

fn base64_encode(bytes: &soroban_sdk::Bytes) -> std::string::String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = std::string::String::new();
    let mut chunk = [0u8; 3];
    let mut n = 0;
    for byte in bytes.iter() {
        chunk[n] = byte;
        n += 1;
        if n == 3 {
            out.push(ALPHABET[(chunk[0] >> 2) as usize] as char);
            out.push(ALPHABET[(((chunk[0] & 0x03) << 4) | (chunk[1] >> 4)) as usize] as char);
            out.push(ALPHABET[(((chunk[1] & 0x0f) << 2) | (chunk[2] >> 6)) as usize] as char);
            out.push(ALPHABET[(chunk[2] & 0x3f) as usize] as char);
            n = 0;
        }
    }
    if n == 1 {
        out.push(ALPHABET[(chunk[0] >> 2) as usize] as char);
        out.push(ALPHABET[((chunk[0] & 0x03) << 4) as usize] as char);
        out.push('=');
        out.push('=');
    } else if n == 2 {
        out.push(ALPHABET[(chunk[0] >> 2) as usize] as char);
        out.push(ALPHABET[(((chunk[0] & 0x03) << 4) | (chunk[1] >> 4)) as usize] as char);
        out.push(ALPHABET[((chunk[1] & 0x0f) << 2) as usize] as char);
        out.push('=');
    }
    out
}

fn fixture_path(topic: &str) -> std::string::String {
    std::format!("{}/fixtures/events/{}.json", env!("CARGO_MANIFEST_DIR"), topic)
}

fn fixture_json(topics_b64: &str, data_b64: &str, decoded: &str) -> std::string::String {
    std::format!(
        "{{\n  \"topics_xdr_b64\": \"{}\",\n  \"data_xdr_b64\": \"{}\",\n  \"decoded\": {}\n}}\n",
        topics_b64,
        data_b64,
        decoded
    )
}

/// Regenerates `fixtures/events/<topic>.json`. Run with
/// `UPDATE_EVENT_FIXTURES=1 cargo test golden_event_fixtures` to refresh them.
#[test]
fn golden_event_fixtures() {
    let update = std::env::var("UPDATE_EVENT_FIXTURES").is_ok();
    let events = golden_events();
    assert_eq!(events.len(), 8, "every event topic must be covered");

    for (topic, topics_b64, data_b64) in events {
        let decoded = std::format!(
            "{{\"topics_xdr_b64\": \"{}\", \"data_xdr_b64\": \"{}\"}}",
            topics_b64, data_b64
        );
        let expected = fixture_json(&topics_b64, &data_b64, &decoded);
        let path = fixture_path(&topic);
        if update {
            std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
            std::fs::write(&path, expected.as_bytes()).unwrap();
        } else {
            let actual = std::fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("missing fixture {path}; run UPDATE_EVENT_FIXTURES=1"));
            assert_eq!(actual, expected, "fixture drift for {topic}");
        }
    }
}

/// Round-trips every committed fixture back through XDR decoding.
#[test]
fn golden_event_fixtures_round_trip() {
    let env = Env::default();
    let dir = std::format!("{}/fixtures/events", env!("CARGO_MANIFEST_DIR"));
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path).unwrap();
        let topics_b64 = extract_json_field(&raw, "topics_xdr_b64");
        let data_b64 = extract_json_field(&raw, "data_xdr_b64");
        let topics = soroban_sdk::xdr::ScVal::from_xdr_base64(&env, &topics_b64).unwrap();
        let data = soroban_sdk::xdr::ScVal::from_xdr_base64(&env, &data_b64).unwrap();
        assert_eq!(topics.to_xdr_base64(&env), topics_b64);
        assert_eq!(data.to_xdr_base64(&env), data_b64);
        count += 1;
    }
    assert_eq!(count, 8, "expected one fixture per event topic");
}

fn extract_json_field(raw: &str, field: &str) -> std::string::String {
    let key = std::format!("\"{field}\": \"");
    let start = raw.find(&key).unwrap_or_else(|| panic!("missing {field}")) + key.len();
    let rest = &raw[start..];
    let end = rest.find('"').unwrap();
    std::string::String::from(&rest[..end])
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
/// Real CIDv0 vectors (sha2-256 multihash, base58btc).
const VALID_CIDV0_VECTORS: [&str; 5] = [
    "QmU3p6RJzZsp4ZTsi6JWeCDxD3JPinVgsZB2uRUS6APEce",
    "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG",
    "QmbFMke1KXqnYyBBWxB74N4c5SBnJMVAiMNRcGu6x1AwQH",
    "QmT78zSuBmuS4z925WZfrqQ1qHaJ56DQaTfyMUF7F8ff5o",
    "QmZULkCELmmk5XNfCgTnCyFgAVxBRBXyDHGGMVoLFLiXEN",
];

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
fn test_cidv0_multihash_vectors() {
    let (t, user) = minted_env();
    for good in VALID_CIDV0_VECTORS {
        let cid = String::from_str(&t.env, good);
        t.client().update_portfolio(&user, &cid);
        assert_eq!(t.client().get_portfolio(&user).ipfs_cid, cid);
    }
    // Corrupted vectors: changed digest byte, wrong multihash prefix, and
    // characters outside the base58btc alphabet.
    let mut corrupted: std::vec::Vec<std::string::String> = std::vec::Vec::new();
    for good in VALID_CIDV0_VECTORS {
        let mut chars: std::vec::Vec<char> = good.chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == 'a' { 'b' } else { 'a' };
        corrupted.push(chars.into_iter().collect());
    }
    // Decodes to a multihash whose function code is not sha2-256.
    corrupted.push(std::string::String::from(
        "QmT78zSuBmuS4z925WZfrqQ1qHaJ56DQaTfyMUF7F8ff5p",
    ));
    // Decodes to a multihash whose digest length byte is not 0x20.
    corrupted.push(std::string::String::from(
        "QmT78zSuBmuS4z925WZfrqQ1qHaJ56DQaTfyMUF7F8ff5n",
    ));
    // Invalid base58btc characters (`0`, `O`, `I`, `l`).
    corrupted.push(std::string::String::from("Qm") + &"0".repeat(44));
    corrupted.push(std::string::String::from("Qm") + &"O".repeat(44));
    corrupted.push(std::string::String::from("Qm") + &"I".repeat(44));
    corrupted.push(std::string::String::from("Qm") + &"l".repeat(44));
    for bad in corrupted {
        let res = t
            .client()
            .try_update_portfolio(&user, &String::from_str(&t.env, &bad));
        expect_contract_error(res, ContractError::InvalidCID);
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
    t.client().add_admin(&t.admin, &other_admin);
    assert_single_auth(&t, &t.admin, "execute_admin_change");
    t.client().remove_admin(&other_admin, &t.admin);
    assert_single_auth(&t, &other_admin, "remove_admin");
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

#[test]
fn test_address_blocklist_write_paths_and_unblock() {
    let t = TestEnv::new();
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia/template.json",
    );
    t.client().register_template(&t.admin, &1u32, &uri);

    let user = Address::generate(&t.env);
    t.client().block_address(&t.admin, &user, &7u32);
    expect_contract_error(
        t.client()
            .try_claim_folder(&user, &String::from_str(&t.env, "blocked.folder")),
        ContractError::AddressBlocked,
    );
    t.client().unblock_address(&t.admin, &user);
    t.client()
        .claim_folder(&user, &String::from_str(&t.env, "blocked.folder"));

    t.client().block_address(&t.admin, &user, &7u32);
    expect_contract_error(
        t.client()
            .try_mint_portfolio(&user, &1u32, &String::from_str(&t.env, VALID_CIDV0)),
        ContractError::AddressBlocked,
    );
    t.client().unblock_address(&t.admin, &user);
    t.client()
        .mint_portfolio(&user, &1u32, &String::from_str(&t.env, VALID_CIDV0));

    t.client().block_address(&t.admin, &user, &7u32);
    expect_contract_error(
        t.client()
            .try_update_portfolio(&user, &String::from_str(&t.env, VALID_CIDV1_RAW)),
        ContractError::AddressBlocked,
    );
    t.client().unblock_address(&t.admin, &user);
    t.client()
        .update_portfolio(&user, &String::from_str(&t.env, VALID_CIDV1_RAW));

    t.client().block_address(&t.admin, &user, &7u32);
    expect_contract_error(
        t.client().try_set_user_template(&user, &1u32),
        ContractError::AddressBlocked,
    );
    t.client().unblock_address(&t.admin, &user);
    t.client().set_user_template(&user, &1u32);

    expect_contract_error(
        t.client().try_block_address(&t.admin, &t.admin, &1u32),
        ContractError::CannotBlockAdmin,
    );
}

#[test]
fn test_portfolio_flag_set_clear_and_persist_across_update() {
    let t = TestEnv::new();
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia/template.json",
    );
    t.client().register_template(&t.admin, &1u32, &uri);
    let owner = Address::generate(&t.env);
    t.client()
        .claim_folder(&owner, &String::from_str(&t.env, "flagged.folder"));
    t.client()
        .mint_portfolio(&owner, &1u32, &String::from_str(&t.env, VALID_CIDV0));

    t.client().set_portfolio_flag(&t.admin, &owner, &1u32, &9u32);
    assert_eq!(t.client().get_portfolio_flag(&owner), 1u32);
    t.client()
        .update_portfolio(&owner, &String::from_str(&t.env, VALID_CIDV1_RAW));
    assert_eq!(t.client().get_portfolio_flag(&owner), 1u32);

    t.client().set_portfolio_flag(&t.admin, &owner, &0u32, &10u32);
    assert_eq!(t.client().get_portfolio_flag(&owner), 0u32);
    expect_contract_error(
        t.client().try_set_portfolio_flag(
            &Address::generate(&t.env),
            &owner,
            &2u32,
            &1u32,
        ),
        ContractError::Unauthorized,
    );
}

#[test]
fn test_template_submission_lifecycle_cap_and_usage() {
    let t = TestEnv::new();
    let designer = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia/template.json",
    );
    let submission_id = t.client().submit_template(&designer, &uri);
    t.client()
        .approve_submission(&t.admin, &submission_id, &1u32);
    assert_eq!(
        t.client().get_template_designer(&1u32),
        Some(designer.clone())
    );
    let duplicate_id = t.client().submit_template(
        &designer,
        &String::from_str(
            &t.env,
            "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety/duplicate.json",
        ),
    );
    expect_contract_error(
        t.client()
            .try_approve_submission(&t.admin, &duplicate_id, &1u32),
        ContractError::TemplateAlreadyExists,
    );

    let rejected_id = t
        .client()
        .submit_template(&designer, &String::from_str(
            &t.env,
            "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety/template.json",
        ));
    t.client().reject_submission(&t.admin, &rejected_id, &3u32);
    expect_contract_error(
        t.client()
            .try_approve_submission(&t.admin, &rejected_id, &2u32),
        ContractError::SubmissionNotFound,
    );

    for i in 0u32..49u32 {
        let uri = String::from_str(
            &t.env,
            &std::format!(
                "ipfs://bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia/{}.json",
                i
            ),
        );
        t.client().submit_template(&designer, &uri);
    }
    expect_contract_error(
        t.client().try_submit_template(
            &designer,
            &String::from_str(
                &t.env,
                "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety/cap.json",
            ),
        ),
        ContractError::SubmissionQueueFull,
    );

    let owner1 = Address::generate(&t.env);
    let owner2 = Address::generate(&t.env);
    let owner3 = Address::generate(&t.env);
    for (owner, name) in [
        (owner1.clone(), "usage1.folder"),
        (owner2.clone(), "usage2.folder"),
        (owner3.clone(), "usage3.folder"),
    ] {
        t.client()
            .claim_folder(&owner, &String::from_str(&t.env, name));
        t.client().set_user_template(&owner, &1u32);
    }
    assert_eq!(t.client().get_template_usage(&1u32), 3u32);
    t.client().set_user_template(&owner1, &1u32);
    assert_eq!(t.client().get_template_usage(&1u32), 3u32);

    let uri2 = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety/second.json",
    );
    t.client().register_template(&t.admin, &2u32, &uri2);
    t.client().set_user_template(&owner1, &2u32);
    assert_eq!(t.client().get_template_usage(&1u32), 2u32);
    assert_eq!(t.client().get_template_usage(&2u32), 1u32);
    t.client().set_user_template(&owner1, &2u32);
    assert_eq!(t.client().get_template_usage(&1u32), 2u32);
    assert_eq!(t.client().get_template_usage(&2u32), 1u32);
}
