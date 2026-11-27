use soroban_sdk::{Address, Env, String, Vec};

use crate::access_control::{require_admin, require_owner};
use crate::constants::{MAX_PENDING_TEMPLATE_SUBMISSIONS, MAX_TEMPLATES};
use crate::errors::ContractError;
use crate::events;
use crate::identity_map;
use crate::storage;
use crate::types::{Template, TemplateSubmission};
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

pub fn set_user_template(
    env: &Env,
    owner: &Address,
    template_id: u32,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;
    if storage::is_blocked(env, owner) {
        return Err(ContractError::AddressBlocked);
    }
    identity_map::get_identity(env, owner)?;
    validation::require_valid_template_id(template_id)?;
    storage::get_template(env, template_id).ok_or(ContractError::TemplateNotFound)?;
    let old_template = storage::get_user_template(env, owner);
    if old_template == Some(template_id) {
        return Ok(());
    }
    storage::set_user_template(env, owner, template_id);
    if let Some(old_id) = old_template {
        let old_usage = storage::get_template_usage(env, old_id);
        storage::set_template_usage(env, old_id, old_usage.saturating_sub(1));
    }
    let new_usage = storage::get_template_usage(env, template_id);
    storage::set_template_usage(env, template_id, new_usage.saturating_add(1));
    // A minted portfolio always renders with the owner's current template.
    if let Some(mut nft) = storage::load_portfolio(env, owner) {
        nft.template_id = template_id;
        storage::save_portfolio(env, &nft);
    }
    events::template_assigned(env, owner, template_id);
    Ok(())
}

pub fn submit_template(
    env: &Env,
    designer: &Address,
    metadata_uri: &String,
) -> Result<u32, ContractError> {
    designer.require_auth();
    validation::require_valid_metadata_uri(metadata_uri)?;
    let mut pending = storage::get_pending_template_submissions(env);
    if pending.len() >= MAX_PENDING_TEMPLATE_SUBMISSIONS {
        return Err(ContractError::SubmissionQueueFull);
    }
    let submission_id = storage::next_template_submission_id(env);
    storage::save_template_submission(
        env,
        submission_id,
        &TemplateSubmission {
            designer: designer.clone(),
            metadata_uri: metadata_uri.clone(),
        },
    );
    pending.push_back(submission_id);
    storage::save_pending_template_submissions(env, &pending);
    events::template_submitted(env, submission_id, designer, metadata_uri);
    Ok(submission_id)
}

pub fn approve_submission(
    env: &Env,
    admin: &Address,
    submission_id: u32,
    template_id: u32,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    let pending = storage::get_pending_template_submissions(env);
    if !pending.contains(submission_id) {
        return Err(ContractError::SubmissionNotFound);
    }
    let submission =
        storage::get_template_submission(env, submission_id).ok_or(ContractError::SubmissionNotFound)?;

    // Registration performs all template-ID and URI validation.
    validation::require_valid_template_id(template_id)?;
    validation::require_valid_metadata_uri(&submission.metadata_uri)?;
    if storage::get_template(env, template_id).is_some() {
        return Err(ContractError::TemplateAlreadyExists);
    }
    let list = storage::get_template_list(env);
    if list.len() >= MAX_TEMPLATES {
        return Err(ContractError::TemplateLimitReached);
    }
    let template = Template {
        template_id,
        metadata_uri: submission.metadata_uri.clone(),
        creator: admin.clone(),
    };
    storage::save_template_with_list(env, &template, list);
    events::template_registered(
        env,
        template_id,
        &submission.metadata_uri,
        admin,
    );
    storage::set_template_designer(env, template_id, &submission.designer);
    remove_pending_submission(env, submission_id);
    storage::remove_template_submission(env, submission_id);
    events::template_submission_approved(env, admin, submission_id, template_id);
    Ok(())
}

pub fn reject_submission(
    env: &Env,
    admin: &Address,
    submission_id: u32,
    reason_code: u32,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    let pending = storage::get_pending_template_submissions(env);
    if !pending.contains(submission_id) {
        return Err(ContractError::SubmissionNotFound);
    }
    remove_pending_submission(env, submission_id);
    storage::remove_template_submission(env, submission_id);
    events::template_submission_rejected(env, admin, submission_id, reason_code);
    Ok(())
}

fn remove_pending_submission(env: &Env, submission_id: u32) {
    let pending = storage::get_pending_template_submissions(env);
    let mut remaining = Vec::new(env);
    for id in pending.iter() {
        if id != submission_id {
            remaining.push_back(id);
        }
    }
    storage::save_pending_template_submissions(env, &remaining);
}
