extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::{FolderTemplates, FolderTemplatesClient, TemplatesError};

const URI: &str =
    "ipfs://bafkreic43yhrfghud56rzc4qpi3jskt2kezclitblplogb57dkiutmdlia/template.json";

fn setup() -> (Env, Address, FolderTemplatesClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(FolderTemplates, ());
    let client = FolderTemplatesClient::new(&env, &id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, admin, client)
}

#[test]
fn register_get_and_list() {
    let (env, admin, c) = setup();
    c.register_template(&admin, &1, &String::from_str(&env, URI));
    let t = c.get_template(&1);
    assert_eq!(t.creator, admin);
    assert_eq!(c.list_templates().len(), 1);
}

#[test]
fn register_rules() {
    let (env, admin, c) = setup();
    let uri = String::from_str(&env, URI);
    assert_eq!(
        c.try_initialize(&admin).err(),
        Some(Ok(TemplatesError::AlreadyInitialized))
    );
    assert_eq!(
        c.try_register_template(&Address::generate(&env), &1, &uri)
            .err(),
        Some(Ok(TemplatesError::Unauthorized))
    );
    assert_eq!(
        c.try_register_template(&admin, &0, &uri).err(),
        Some(Ok(TemplatesError::InvalidTemplateID))
    );
    assert_eq!(
        c.try_register_template(&admin, &1, &String::from_str(&env, "https://x"))
            .err(),
        Some(Ok(TemplatesError::InvalidMetadataURI))
    );
    c.register_template(&admin, &1, &uri);
    assert_eq!(
        c.try_register_template(&admin, &1, &uri).err(),
        Some(Ok(TemplatesError::TemplateAlreadyExists))
    );
    assert_eq!(
        c.try_get_template(&2).err(),
        Some(Ok(TemplatesError::TemplateNotFound))
    );
}
