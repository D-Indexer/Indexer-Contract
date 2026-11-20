#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::{
        storage::{Instance as _, Persistent as _},
        Address as _, AuthorizedFunction, Events as _, Ledger as _,
    },
    xdr::{self, WriteXdr as _},
    Address, Bytes, BytesN, Env, Event as _, String,
};

use crate::constants::{
    DAY_IN_LEDGERS, INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, MAX_ADMINS, MAX_TEMPLATES,
    PERSISTENT_BUMP_AMOUNT,
};
use crate::errors::ContractError;
use crate::events::{
    AdminAdded, AdminRemoved, ContractInitialized, PortfolioMinted, PortfolioUpdated,
    TemplateRegistered,
};
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

#[test]
fn test_add_and_remove_admin() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);
    let uri = String::from_str(
        &t.env,
        "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety",
    );

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
            seq: 1,
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

    t.client().add_admin(&t.admin, &new_admin);
    assert_eq!(
        t.env.events().all().filter_by_contract(&t.contract_id),
        [AdminAdded {
            admin: t.admin.clone(),
            new_admin: new_admin.clone(),
            seq: 2,
        }
        .to_xdr(&t.env, &t.contract_id)]
    );

    t.client().remove_admin(&new_admin, &t.admin);
    assert_eq!(
        t.env.events().all().filter_by_contract(&t.contract_id),
        [AdminRemoved {
            admin: new_admin.clone(),
            target: t.admin.clone(),
            seq: 3,
        }
        .to_xdr(&t.env, &t.contract_id)]
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
        t.client().add_admin(&t.admin, &Address::generate(&t.env));
    }
    assert_eq!(t.client().get_admins().len(), MAX_ADMINS);

    let res = t
        .client()
        .try_add_admin(&t.admin, &Address::generate(&t.env));
    expect_contract_error(res, ContractError::TooManyAdmins);
}

#[test]
fn test_removed_admin_loses_admin_rights() {
    let t = TestEnv::new();
    let other = Address::generate(&t.env);
    t.client().add_admin(&t.admin, &other);
    t.client().remove_admin(&t.admin, &t.admin);

    let res = t
        .client()
        .try_add_admin(&t.admin, &Address::generate(&t.env));
    expect_contract_error(res, ContractError::Unauthorized);
    let res = t.client().try_remove_admin(&t.admin, &other);
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
            seq: 2,
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
    t.client().add_admin(&t.admin, &other_admin);
    assert_single_auth(&t, &t.admin, "add_admin");
    t.client().remove_admin(&other_admin, &t.admin);
    assert_single_auth(&t, &other_admin, "remove_admin");

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
    assert!(t
        .client()
        .try_add_admin(&t.admin, &Address::generate(&t.env))
        .is_err());
    assert_eq!(
        t.client().get_portfolio(&user).ipfs_cid,
        String::from_str(&t.env, VALID_CIDV1_RAW)
    );
}

// ── Event sequence and digest (#250) ────────────────────────────────────────

const URI: &str = "ipfs://bafkreibtrcmlf7jk35nxe3at537pcpocgdjplvoi37u42yh43acwyegety";
const CID_A: &str = "bafkreiaonkhaxbe63gygjrnclypokwjpij7d5ooskdsca2ooiykh2ahi2q";
const CID_B: &str = "bafkreifwfa7v77kn2om36djjtqiwqykldpady3ytsfbqkqgcrfilhjqtnu";

fn s(t: &TestEnv, v: &str) -> String {
    String::from_str(&t.env, v)
}

/// Registers template 1 and claims a Folder for a fresh user.
fn user_with_folder(t: &TestEnv, label: &str) -> Address {
    if t.client().try_get_template(&1u32).is_err() {
        t.client().register_template(&t.admin, &1u32, &s(t, URI));
    }
    let user = Address::generate(&t.env);
    let mut name = std::string::String::from(label);
    name.push_str(".folder");
    t.client().claim_folder(&user, &s(t, &name));
    user
}

