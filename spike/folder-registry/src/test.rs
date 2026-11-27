extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::{FolderRegistry, FolderRegistryClient, RegistryError};

fn setup() -> (Env, FolderRegistryClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(FolderRegistry, ());
    let client = FolderRegistryClient::new(&env, &id);
    (env, client)
}

#[test]
fn claim_and_resolve() {
    let (env, c) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "alice.folder");
    c.claim_folder(&owner, &name);
    assert_eq!(c.get_folder(&name), owner);
    assert_eq!(c.get_identity(&owner), name);
}

#[test]
fn name_taken_and_one_name_per_owner() {
    let (env, c) = setup();
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let name = String::from_str(&env, "alice.folder");
    c.claim_folder(&a, &name);
    assert_eq!(
        c.try_claim_folder(&b, &name).err(),
        Some(Ok(RegistryError::FolderNameTaken))
    );
    assert_eq!(
        c.try_claim_folder(&a, &String::from_str(&env, "other.folder"))
            .err(),
        Some(Ok(RegistryError::FolderAlreadyClaimed))
    );
}

#[test]
fn invalid_names_rejected() {
    let (env, c) = setup();
    let owner = Address::generate(&env);
    for bad in [
        "ab.folder",
        "alice",
        "-bob.folder",
        "bob-.folder",
        "Bob.folder",
        "a_b.folder",
    ] {
        assert_eq!(
            c.try_claim_folder(&owner, &String::from_str(&env, bad))
                .err(),
            Some(Ok(RegistryError::InvalidFolderName)),
            "{bad}"
        );
    }
}

#[test]
fn unknown_lookups_fail() {
    let (env, c) = setup();
    assert_eq!(
        c.try_get_folder(&String::from_str(&env, "none.folder"))
            .err(),
        Some(Ok(RegistryError::FolderNotFound))
    );
    assert_eq!(
        c.try_get_identity(&Address::generate(&env)).err(),
        Some(Ok(RegistryError::FolderNotFound))
    );
}
