use soroban_sdk::{Address, Env, String, Vec};

use crate::access_control::{require_admin, require_owner};
use crate::errors::ContractError;
use crate::events;
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
    validation::require_valid_metadata_uri(metadata_uri)?;
    if storage::get_template(env, template_id).is_some() {
        return Err(ContractError::TemplateAlreadyExists);
    }

    let template = Template {
        template_id,
        metadata_uri: metadata_uri.clone(),
        creator: admin.clone(),
    };
    storage::save_template(env, &template);
    events::template_registered(env, template_id, metadata_uri);
    Ok(())
}

pub fn get_template(env: &Env, template_id: u32) -> Result<Template, ContractError> {
    storage::get_template(env, template_id).ok_or(ContractError::TemplateNotFound)
}

pub fn list_templates(env: &Env) -> Vec<u32> {
    storage::get_template_list(env)
}

pub fn set_user_template(
    env: &Env,
    owner: &Address,
    template_id: u32,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;
    storage::get_template(env, template_id).ok_or(ContractError::TemplateNotFound)?;
    storage::set_user_template(env, owner, template_id);
    events::template_assigned(env, owner, template_id);
    Ok(())
}
