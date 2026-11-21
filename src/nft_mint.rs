use soroban_sdk::{Address, Env, String};

use crate::access_control::require_owner;
use crate::errors::ContractError;
use crate::events;
use crate::identity_map;
use crate::storage;
use crate::types::{PortfolioDelegate, PortfolioMeta, PortfolioNFT};
use crate::validation;

pub fn mint_portfolio(
    env: &Env,
    owner: &Address,
    template_id: u32,
    ipfs_cid: &String,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;

    // Must have claimed a Folder name before minting
    identity_map::get_identity(env, owner)?;

    // Prevent double-mint (single read, also refreshes the entry's TTL)
    if storage::load_portfolio(env, owner).is_some() {
        return Err(ContractError::PortfolioAlreadyMinted);
    }

    validation::require_valid_template_id(template_id)?;
    validation::require_valid_ipfs_cid(ipfs_cid)?;
    storage::get_template(env, template_id).ok_or(ContractError::TemplateNotFound)?;

    let nft = PortfolioNFT {
        owner: owner.clone(),
        template_id,
        ipfs_cid: ipfs_cid.clone(),
        minted_at: env.ledger().timestamp(),
    };
    storage::save_portfolio(env, &nft);
    storage::save_portfolio_meta(
        env,
        owner,
        &PortfolioMeta {
            revision: 0,
            updated_at: nft.minted_at,
        },
    );
    // Keep the user's template selection consistent with the minted portfolio.
    storage::set_user_template(env, owner, template_id);
    events::portfolio_minted(env, owner, template_id, ipfs_cid);
    Ok(())
}

pub fn update_portfolio(
    env: &Env,
    owner: &Address,
    ipfs_cid: &String,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;

    validation::require_valid_ipfs_cid(ipfs_cid)?;
    let nft = storage::get_portfolio(env, owner)?;
    write_cid(env, nft, ipfs_cid);
    Ok(())
}

/// Compare-and-swap variant of `update_portfolio`: writes only if the
/// current CID equals `expected_cid`, otherwise fails with `StaleUpdate`.
pub fn update_portfolio_if(
    env: &Env,
    owner: &Address,
    ipfs_cid: &String,
    expected_cid: &String,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;

    validation::require_valid_ipfs_cid(ipfs_cid)?;
    validation::require_valid_ipfs_cid(expected_cid)?;
    let nft = storage::get_portfolio(env, owner)?;
    if nft.ipfs_cid != *expected_cid {
        return Err(ContractError::StaleUpdate);
    }
    write_cid(env, nft, ipfs_cid);
    Ok(())
}

/// Authorizes `delegate` to update the owner's portfolio CID until
/// `expires_ledger` (inclusive; 0 = no expiry). Replaces any previous
/// delegate. The delegate can only call `delegate_update_portfolio`.
pub fn set_portfolio_delegate(
    env: &Env,
    owner: &Address,
    delegate: &Address,
    expires_ledger: u32,
) -> Result<(), ContractError> {
    require_owner(env, owner)?;
    if delegate == owner || (expires_ledger != 0 && expires_ledger < env.ledger().sequence()) {
        return Err(ContractError::InvalidDelegate);
    }
    // Delegation is portfolio-scoped: burn_portfolio removes it.
    storage::get_portfolio(env, owner)?;
    storage::set_portfolio_delegate(
        env,
        owner,
        &PortfolioDelegate {
            delegate: delegate.clone(),
            expires_ledger,
        },
    );
    events::delegate_set(env, owner, delegate, expires_ledger);
    Ok(())
}

/// Revokes the owner's delegate. A no-op (no event) when none is set.
pub fn clear_portfolio_delegate(env: &Env, owner: &Address) -> Result<(), ContractError> {
    require_owner(env, owner)?;
    if let Some(current) = storage::get_portfolio_delegate(env, owner) {
        storage::remove_portfolio_delegate(env, owner);
        events::delegate_cleared(env, owner, &current.delegate);
    }
    Ok(())
}

/// `update_portfolio` signed by the owner's delegate instead of the owner.
pub fn delegate_update_portfolio(
    env: &Env,
    delegate: &Address,
    owner: &Address,
    ipfs_cid: &String,
) -> Result<(), ContractError> {
    delegate.require_auth();

    validation::require_valid_ipfs_cid(ipfs_cid)?;
    let record = storage::get_portfolio_delegate(env, owner)
        .filter(|d| d.delegate == *delegate)
        .ok_or(ContractError::Unauthorized)?;
    if record.expires_ledger != 0 && env.ledger().sequence() > record.expires_ledger {
        return Err(ContractError::DelegateExpired);
    }
    let nft = storage::get_portfolio(env, owner)?;
    write_cid(env, nft, ipfs_cid);
    Ok(())
}

pub fn get_portfolio_delegate(env: &Env, owner: &Address) -> Option<PortfolioDelegate> {
    storage::get_portfolio_delegate(env, owner)
}

/// Stores the new CID, bumps the revision and emits `port_upd`. Callers have
/// already authorized and validated.
fn write_cid(env: &Env, mut nft: PortfolioNFT, ipfs_cid: &String) {
    nft.ipfs_cid = ipfs_cid.clone();
    storage::save_portfolio(env, &nft);
    let meta = bump_revision(env, &nft);
    events::portfolio_updated(env, &nft.owner, ipfs_cid, meta.revision);
}

/// The portfolio's change-tracking entry. A portfolio minted before
/// `PortfolioMeta` existed has no entry; it reads as revision 0, last
/// updated at `minted_at`, and gets a real entry on its next change.
pub fn portfolio_meta(env: &Env, nft: &PortfolioNFT) -> PortfolioMeta {
    storage::get_portfolio_meta(env, &nft.owner).unwrap_or(PortfolioMeta {
        revision: 0,
        updated_at: nft.minted_at,
    })
}

/// Records one state change of `nft`: revision + 1, `updated_at` = now.
pub fn bump_revision(env: &Env, nft: &PortfolioNFT) -> PortfolioMeta {
    let mut meta = portfolio_meta(env, nft);
    meta.revision += 1;
    meta.updated_at = env.ledger().timestamp();
    storage::save_portfolio_meta(env, &nft.owner, &meta);
    meta
}

pub fn get_portfolio_meta(env: &Env, owner: &Address) -> Result<PortfolioMeta, ContractError> {
    let nft = storage::get_portfolio(env, owner)?;
    Ok(portfolio_meta(env, &nft))
}

/// Deletes the owner's portfolio so they can mint again. The Folder name and
/// the selected template (`UserTemplate`) are kept.
pub fn burn_portfolio(env: &Env, owner: &Address) -> Result<(), ContractError> {
    require_owner(env, owner)?;
    let nft = storage::get_portfolio(env, owner)?;
    storage::remove_portfolio(env, owner);
    events::portfolio_burned(env, owner, &nft.ipfs_cid);
    Ok(())
}

pub fn get_portfolio(env: &Env, owner: &Address) -> Result<PortfolioNFT, ContractError> {
    storage::get_portfolio(env, owner)
}

pub fn verify_portfolio(env: &Env, owner: &Address) -> Result<bool, ContractError> {
    match storage::get_portfolio(env, owner) {
        Ok(_) => Ok(true),
        Err(ContractError::PortfolioNotFound) => Ok(false),
        Err(err) => Err(err),
    }
}