/// Backend-side digest step, recomputed from an emitted event.
fn fold_event(env: &Env, digest: [u8; 32], ev: &xdr::ContractEvent) -> (u64, [u8; 32]) {
    let xdr::ContractEventBody::V0(body) = &ev.body;
    let xdr::ScVal::Vec(Some(items)) = &body.data else {
        panic!("event data is not a vec");
    };
    let xdr::ScVal::U64(seq) = items.last().unwrap() else {
        panic!("last data field is not the u64 seq");
    };
    let topics = xdr::ScVal::Vec(Some(xdr::ScVec(body.topics.clone())));
    let mut preimage = digest.to_vec();
    preimage.extend_from_slice(&seq.to_be_bytes());
    preimage.extend(topics.to_xdr(xdr::Limits::none()).unwrap());
    preimage.extend(body.data.to_xdr(xdr::Limits::none()).unwrap());
    let next = env
        .crypto()
        .sha256(&Bytes::from_slice(env, &preimage))
        .to_array();
    (*seq, next)
}

fn collect(t: &TestEnv, out: &mut std::vec::Vec<xdr::ContractEvent>) {
    out.extend(
        t.env
            .events()
            .all()
            .filter_by_contract(&t.contract_id)
            .events()
            .iter()
            .cloned(),
    );
}

#[test]
fn test_event_cursor_starts_empty() {
    let t = uninitialized_env();
    assert_eq!(
        t.client().get_event_cursor(),
        (0u64, BytesN::from_array(&t.env, &[0; 32]))
    );
}

#[test]
fn test_event_encoding_vector() {
    let t = uninitialized_env();
    t.client().initialize(&t.admin);
    let events = t.env.events().all().filter_by_contract(&t.contract_id);
    let xdr::ContractEventBody::V0(body) = &events.events()[0].body;

    // topics = ScVal::Vec([Symbol("init")])
    let topics = xdr::ScVal::Vec(Some(xdr::ScVec(body.topics.clone())));
    assert_eq!(
        topics.to_xdr(xdr::Limits::none()).unwrap(),
        [
            0, 0, 0, 16, // SCV_VEC
            0, 0, 0, 1, // present
            0, 0, 0, 1, // 1 element
            0, 0, 0, 15, // SCV_SYMBOL
            0, 0, 0, 4, b'i', b'n', b'i', b't',
        ]
    );

    // digest_1 = sha256(32 zero bytes || seq 1 as u64 BE || topics || data)
    let mut preimage = std::vec![0u8; 32];
    preimage.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    preimage.extend(topics.to_xdr(xdr::Limits::none()).unwrap());
    preimage.extend(body.data.to_xdr(xdr::Limits::none()).unwrap());
    let expected = t.env.crypto().sha256(&Bytes::from_slice(&t.env, &preimage));
    assert_eq!(t.client().get_event_cursor(), (1u64, expected.to_bytes()));
}

#[test]
fn test_event_digest_recomputes_over_mixed_sequence() {
    let t = uninitialized_env();
    let mut events = std::vec::Vec::new();
    let mut ops = 0;

    t.client().initialize(&t.admin);
    collect(&t, &mut events);
    ops += 1;
    for id in 1..=7u32 {
        t.client().register_template(&t.admin, &id, &s(&t, URI));
        collect(&t, &mut events);
        ops += 1;
    }
    let second_admin = Address::generate(&t.env);
    t.client().add_admin(&t.admin, &second_admin);
    collect(&t, &mut events);
    ops += 1;
    for i in 0..5 {
        let user = Address::generate(&t.env);
        let name = std::format!("user{i}.folder");
        t.client().claim_folder(&user, &s(&t, &name));
        collect(&t, &mut events);
        t.client().mint_portfolio(&user, &1u32, &s(&t, CID_A));
        collect(&t, &mut events);
        t.client().update_portfolio(&user, &s(&t, CID_B));
        collect(&t, &mut events);
        t.client().set_user_template(&user, &2u32);
        collect(&t, &mut events);
        ops += 4;
    }
    // A failed call emits nothing and must not consume a sequence number.
    assert!(t
        .client()
        .try_register_template(&t.admin, &1u32, &s(&t, URI))
        .is_err());
    t.client().remove_admin(&t.admin, &second_admin);
    collect(&t, &mut events);
    ops += 1;
    assert_eq!(ops, 30);
    assert_eq!(events.len(), 30);

    let mut digest = [0u8; 32];
    for (i, ev) in events.iter().enumerate() {
        let (seq, next) = fold_event(&t.env, digest, ev);
        assert_eq!(seq, i as u64 + 1, "sequence numbers must be gapless");
        digest = next;
    }
    assert_eq!(
        t.client().get_event_cursor(),
        (30u64, BytesN::from_array(&t.env, &digest))
    );
}

