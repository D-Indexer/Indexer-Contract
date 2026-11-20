use soroban_sdk::{Address, Env, String, Vec};

use crate::access_control::{require_admin, require_owner};
use crate::constants::MAX_TEMPLATES;
use crate::errors::ContractError;
use crate::events;
use crate::storage;
use crate::types::{PortfolioNFT, Template};
use crate::validation;

/// Validated, side-effect-free description of a template registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterTemplatePlan {
    pub template: Template,
}

/// Pure validation phase: checks auth and state, performs no writes.
pub fn plan_register_template(
    env: &Env,
    admin: &Address,
    template_id: u32,
    metadata_uri: &String,
) -> Result<RegisterTemplatePlan, ContractError> {
    require_admin(env, admin)?;
    validation::require_valid_template_id(template_id)?;
    validation::require_valid_metadata_uri(metadata_uri)?;
    if storage::get_template(env, template_id).is_some() {
        return Err(ContractError::TemplateAlreadyExists);
    }
    if storage::get_template_list(env).len() >= MAX_TEMPLATES {
        return Err(ContractError::TemplateLimitReached);
    }

    Ok(RegisterTemplatePlan {
        template: Template {
            template_id,
            metadata_uri: metadata_uri.clone(),
            creator: admin.clone(),
        },
    })
}

/// Write phase: applies a plan and emits its events.
pub fn apply_register_template(env: &Env, plan: &RegisterTemplatePlan) {
    storage::save_template(env, &plan.template);
    events::template_registered(
        env,
        plan.template.template_id,
        &plan.template.metadata_uri,
        &plan.template.creator,
    );
}

pub fn register_template(
    env: &Env,
    admin: &Address,
    template_id: u32,
    metadata_uri: &String,
) -> Result<(), ContractError> {
    let plan = plan_register_template(env, admin, template_id, metadata_uri)?;
    apply_register_template(env, &plan);
    Ok(())
}

pub fn get_template(env: &Env, template_id: u32) -> Result<Template, ContractError> {
    validation::require_valid_template_id(template_id)?;
    storage::get_template(env, template_id).ok_or(ContractError::TemplateNotFound)
}

pub fn list_templates(env: &Env) -> Vec<u32> {
    storage::get_template_list(env)
}

/// Validated, side-effect-free description of a user template assignment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetUserTemplatePlan {
    pub owner: Address,
    pub template_id: u32,
    /// Existing portfolio to keep in sync, if the owner has minted one.
    pub portfolio: Option<PortfolioNFT>,
}

/// Pure validation phase: checks auth and state, performs no writes.
pub fn plan_set_user_template(
    env: &Env,
    owner: &Address,
    template_id: u32,
) -> Result<SetUserTemplatePlan, ContractError> {
    require_owner(env, owner)?;
    storage::get_owner_folder(env, owner).ok_or(ContractError::FolderNotFound)?;
    validation::require_valid_template_id(template_id)?;
    storage::get_template(env, template_id).ok_or(ContractError::TemplateNotFound)?;
    // A minted portfolio always renders with the owner's current template.
    let portfolio = storage::get_portfolio(env, owner).ok().map(|mut nft| {
        nft.template_id = template_id;
        nft
    });
    Ok(SetUserTemplatePlan {
        owner: owner.clone(),
        template_id,
        portfolio,
    })
}

/// Write phase: applies a plan and emits its events.
pub fn apply_set_user_template(env: &Env, plan: &SetUserTemplatePlan) {
    storage::set_user_template(env, &plan.owner, plan.template_id);
    if let Some(nft) = &plan.portfolio {
        storage::save_portfolio(env, nft);
    }
    events::template_assigned(env, &plan.owner, plan.template_id);
}

pub fn set_user_template(
    env: &Env,
    owner: &Address,
    template_id: u32,
) -> Result<(), ContractError> {
    let plan = plan_set_user_template(env, owner, template_id)?;
    apply_set_user_template(env, &plan);
    Ok(())
}
