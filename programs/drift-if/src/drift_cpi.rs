use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
};

use adapter_interface::AdapterError;

// Drift v2 discriminators (sha256("global:<name>")[..8]).
pub const INIT_USER_STATS: [u8; 8] = [254, 243, 72, 98, 251, 130, 168, 213];
pub const INIT_IF_STAKE: [u8; 8] = [187, 179, 243, 70, 248, 90, 92, 147];
pub const ADD_IF_STAKE: [u8; 8] = [251, 144, 115, 11, 222, 47, 62, 236];
pub const REQUEST_REMOVE_IF_STAKE: [u8; 8] = [142, 70, 204, 92, 73, 106, 180, 52];
pub const REMOVE_IF_STAKE: [u8; 8] = [128, 166, 142, 9, 254, 187, 143, 174];

/// USDC insurance fund.
pub const MARKET_INDEX: u16 = 0;

// InsuranceFundStake.if_shares (u128) @40 (after 8 disc + authority 32).
const IF_STAKE_SHARES: usize = 40;
// SpotMarket.insurance_fund.total_shares (u128) @336.
const SM_IF_TOTAL_SHARES: usize = 336;
// SPL TokenAccount.amount (u64) @64.
const TOKEN_AMOUNT: usize = 64;

fn read_u128(d: &[u8], off: usize) -> Result<u128> {
    let b: [u8; 16] = d.get(off..off + 16).ok_or(AdapterError::MathOverflow)?
        .try_into().map_err(|_| AdapterError::MathOverflow)?;
    Ok(u128::from_le_bytes(b))
}
fn read_u64(d: &[u8], off: usize) -> Result<u64> {
    let b: [u8; 8] = d.get(off..off + 8).ok_or(AdapterError::MathOverflow)?
        .try_into().map_err(|_| AdapterError::MathOverflow)?;
    Ok(u64::from_le_bytes(b))
}

pub fn stake_shares(if_stake_data: &[u8]) -> Result<u128> {
    read_u128(if_stake_data, IF_STAKE_SHARES)
}

/// USDC value of the position's IF stake = if_shares × vault_balance / total_shares.
pub fn stake_value_usdc(if_stake_data: &[u8], spot_market_data: &[u8], if_vault_data: &[u8]) -> Result<u64> {
    let shares = read_u128(if_stake_data, IF_STAKE_SHARES)?;
    if shares == 0 {
        return Ok(0);
    }
    let total = read_u128(spot_market_data, SM_IF_TOTAL_SHARES)?;
    if total == 0 {
        return Err(AdapterError::MathOverflow.into());
    }
    let vault = read_u64(if_vault_data, TOKEN_AMOUNT)? as u128;
    let value = shares.checked_mul(vault).ok_or(AdapterError::MathOverflow)?
        .checked_div(total).ok_or(AdapterError::MathOverflow)?;
    u64::try_from(value).map_err(|_| AdapterError::MathOverflow.into())
}

fn cpi<'info>(program: &Pubkey, metas: Vec<AccountMeta>, infos: &[AccountInfo<'info>], data: Vec<u8>) -> Result<()> {
    invoke(&Instruction { program_id: *program, accounts: metas, data }, infos).map_err(|e| {
        msg!("drift CPI failed: {:?}", e);
        error!(AdapterError::CpiFailed)
    })
}

/// CPI `initialize_user_stats`. [user_stats(w), state(w), authority, payer(signer,w), rent, system].
#[allow(clippy::too_many_arguments)]
pub fn init_user_stats<'info>(
    program: &AccountInfo<'info>, user_stats: &AccountInfo<'info>, state: &AccountInfo<'info>,
    authority: &AccountInfo<'info>, payer: &AccountInfo<'info>, rent: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
) -> Result<()> {
    let metas = vec![
        AccountMeta::new(user_stats.key(), false),
        AccountMeta::new(state.key(), false),
        AccountMeta::new_readonly(authority.key(), false),
        AccountMeta::new(payer.key(), true),
        AccountMeta::new_readonly(rent.key(), false),
        AccountMeta::new_readonly(system_program.key(), false),
    ];
    let infos = [program.clone(), user_stats.clone(), state.clone(), authority.clone(), payer.clone(), rent.clone(), system_program.clone()];
    cpi(&program.key(), metas, &infos, INIT_USER_STATS.to_vec())
}

/// CPI `initialize_insurance_fund_stake(market_index)`.
/// [spot_market, if_stake(w), user_stats(w), state, authority(signer), payer(signer,w), rent, system].
#[allow(clippy::too_many_arguments)]
pub fn init_if_stake<'info>(
    program: &AccountInfo<'info>, spot_market: &AccountInfo<'info>, if_stake: &AccountInfo<'info>,
    user_stats: &AccountInfo<'info>, state: &AccountInfo<'info>, authority: &AccountInfo<'info>,
    payer: &AccountInfo<'info>, rent: &AccountInfo<'info>, system_program: &AccountInfo<'info>,
) -> Result<()> {
    let metas = vec![
        AccountMeta::new_readonly(spot_market.key(), false),
        AccountMeta::new(if_stake.key(), false),
        AccountMeta::new(user_stats.key(), false),
        AccountMeta::new_readonly(state.key(), false),
        AccountMeta::new_readonly(authority.key(), true),
        AccountMeta::new(payer.key(), true),
        AccountMeta::new_readonly(rent.key(), false),
        AccountMeta::new_readonly(system_program.key(), false),
    ];
    let infos = [program.clone(), spot_market.clone(), if_stake.clone(), user_stats.clone(), state.clone(), authority.clone(), payer.clone(), rent.clone(), system_program.clone()];
    let mut data = INIT_IF_STAKE.to_vec();
    data.extend_from_slice(&MARKET_INDEX.to_le_bytes());
    cpi(&program.key(), metas, &infos, data)
}