// ── Content schemas (#251) ──────────────────────────────────────────────────

#[test]
fn test_legacy_entry_points_default_to_schema_1() {
    let t = TestEnv::new();
    let user = user_with_folder(&t, "legacy");
    assert_eq!(
        t.client().get_template_schemas(&1u32),
        soroban_sdk::vec![&t.env, 1u32]
    );
    t.client().mint_portfolio(&user, &1u32, &s(&t, CID_A));
    assert_eq!(t.client().get_portfolio_schema(&user), 1);

    let res = t
        .client()
        .try_update_portfolio_v2(&user, &s(&t, CID_B), &2u32);
    expect_contract_error(res, ContractError::IncompatibleSchema);
    t.client().update_portfolio(&user, &s(&t, CID_B));
    assert_eq!(t.client().get_portfolio_schema(&user), 1);
}

#[test]
fn test_schema_compatibility_matrix() {
    let t = TestEnv::new();
    let a = user_with_folder(&t, "alice");
    let b = user_with_folder(&t, "bobby");
    t.client().register_template(&t.admin, &2u32, &s(&t, URI));
    t.client()
        .set_template_schemas(&t.admin, &2u32, &soroban_sdk::vec![&t.env, 2u32, 3u32]);

    // (template, schema) -> compatible?
    for (template, schema, ok) in [
        (1u32, 1u32, true),
        (1, 2, false),
        (2, 1, false),
        (2, 2, true),
        (2, 3, true),
    ] {
        let user = user_with_folder(&t, &std::format!("m{template}x{schema}"));
        let res = t
            .client()
            .try_mint_portfolio_v2(&user, &template, &s(&t, CID_A), &schema);
        assert_eq!(res.is_ok(), ok, "mint template {template} schema {schema}");
        if !ok {
            expect_contract_error(res, ContractError::IncompatibleSchema);
        }
    }

    // Legacy mint on a template without schema 1.
    let res = t.client().try_mint_portfolio(&a, &2u32, &s(&t, CID_A));
    expect_contract_error(res, ContractError::IncompatibleSchema);

    t.client().mint_portfolio(&a, &1u32, &s(&t, CID_A));
    t.client()
        .mint_portfolio_v2(&b, &2u32, &s(&t, CID_A), &3u32);
    assert_eq!(t.client().get_portfolio_schema(&b), 3);

    // Update.
    t.client().update_portfolio_v2(&b, &s(&t, CID_B), &2u32);
    assert_eq!(t.client().get_portfolio_schema(&b), 2);
    let res = t.client().try_update_portfolio(&b, &s(&t, CID_A));
    expect_contract_error(res, ContractError::IncompatibleSchema);

    // Template change.
    let res = t.client().try_set_user_template(&a, &2u32);
    expect_contract_error(res, ContractError::IncompatibleSchema);
    let res = t.client().try_set_user_template(&b, &1u32);
    expect_contract_error(res, ContractError::IncompatibleSchema);
    t.client()
        .set_template_schemas(&t.admin, &1u32, &soroban_sdk::vec![&t.env, 1u32, 2u32]);
    t.client().set_user_template(&b, &1u32);
    assert_eq!(t.client().get_portfolio(&b).template_id, 1);
}

#[test]
fn test_template_schema_list_rules() {
    let t = TestEnv::new();
    user_with_folder(&t, "rules");
    let mut nine = soroban_sdk::Vec::new(&t.env);
    for i in 1..=9u32 {
        nine.push_back(i);
    }
    for bad in [
        soroban_sdk::Vec::new(&t.env),
        nine,
        soroban_sdk::vec![&t.env, 0u32],
    ] {
        let res = t.client().try_set_template_schemas(&t.admin, &1u32, &bad);
        expect_contract_error(res, ContractError::InvalidSchemaList);
    }
    let res = t.client().try_set_template_schemas(
        &Address::generate(&t.env),
        &1u32,
        &soroban_sdk::vec![&t.env, 1u32],
    );
    expect_contract_error(res, ContractError::Unauthorized);
    let res =
        t.client()
            .try_set_template_schemas(&t.admin, &9u32, &soroban_sdk::vec![&t.env, 1u32]);
    expect_contract_error(res, ContractError::TemplateNotFound);
}

