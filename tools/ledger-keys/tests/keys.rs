use indexer_contract::{FolderContract, FolderContractClient, TemplatePrice};
use ledger_keys::{ledger_key, ledger_key_base64, Entry};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::xdr::{LedgerKey, Limits, ReadXdr};
use soroban_sdk::{Address, Env, String};

const CID: &str = "bafkreiejxg6d3gqvi3737mcwxkv4u7gismaraxgv5zfoqtfuc3nbdgve7i";

fn strkey(a: &Address) -> std::string::String {
    a.to_string().to_string()
}

/// Every key the tool prints must name an entry the contract actually wrote.
#[test]
fn tool_keys_match_entries_written_by_the_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(FolderContract, ());
    let client = FolderContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin);
    client.set_treasury(&admin, &admin);
    let uri = String::from_str(&env, &format!("ipfs://{CID}"));
    client.register_template(&admin, &7u32, &uri);
    let token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let price = TemplatePrice {
        token: token.clone(),
        amount: 1,
        designer: admin.clone(),
        designer_bps: 0,
    };
    client.set_template_price(&admin, &7u32, &price);
    client.claim_folder(&user, &String::from_str(&env, "keys.folder"));
    soroban_sdk::token::StellarAssetClient::new(&env, &token).mint(&user, &1);
    client.mint_portfolio(&user, &7u32, &String::from_str(&env, CID));

    let written: Vec<LedgerKey> = env
        .to_snapshot()
        .ledger
        .ledger_entries
        .into_iter()
        .map(|(key, _)| *key)
        .collect();

    let contract = strkey(&id);
    let owner = strkey(&user);
    let entries = [
        Entry::Instance,
        Entry::FolderOwner("keys.folder".into()),
        Entry::OwnerFolder(owner.clone()),
        Entry::Template(7),
        Entry::TemplateList,
        Entry::TemplatePrice(7),
        Entry::UserTemplate(owner.clone()),
        Entry::Portfolio(owner),
    ];
    for entry in &entries {
        let printed = ledger_key_base64(&contract, entry);
        let decoded = LedgerKey::from_xdr_base64(&printed, Limits::none()).unwrap();
        assert_eq!(decoded, ledger_key(&env, &id, entry));
        assert!(written.contains(&decoded), "no entry for {printed}");
    }
}

#[test]
fn parse_rejects_unknown_and_missing_args() {
    assert!(Entry::parse("bogus", None).is_err());
    assert!(Entry::parse("template", None).is_err());
    assert!(Entry::parse("template", Some("x")).is_err());
    assert!(Entry::parse("template-list", None).is_ok());
}
