use soroban_sdk::{token, Address, Env};

use crate::access_control::require_admin;
use crate::constants::MAX_BPS;
use crate::errors::ContractError;
use crate::events;
use crate::storage;
use crate::types::TemplatePrice;
use crate::validation;

/// Splits `amount` into `(designer_share, treasury_share)`. The designer gets
/// `amount * bps / 10_000` rounded down; the rounding remainder goes to the
/// treasury, so the two shares always sum to `amount`.
pub fn split(amount: i128, designer_bps: u32) -> Option<(i128, i128)> {
    if designer_bps > MAX_BPS || amount < 0 {
        return None;
    }
    let designer = amount
        .checked_mul(designer_bps as i128)?
        .checked_div(MAX_BPS as i128)?;
    let treasury = amount.checked_sub(designer)?;
    Some((designer, treasury))
}

pub fn set_treasury(env: &Env, admin: &Address, treasury: &Address) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    storage::set_treasury(env, treasury);
    events::treasury_set(env, admin, treasury);
    Ok(())
}

pub fn set_template_price(
    env: &Env,
    admin: &Address,
    template_id: u32,
    price: &TemplatePrice,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    validation::require_valid_template_id(template_id)?;
    storage::get_template(env, template_id).ok_or(ContractError::TemplateNotFound)?;
    if price.designer_bps > MAX_BPS {
        return Err(ContractError::InvalidRoyaltyShare);
    }
    if price.amount <= 0 {
        return Err(ContractError::InvalidPrice);
    }
    storage::set_template_price(env, template_id, price);
    events::template_price_set(env, admin, template_id, Some(price.clone()));
    Ok(())
}

pub fn remove_template_price(
    env: &Env,
    admin: &Address,
    template_id: u32,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    validation::require_valid_template_id(template_id)?;
    storage::remove_template_price(env, template_id);
    events::template_price_set(env, admin, template_id, None);
    Ok(())
}

/// Charges `owner` for switching to `template_id`. Free templates and
/// re-selecting the owner's current template cost nothing.
pub fn charge_on_switch(env: &Env, owner: &Address, template_id: u32) -> Result<(), ContractError> {
    if storage::get_user_template(env, owner) == Some(template_id) {
        return Ok(());
    }
    let price = match storage::get_template_price(env, template_id) {
        Some(price) => price,
        None => return Ok(()),
    };
    let treasury = storage::get_treasury(env).ok_or(ContractError::TreasuryNotSet)?;
    let (designer_share, treasury_share) =
        split(price.amount, price.designer_bps).ok_or(ContractError::InvalidRoyaltyShare)?;

    let client = token::TokenClient::new(env, &price.token);
    if designer_share > 0 {
        client.transfer(owner, &price.designer, &designer_share);
    }
    if treasury_share > 0 {
        client.transfer(owner, &treasury, &treasury_share);
    }
    Ok(())
}
