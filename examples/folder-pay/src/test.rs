#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::Address as _, token, Address, Env, String,
};

use indexer_contract::{FolderContract, FolderContractClient};

use crate::{FolderPay, FolderPayClient, FolderPayError};

struct Setup {
    env: Env,
    resolver: Address,
    token: Address,
    payer: Address,
    owner: Address,
    pay: Address,
}

fn setup() -> Setup {
    let env = Env::default();
    env.mock_all_auths();

    let resolver = env.register(FolderContract, ());
    let admin = Address::generate(&env);
    FolderContractClient::new(&env, &resolver).initialize(&admin);

    let owner = Address::generate(&env);
    FolderContractClient::new(&env, &resolver).claim_folder(&owner, &String::from_str(&env, "bob.folder"));

    let payer = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token = sac.address();
    token::StellarAssetClient::new(&env, &token).mint(&payer, &1_000);

    let pay = env.register(FolderPay, ());
    Setup {
        env,
        resolver,
        token,
        payer,
        owner,
        pay,
    }
}

#[test]
fn resolve_existing_and_missing_names() {
    let s = setup();
    let client = FolderPayClient::new(&s.env, &s.pay);

    assert_eq!(
        client.resolve(&s.resolver, &String::from_str(&s.env, "bob.folder")),
        Some(s.owner.clone())
    );
    assert_eq!(
        client.resolve(&s.resolver, &String::from_str(&s.env, "nobody.folder")),
        None
    );
}

#[test]
fn reverse_existing_and_missing_owners() {
    let s = setup();
    let client = FolderPayClient::new(&s.env, &s.pay);

    assert_eq!(
        client.reverse(&s.resolver, &s.owner),
        Some(String::from_str(&s.env, "bob.folder"))
    );
    assert_eq!(client.reverse(&s.resolver, &Address::generate(&s.env)), None);
}

#[test]
fn pays_token_to_resolved_name() {
    let s = setup();
    let client = FolderPayClient::new(&s.env, &s.pay);
    let token_client = token::Client::new(&s.env, &s.token);

    let recipient = client.pay(
        &s.resolver,
        &s.token,
        &s.payer,
        &String::from_str(&s.env, "bob.folder"),
        &250,
    );

    assert_eq!(recipient, s.owner);
    assert_eq!(token_client.balance(&s.owner), 250);
    assert_eq!(token_client.balance(&s.payer), 750);
}

#[test]
fn pay_unknown_name_fails() {
    let s = setup();
    let client = FolderPayClient::new(&s.env, &s.pay);

    let result = client.try_pay(
        &s.resolver,
        &s.token,
        &s.payer,
        &String::from_str(&s.env, "nobody.folder"),
        &250,
    );

    assert_eq!(result, Err(Ok(FolderPayError::NameNotFound)));
}
