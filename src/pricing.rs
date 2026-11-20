use soroban_sdk::{token, Address, Env, String};

use crate::access_control::require_admin;
use crate::constants::{FOLDER_SUFFIX, MAX_PRICE_TIERS, MIN_FOLDER_LABEL_LEN};
use crate::errors::ContractError;
use crate::storage;
use crate::types::{FeeConfig, PriceTiers};
use crate::validation;

/// Validates a tier table: at most `MAX_PRICE_TIERS` entries, strictly
/// ascending `max_label_len` values of at least `MIN_FOLDER_LABEL_LEN`, and
/// non-negative amounts.
pub fn validate_tiers(tiers: &PriceTiers) -> Result<(), ContractError> {
    if tiers.by_len.len() > MAX_PRICE_TIERS {
        return Err(ContractError::InvalidPriceTiers);
    }
    let mut prev: Option<u32> = None;
    for (max_len, amount) in tiers.by_len.iter() {
        if max_len < MIN_FOLDER_LABEL_LEN || amount < 0 || prev.is_some_and(|p| max_len <= p) {
            return Err(ContractError::InvalidPriceTiers);
        }
        prev = Some(max_len);
    }
    Ok(())
}

/// Pure fee calculation: the first tier whose `max_label_len` covers
/// `label_len`, otherwise `base_fee`. Assumes `tiers` passed `validate_tiers`.
pub fn fee_for_label_len(tiers: &PriceTiers, base_fee: i128, label_len: u32) -> i128 {
    for (max_len, amount) in tiers.by_len.iter() {
        if label_len <= max_len {
            return amount;
        }
    }
    base_fee
}

/// Fee for one claim (or one lease period) of a valid Folder name. Zero when
/// no fee is configured.
pub fn quote_claim(env: &Env, folder_name: &String) -> Result<i128, ContractError> {
    validation::require_valid_folder_name(folder_name)?;
    let Some(config) = storage::get_fee_config(env) else {
        return Ok(0);
    };
    let label_len = folder_name.len() - FOLDER_SUFFIX.len() as u32;
    Ok(fee_for_label_len(
        &storage::get_price_tiers(env),
        config.base_fee,
        label_len,
    ))
}

/// Transfers `amount` of the configured fee token from `payer` to the treasury.
pub fn charge(env: &Env, payer: &Address, amount: i128) {
    if amount <= 0 {
        return;
    }
    if let Some(config) = storage::get_fee_config(env) {
        token::Client::new(env, &config.token).transfer(payer, &config.treasury, &amount);
    }
}

pub fn set_claim_fee(
    env: &Env,
    admin: &Address,
    token: &Address,
    treasury: &Address,
    base_fee: i128,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    if base_fee < 0 {
        return Err(ContractError::InvalidFee);
    }
    storage::set_fee_config(
        env,
        &FeeConfig {
            token: token.clone(),
            treasury: treasury.clone(),
            base_fee,
        },
    );
    Ok(())
}

pub fn set_price_tiers(
    env: &Env,
    admin: &Address,
    tiers: &PriceTiers,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    validate_tiers(tiers)?;
    storage::set_price_tiers(env, tiers);
    Ok(())
}
