use soroban_sdk::{token, Address, Env};

use crate::access_control::require_admin;
use crate::constants::{MAX_CLAIM_FEE, MAX_WITHDRAW_PER_CALL};
use crate::errors::ContractError;
use crate::events;
use crate::storage;
use crate::types::ClaimFee;

pub fn set_claim_fee(
    env: &Env,
    admin: &Address,
    token: &Address,
    amount: i128,
    treasury: &Address,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    if !(0..=MAX_CLAIM_FEE).contains(&amount) {
        return Err(ContractError::InvalidAmount);
    }
    let fee = ClaimFee {
        token: token.clone(),
        amount,
        treasury: treasury.clone(),
    };
    storage::set_claim_fee(env, &fee);
    events::claim_fee_set(env, admin, &fee);
    Ok(())
}

pub fn treasury_balance(env: &Env, token: &Address) -> i128 {
    token::Client::new(env, token).balance(&env.current_contract_address())
}

pub fn withdraw(
    env: &Env,
    admin: &Address,
    token: &Address,
    to: &Address,
    amount: i128,
) -> Result<(), ContractError> {
    require_admin(env, admin)?;
    if amount <= 0 || amount > MAX_WITHDRAW_PER_CALL {
        return Err(ContractError::InvalidAmount);
    }
    if amount > treasury_balance(env, token) {
        return Err(ContractError::InsufficientBalance);
    }
    token::Client::new(env, token).transfer(&env.current_contract_address(), to, &amount);
    events::withdrawn(env, admin, token, to, amount);
    Ok(())
}