// ── Portfolio slots (#252) ──────────────────────────────────────────────────

#[test]
fn test_portfolio_slots_legacy_compatibility() {
    let t = TestEnv::new();
    let user = user_with_folder(&t, "slots");
    assert_eq!(t.client().list_portfolio_slots(&user).len(), 0);

    t.client().mint_portfolio(&user, &1u32, &s(&t, CID_A));
    assert_eq!(
        t.client().get_portfolio_slot(&user, &0u32),
        t.client().get_portfolio(&user)
    );
    let res = t
        .client()
        .try_mint_portfolio_slot(&user, &0u32, &1u32, &s(&t, CID_A));
    expect_contract_error(res, ContractError::PortfolioAlreadyMinted);

    t.client()
        .update_portfolio_slot(&user, &0u32, &s(&t, CID_B));
    assert_eq!(t.client().get_portfolio(&user).ipfs_cid, s(&t, CID_B));

    // Slot 0 through the slot API is the legacy portfolio.
    let other = user_with_folder(&t, "other");
    t.client()
        .mint_portfolio_slot(&other, &0u32, &1u32, &s(&t, CID_A));
    assert!(t.client().verify_portfolio(&other));
}

#[test]
fn test_multiple_slots_and_cap() {
    let t = TestEnv::new();
    let user = user_with_folder(&t, "multi");
    for slot in [7u32, 1, 2, 3] {
        t.client()
            .mint_portfolio_slot(&user, &slot, &1u32, &s(&t, CID_A));
    }
    assert!(!t.client().verify_portfolio(&user));
    let res = t
        .client()
        .try_mint_portfolio_slot(&user, &4u32, &1u32, &s(&t, CID_A));
    expect_contract_error(res, ContractError::PortfolioLimitReached);
    let res = t
        .client()
        .try_mint_portfolio_slot(&user, &7u32, &1u32, &s(&t, CID_A));
    expect_contract_error(res, ContractError::PortfolioAlreadyMinted);

    // Slot 0 is always available on top of the extra slots.
    t.client().mint_portfolio(&user, &1u32, &s(&t, CID_B));
    assert_eq!(
        t.client().list_portfolio_slots(&user),
        soroban_sdk::vec![&t.env, 0u32, 7, 1, 2, 3]
    );

    t.client()
        .update_portfolio_slot(&user, &2u32, &s(&t, CID_B));
    assert_eq!(
        t.client().get_portfolio_slot(&user, &2u32).ipfs_cid,
        s(&t, CID_B)
    );
    assert_eq!(
        t.client().get_portfolio_slot(&user, &1u32).ipfs_cid,
        s(&t, CID_A)
    );
    assert_eq!(t.client().get_portfolio(&user).ipfs_cid, s(&t, CID_B));

    let res = t.client().try_get_portfolio_slot(&user, &4u32);
    expect_contract_error(res, ContractError::PortfolioNotFound);
    let res = t
        .client()
        .try_update_portfolio_slot(&user, &4u32, &s(&t, CID_A));
    expect_contract_error(res, ContractError::PortfolioNotFound);
}

#[test]
fn test_portfolio_events_carry_slot() {
    let t = TestEnv::new();
    let user = user_with_folder(&t, "evslot");
    let last = |t: &TestEnv| t.env.events().all().filter_by_contract(&t.contract_id);

    let seq = t.client().get_event_cursor().0 + 1;
    t.client().mint_portfolio(&user, &1u32, &s(&t, CID_A));
    assert_eq!(
        last(&t),
        [PortfolioMinted {
            owner: user.clone(),
            template_id: 1,
            ipfs_cid: s(&t, CID_A),
            slot: 0,
            seq,
        }
        .to_xdr(&t.env, &t.contract_id)]
    );

    t.client()
        .mint_portfolio_slot(&user, &3u32, &1u32, &s(&t, CID_A));
    assert_eq!(
        last(&t),
        [PortfolioMinted {
            owner: user.clone(),
            template_id: 1,
            ipfs_cid: s(&t, CID_A),
            slot: 3,
            seq: seq + 1,
        }
        .to_xdr(&t.env, &t.contract_id)]
    );

    t.client()
        .update_portfolio_slot(&user, &3u32, &s(&t, CID_B));
    assert_eq!(
        last(&t),
        [PortfolioUpdated {
            owner: user.clone(),
            ipfs_cid: s(&t, CID_B),
            slot: 3,
            seq: seq + 2,
        }
        .to_xdr(&t.env, &t.contract_id)]
    );
}

