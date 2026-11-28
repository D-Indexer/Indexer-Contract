extern crate std;

use folder_registry::{FolderRegistry, FolderRegistryClient};
use folder_templates::{FolderTemplates, FolderTemplatesClient};
use indexer_contract::{FolderContract, FolderContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::{FolderPortfolio, FolderPortfolioClient, PortfolioError};

const URI: &str =
    "ipfs://bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia/template.json";
const CID: &str = "QmU3p6RJzZsp4ZTsi6JWeCDxD3JPinVgsZB2uRUS6APEce";
const CID2: &str = "bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia";

struct Split {
    env: Env,
    registry: FolderRegistryClient<'static>,
    portfolio: FolderPortfolioClient<'static>,
}

fn setup() -> Split {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let registry_id = env.register(FolderRegistry, ());
    let templates_id = env.register(FolderTemplates, ());
    let portfolio_id = env.register(FolderPortfolio, ());
    let templates = FolderTemplatesClient::new(&env, &templates_id);
    templates.initialize(&admin);
    templates.register_template(&admin, &1, &String::from_str(&env, URI));
    templates.register_template(&admin, &2, &String::from_str(&env, URI));
    let portfolio = FolderPortfolioClient::new(&env, &portfolio_id);
    portfolio.initialize(&admin, &registry_id, &templates_id);
    Split {
        registry: FolderRegistryClient::new(&env, &registry_id),
        portfolio,
        env,
    }
}

impl Split {
    fn user_with_folder(&self, name: &str) -> Address {
        let user = Address::generate(&self.env);
        self.registry
            .claim_folder(&user, &String::from_str(&self.env, name));
        user
    }
}

#[test]
fn mint_update_and_verify() {
    let s = setup();
    let user = s.user_with_folder("alice.folder");
    assert!(!s.portfolio.verify_portfolio(&user));
    s.portfolio
        .mint_portfolio(&user, &1, &String::from_str(&s.env, CID));
    assert!(s.portfolio.verify_portfolio(&user));
    s.portfolio
        .update_portfolio(&user, &String::from_str(&s.env, CID2));
    assert_eq!(
        s.portfolio.get_portfolio(&user).ipfs_cid,
        String::from_str(&s.env, CID2)
    );
    s.portfolio.set_user_template(&user, &2);
    assert_eq!(s.portfolio.get_portfolio(&user).template_id, 2);
}

#[test]
fn mint_checks_registry_and_templates() {
    let s = setup();
    let cid = String::from_str(&s.env, CID);
    let stranger = Address::generate(&s.env);
    assert_eq!(
        s.portfolio.try_mint_portfolio(&stranger, &1, &cid).err(),
        Some(Ok(PortfolioError::FolderNotFound))
    );
    let user = s.user_with_folder("alice.folder");
    assert_eq!(
        s.portfolio.try_mint_portfolio(&user, &9, &cid).err(),
        Some(Ok(PortfolioError::TemplateNotFound))
    );
    assert_eq!(
        s.portfolio
            .try_mint_portfolio(&user, &1, &String::from_str(&s.env, "short"))
            .err(),
        Some(Ok(PortfolioError::InvalidCID))
    );
    s.portfolio.mint_portfolio(&user, &1, &cid);
    assert_eq!(
        s.portfolio.try_mint_portfolio(&user, &1, &cid).err(),
        Some(Ok(PortfolioError::PortfolioAlreadyMinted))
    );
}

#[test]
fn update_without_portfolio_fails() {
    let s = setup();
    let user = s.user_with_folder("alice.folder");
    assert_eq!(
        s.portfolio
            .try_update_portfolio(&user, &String::from_str(&s.env, CID))
            .err(),
        Some(Ok(PortfolioError::PortfolioNotFound))
    );
}

/// Registers a contract from its release WASM when it has been built
/// (`cargo build --workspace --target wasm32v1-none --release`), so the
/// measurement includes VM instantiation; otherwise uses the native build.
fn register_wasm_or<C: soroban_sdk::testutils::ContractFunctionSet + 'static>(
    env: &Env,
    crate_name: &str,
    native: C,
) -> (Address, bool) {
    let path = std::format!(
        "{}/../../target/wasm32v1-none/release/{crate_name}.wasm",
        env!("CARGO_MANIFEST_DIR")
    );
    match std::fs::read(path) {
        Ok(wasm) => (env.register(wasm.as_slice(), ()), true),
        Err(_) => (env.register(native, ()), false),
    }
}

fn cost(env: &Env, f: impl FnOnce()) -> (u64, u64, i64) {
    env.cost_estimate().budget().reset_default();
    f();
    let budget = env.cost_estimate().budget();
    (
        budget.cpu_instruction_cost(),
        budget.memory_bytes_cost(),
        env.cost_estimate().fee().total,
    )
}

/// Measures `mint_portfolio` in the monolith and in the split setup.
/// Run with `cargo test -p folder-portfolio mint_cost -- --nocapture`.
#[test]
fn mint_cost_split_vs_monolith() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let uri = String::from_str(&env, URI);
    let cid = String::from_str(&env, CID);
    let name = String::from_str(&env, "alice.folder");

    let (registry_id, wasm) = register_wasm_or(&env, "folder_registry", FolderRegistry);
    let (templates_id, _) = register_wasm_or(&env, "folder_templates", FolderTemplates);
    let (portfolio_id, _) = register_wasm_or(&env, "folder_portfolio", FolderPortfolio);
    let (mono_id, _) = register_wasm_or(&env, "indexer_contract", FolderContract);

    let templates = FolderTemplatesClient::new(&env, &templates_id);
    templates.initialize(&admin);
    templates.register_template(&admin, &1, &uri);
    let portfolio = FolderPortfolioClient::new(&env, &portfolio_id);
    portfolio.initialize(&admin, &registry_id, &templates_id);
    let split_user = Address::generate(&env);
    FolderRegistryClient::new(&env, &registry_id).claim_folder(&split_user, &name);

    let mono = FolderContractClient::new(&env, &mono_id);
    mono.initialize(&admin);
    mono.register_template(&admin, &1, &uri);
    let mono_user = Address::generate(&env);
    mono.claim_folder(&mono_user, &name);

    let split = cost(&env, || portfolio.mint_portfolio(&split_user, &1, &cid));
    let monolith = cost(&env, || mono.mint_portfolio(&mono_user, &1, &cid));

    std::println!("mode: {}", if wasm { "wasm" } else { "native" });
    std::println!("mint_portfolio monolith (cpu, mem, fee): {monolith:?}");
    std::println!("mint_portfolio split    (cpu, mem, fee): {split:?}");
    assert!(split.0 > 0 && monolith.0 > 0);
}
