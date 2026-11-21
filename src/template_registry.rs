use soroban_sdk::{Address, Env, String, Vec};

use crate::access_control::{require_admin, require_owner};
use crate::constants::{MAX_TEMPLATES, MAX_TEMPLATE_PAGE_SIZE};
use crate::errors::ContractError;
use crate::events;
use crate::identity_map;
use crate::storage;
use crate::types::Template;
use crate::validation;

pub fn register_template(
    env: &Env,
    admin: &Address,
    template_id: u32,
    metadata_uri: &String,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    validation::require_valid_template_id(template_id)?;
    validation::require_valid_metadata_uri(metadata_uri)?;
    if storage::get_template(env, template_id).is_some() {
        return Err(ContractError::TemplateAlreadyExists);
    }
    let list = storage::get_template_list(env);
    if list.len() >= MAX_TEMPLATES {
        return Err(ContractError::TemplateLimitReached);
    }

    let template = Template {
        template_id,
        metadata_uri: metadata_uri.clone(),
        creator: admin.clone(),
    };
    storage::save_template_with_list(env, &template, list);
    events::template_registered(env, template_id, metadata_uri, admin);
    Ok(())
}

pub fn get_template(env: &Env, template_id: u32) -> Result<Template, ContractError> {
    validation::require_valid_template_id(template_id)?;
    storage::get_template(env, template_id).ok_or(ContractError::TemplateNotFound)
}

pub fn list_templates(env: &Env) -> Vec<u32> {
    storage::get_template_list(env)
}

pub fn list_templates_page(env: &Env, start: u32, limit: u32) -> Vec<Template> {
    let ids = storage::get_template_list(env);
    let mut templates = Vec::new(env);
    let end = start
        .saturating_add(limit.min(MAX_TEMPLATE_PAGE_SIZE))
        .min(ids.len());
    let mut index = start;
    while index < end {
        if let Some(template_id) = ids.get(index) {
            if let Some(template) = storage::get_template(env, template_id) {
                templates.push_back(template);
            }
        }
        index += 1;
    }
    templates
}

pub fn set_user_template(
    env: &Env,
    owner: &Address,
    template_id: u32,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;
    identity_map::get_identity(env, owner)?;
    validation::require_valid_template_id(template_id)?;
    storage::get_template(env, template_id).ok_or(ContractError::TemplateNotFound)?;
    storage::set_user_template(env, owner, template_id);
    // A minted portfolio always renders with the owner's current template.
    if let Some(mut nft) = storage::load_portfolio(env, owner) {
        nft.template_id = template_id;
        storage::save_portfolio(env, &nft);
    }
    events::template_assigned(env, owner, template_id);
    Ok(())
}