/// CPI `add_insurance_fund_stake(market_index, amount)`.
#[allow(clippy::too_many_arguments)]
pub fn add_stake<'info>(
    program: &AccountInfo<'info>, state: &AccountInfo<'info>, spot_market: &AccountInfo<'info>,
    if_stake: &AccountInfo<'info>, user_stats: &AccountInfo<'info>, authority: &AccountInfo<'info>,
    spot_market_vault: &AccountInfo<'info>, if_vault: &AccountInfo<'info>, drift_signer: &AccountInfo<'info>,
    user_token_account: &AccountInfo<'info>, token_program: &AccountInfo<'info>, amount: u64,
) -> Result<()> {
    let metas = vec![
        AccountMeta::new_readonly(state.key(), false),
        AccountMeta::new(spot_market.key(), false),
        AccountMeta::new(if_stake.key(), false),
        AccountMeta::new(user_stats.key(), false),
        AccountMeta::new_readonly(authority.key(), true),
        AccountMeta::new(spot_market_vault.key(), false),
        AccountMeta::new(if_vault.key(), false),
        AccountMeta::new_readonly(drift_signer.key(), false),
        AccountMeta::new(user_token_account.key(), false),
        AccountMeta::new_readonly(token_program.key(), false),
    ];
    let infos = [program.clone(), state.clone(), spot_market.clone(), if_stake.clone(), user_stats.clone(), authority.clone(), spot_market_vault.clone(), if_vault.clone(), drift_signer.clone(), user_token_account.clone(), token_program.clone()];
    let mut data = ADD_IF_STAKE.to_vec();
    data.extend_from_slice(&MARKET_INDEX.to_le_bytes());
    data.extend_from_slice(&amount.to_le_bytes());
    cpi(&program.key(), metas, &infos, data)
}

/// CPI `request_remove_insurance_fund_stake(market_index, amount)`.
pub fn request_remove<'info>(
    program: &AccountInfo<'info>, spot_market: &AccountInfo<'info>, if_stake: &AccountInfo<'info>,
    user_stats: &AccountInfo<'info>, authority: &AccountInfo<'info>, if_vault: &AccountInfo<'info>, shares: u64,
) -> Result<()> {
    let metas = vec![
        AccountMeta::new(spot_market.key(), false),
        AccountMeta::new(if_stake.key(), false),
        AccountMeta::new(user_stats.key(), false),
        AccountMeta::new_readonly(authority.key(), true),
        AccountMeta::new(if_vault.key(), false),
    ];
    let infos = [program.clone(), spot_market.clone(), if_stake.clone(), user_stats.clone(), authority.clone(), if_vault.clone()];
    let mut data = REQUEST_REMOVE_IF_STAKE.to_vec();
    data.extend_from_slice(&MARKET_INDEX.to_le_bytes());
    data.extend_from_slice(&shares.to_le_bytes());
    cpi(&program.key(), metas, &infos, data)
}

/// CPI `remove_insurance_fund_stake(market_index)`.
#[allow(clippy::too_many_arguments)]
pub fn remove_stake<'info>(
    program: &AccountInfo<'info>, state: &AccountInfo<'info>, spot_market: &AccountInfo<'info>,
    if_stake: &AccountInfo<'info>, user_stats: &AccountInfo<'info>, authority: &AccountInfo<'info>,
    if_vault: &AccountInfo<'info>, drift_signer: &AccountInfo<'info>, user_token_account: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
) -> Result<()> {
    let metas = vec![
        AccountMeta::new_readonly(state.key(), false),
        AccountMeta::new(spot_market.key(), false),
        AccountMeta::new(if_stake.key(), false),
        AccountMeta::new(user_stats.key(), false),
        AccountMeta::new_readonly(authority.key(), true),
        AccountMeta::new(if_vault.key(), false),
        AccountMeta::new_readonly(drift_signer.key(), false),
        AccountMeta::new(user_token_account.key(), false),
        AccountMeta::new_readonly(token_program.key(), false),
    ];
    let infos = [program.clone(), state.clone(), spot_market.clone(), if_stake.clone(), user_stats.clone(), authority.clone(), if_vault.clone(), drift_signer.clone(), user_token_account.clone(), token_program.clone()];
    let mut data = REMOVE_IF_STAKE.to_vec();
    data.extend_from_slice(&MARKET_INDEX.to_le_bytes());
    cpi(&program.key(), metas, &infos, data)
}