// ── Gated templates (#253) ──────────────────────────────────────────────────

#[test]
fn test_gated_template_matrix() {
    let t = TestEnv::new();
    let user = user_with_folder(&t, "gated");
    t.client().register_template(&t.admin, &2u32, &s(&t, URI));
    t.client().set_template_gated(&t.admin, &2u32, &true);

    // Ungated template behaves as today.
    assert!(t.client().can_use_template(&1u32, &user));
    t.client().set_user_template(&user, &1u32);

    // Gated, not allowlisted.
    assert!(!t.client().can_use_template(&2u32, &user));
    let res = t.client().try_set_user_template(&user, &2u32);
    expect_contract_error(res, ContractError::TemplateNotAllowed);
    let res = t.client().try_mint_portfolio(&user, &2u32, &s(&t, CID_A));
    expect_contract_error(res, ContractError::TemplateNotAllowed);
    let res = t
        .client()
        .try_mint_portfolio_slot(&user, &1u32, &2u32, &s(&t, CID_A));
    expect_contract_error(res, ContractError::TemplateNotAllowed);

    // Gated, allowlisted.
    t.client().allow_template_owner(&t.admin, &2u32, &user);
    assert!(t.client().can_use_template(&2u32, &user));
    t.client().set_user_template(&user, &2u32);
    t.client().mint_portfolio(&user, &2u32, &s(&t, CID_A));

    // Ungating opens the template to everyone.
    let stranger = user_with_folder(&t, "stranger");
    t.client().set_template_gated(&t.admin, &2u32, &false);
    assert!(t.client().can_use_template(&2u32, &stranger));
    t.client().mint_portfolio(&stranger, &2u32, &s(&t, CID_A));

    assert!(!t.client().can_use_template(&9u32, &user));
}

#[test]
fn test_allowlist_removal_keeps_existing_selection() {
    let t = TestEnv::new();
    let user = user_with_folder(&t, "revoked");
    t.client().register_template(&t.admin, &2u32, &s(&t, URI));
    t.client().set_template_gated(&t.admin, &2u32, &true);
    t.client().allow_template_owner(&t.admin, &2u32, &user);
    t.client().set_user_template(&user, &2u32);

    t.client().revoke_template_owner(&t.admin, &2u32, &user);
    assert!(!t.client().can_use_template(&2u32, &user));
    assert_eq!(t.client().get_user_template(&user), Some(2));
    // Content updates still work; selecting the template again does not.
    let res = t.client().try_mint_portfolio(&user, &2u32, &s(&t, CID_A));
    expect_contract_error(res, ContractError::TemplateNotAllowed);
    t.client().mint_portfolio(&user, &1u32, &s(&t, CID_A));
    let res = t.client().try_set_user_template(&user, &2u32);
    expect_contract_error(res, ContractError::TemplateNotAllowed);
}

#[test]
fn test_gating_admin_rules() {
    let t = TestEnv::new();
    let user = user_with_folder(&t, "rulez");
    let stranger = Address::generate(&t.env);
    expect_contract_error(
        t.client().try_set_template_gated(&stranger, &1u32, &true),
        ContractError::Unauthorized,
    );
    expect_contract_error(
        t.client().try_allow_template_owner(&stranger, &1u32, &user),
        ContractError::Unauthorized,
    );
    expect_contract_error(
        t.client()
            .try_revoke_template_owner(&stranger, &1u32, &user),
        ContractError::Unauthorized,
    );
    expect_contract_error(
        t.client().try_set_template_gated(&t.admin, &9u32, &true),
        ContractError::TemplateNotFound,
    );
    expect_contract_error(
        t.client().try_allow_template_owner(&t.admin, &9u32, &user),
        ContractError::TemplateNotFound,
    );
}
