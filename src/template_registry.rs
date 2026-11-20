use soroban_sdk::{Address, Env, String, Vec};

use crate::access_control::{require_admin, require_owner};
use crate::constants::{MAX_TEMPLATES, MAX_TEMPLATE_SCHEMAS};
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
    validation::require_valid_template_id(template_id)?;
    validation::require_valid_metadata_uri(metadata_uri)?;
    if storage::get_template(env, template_id).is_some() {
        return Err(ContractError::TemplateAlreadyExists);
    }
    if storage::get_template_list(env).len() >= MAX_TEMPLATES {
        return Err(ContractError::TemplateLimitReached);
    }

    let template = Template {
        template_id,
        metadata_uri: metadata_uri.clone(),
        creator: admin.clone(),
    };
    storage::save_template(env, &template);
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

pub fn set_user_template(
    env: &Env,
    owner: &Address,
    template_id: u32,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;
    storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)?;
    validation::require_valid_template_id(template_id)?;
    storage::get_template(env, template_id).ok_or(ContractError::TemplateNotFound)?;
    require_template_allowed(env, template_id, owner)?;
    if storage::portfolio_exists(env, owner) {
        require_schema_supported(env, template_id, storage::get_portfolio_schema(env, owner))?;
    }
    storage::set_user_template(env, owner, template_id);
    // A minted portfolio always renders with the owner's current template.
    if let Ok(mut nft) = storage::get_portfolio(env, owner) {
        nft.template_id = template_id;
        storage::save_portfolio(env, &nft);
    }
    events::template_assigned(env, owner, template_id);
    Ok(())
}

// ── Content schemas ───────────────────────────────────────────────────────────

pub fn set_template_schemas(
    env: &Env,
    admin: &Address,
    template_id: u32,
    schemas: &Vec<u32>,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    get_template(env, template_id)?;
    if schemas.is_empty() || schemas.len() > MAX_TEMPLATE_SCHEMAS || schemas.contains(0) {
        return Err(ContractError::InvalidSchemaList);
    }
    storage::set_template_schemas(env, template_id, schemas);
    Ok(())
}

pub fn require_schema_supported(
    env: &Env,
    template_id: u32,
    content_schema: u32,
) -> Result<(), ContractError> {
    if storage::get_template_schemas(env, template_id).contains(content_schema) {
        Ok(())
    } else {
        Err(ContractError::IncompatibleSchema)
    }
}

// ── Gating ────────────────────────────────────────────────────────────────────

pub fn set_template_gated(
    env: &Env,
    admin: &Address,
    template_id: u32,
    gated: bool,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    get_template(env, template_id)?;
    storage::set_template_gated(env, template_id, gated);
    Ok(())
}

/// Adds or removes `owner` from a template's allowlist. Removal does not
/// change a selection or portfolio the owner already has.
pub fn set_template_allowed(
    env: &Env,
    admin: &Address,
    template_id: u32,
    owner: &Address,
    allowed: bool,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    get_template(env, template_id)?;
    storage::set_template_allowed(env, template_id, owner, allowed);
    Ok(())
}

pub fn can_use_template(env: &Env, template_id: u32, owner: &Address) -> bool {
    storage::get_template(env, template_id).is_some()
        && require_template_allowed(env, template_id, owner).is_ok()
}

pub fn require_template_allowed(
    env: &Env,
    template_id: u32,
    owner: &Address,
) -> Result<(), ContractError> {
    if storage::is_template_gated(env, template_id)
        && !storage::is_template_allowed(env, template_id, owner)
    {
        return Err(ContractError::TemplateNotAllowed);
    }
    Ok(())
}
