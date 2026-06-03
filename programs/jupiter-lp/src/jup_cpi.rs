use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
};

use adapter_interface::AdapterError;

// Jupiter Perpetuals discriminators (sha256("global:<name>")[..8]).
pub const JUP_ADD_LIQUIDITY2: [u8; 8] = [228, 162, 78, 28, 70, 219, 116, 115];
pub const JUP_REMOVE_LIQUIDITY2: [u8; 8] = [230, 215, 82, 127, 241, 101, 227, 146];

// Pool.aumUsd: u128 at byte 180 (after 8 disc + name "Pool"(4+4) + custodies vec(4 + 5*32)).
const POOL_AUM_USD: usize = 180;
// SPL Mint: supply (u64) at byte 36.
const MINT_SUPPLY: usize = 36;
// SPL TokenAccount: amount (u64) at byte 64.
const TOKEN_AMOUNT: usize = 64;

fn read_u128(data: &[u8], off: usize) -> Result<u128> {
    let b: [u8; 16] = data
        .get(off..off + 16)
        .ok_or(AdapterError::MathOverflow)?
        .try_into()
        .map_err(|_| AdapterError::MathOverflow)?;
    Ok(u128::from_le_bytes(b))
}
fn read_u64(data: &[u8], off: usize) -> Result<u64> {
    let b: [u8; 8] = data
        .get(off..off + 8)
        .ok_or(AdapterError::MathOverflow)?
        .try_into()
        .map_err(|_| AdapterError::MathOverflow)?;
    Ok(u64::from_le_bytes(b))
}

pub fn token_amount(token_account_data: &[u8]) -> Result<u64> {
    read_u64(token_account_data, TOKEN_AMOUNT)
}

/// USDC value of `lp_balance` JLP = lp_balance × pool.aumUsd / lp_supply.
/// aumUsd is 6-decimal USD, so the result is USDC base units; the JLP scale cancels.
pub fn lp_value_usdc(pool_data: &[u8], lp_mint_data: &[u8], lp_balance: u64) -> Result<u64> {
    if lp_balance == 0 {
        return Ok(0);
    }
    let aum = read_u128(pool_data, POOL_AUM_USD)?;
    let supply = read_u64(lp_mint_data, MINT_SUPPLY)? as u128;
    if supply == 0 {
        return Err(AdapterError::MathOverflow.into());
    }
    let value = (lp_balance as u128)
        .checked_mul(aum)
        .ok_or(AdapterError::MathOverflow)?
        .checked_div(supply)
        .ok_or(AdapterError::MathOverflow)?;
    u64::try_from(value).map_err(|_| AdapterError::MathOverflow.into())
}

/// The 14 accounts add_liquidity2/remove_liquidity2 share (slot 1 is funding vs
/// receiving, but the order is identical). `owner` (position) signs (propagated).
pub struct LiquidityAccounts<'info> {
    pub program: AccountInfo<'info>,
    pub owner: AccountInfo<'info>,
    pub token_account: AccountInfo<'info>, // funding (deposit) / receiving (withdraw): position USDC pool
    pub lp_token_account: AccountInfo<'info>,
    pub transfer_authority: AccountInfo<'info>,
    pub perpetuals: AccountInfo<'info>,
    pub pool: AccountInfo<'info>,
    pub custody: AccountInfo<'info>,
    pub doves_price: AccountInfo<'info>,
    pub pythnet_price: AccountInfo<'info>,
    pub custody_token_account: AccountInfo<'info>,
    pub lp_token_mint: AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
    pub event_authority: AccountInfo<'info>,
}

fn metas_and_infos<'info>(
    a: &LiquidityAccounts<'info>,
) -> (Vec<AccountMeta>, Vec<AccountInfo<'info>>) {
    let metas = vec![
        AccountMeta::new_readonly(a.owner.key(), true),
        AccountMeta::new(a.token_account.key(), false),
        AccountMeta::new(a.lp_token_account.key(), false),
        AccountMeta::new_readonly(a.transfer_authority.key(), false),
        AccountMeta::new_readonly(a.perpetuals.key(), false),
        AccountMeta::new(a.pool.key(), false),
        AccountMeta::new(a.custody.key(), false),
        AccountMeta::new_readonly(a.doves_price.key(), false),
        AccountMeta::new_readonly(a.pythnet_price.key(), false),
        AccountMeta::new(a.custody_token_account.key(), false),
        AccountMeta::new(a.lp_token_mint.key(), false),
        AccountMeta::new_readonly(a.token_program.key(), false),
        AccountMeta::new_readonly(a.event_authority.key(), false),
        AccountMeta::new_readonly(a.program.key(), false),
    ];
    let infos = vec![
        a.owner.clone(),
        a.token_account.clone(),
        a.lp_token_account.clone(),
        a.transfer_authority.clone(),
        a.perpetuals.clone(),
        a.pool.clone(),
        a.custody.clone(),
        a.doves_price.clone(),
        a.pythnet_price.clone(),
        a.custody_token_account.clone(),
        a.lp_token_mint.clone(),
        a.token_program.clone(),
        a.event_authority.clone(),
        a.program.clone(),
    ];
    (metas, infos)
}

/// Append the pool's AUM accounts (every custody + its oracles, forwarded
/// verbatim from the caller) — Jupiter reads them to value the whole pool. The
/// dispatcher/SDK supplies them in Jupiter's expected order; the adapter is
/// order-agnostic.
fn append_extra<'info>(
    metas: &mut Vec<AccountMeta>,
    infos: &mut Vec<AccountInfo<'info>>,
    extra: &[AccountInfo<'info>],
) {
    for acc in extra {
        metas.push(AccountMeta::new_readonly(acc.key(), false));
        infos.push(acc.clone());
    }
}

/// CPI `add_liquidity2(token_amount_in, min_lp_amount_out = 0, token_amount_pre_swap = None)`.
pub fn add_liquidity<'info>(
    a: &LiquidityAccounts<'info>,
    token_amount_in: u64,
    aum_accounts: &[AccountInfo<'info>],
) -> Result<()> {
    let (mut metas, mut infos) = metas_and_infos(a);
    append_extra(&mut metas, &mut infos, aum_accounts);
    let mut data = JUP_ADD_LIQUIDITY2.to_vec();
    data.extend_from_slice(&token_amount_in.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes()); // min_lp_amount_out
    data.push(0); // token_amount_pre_swap: Option<u64> = None
    invoke(&Instruction { program_id: a.program.key(), accounts: metas, data }, &infos)
        .map_err(|e| {
            msg!("jup add_liquidity2 failed: {:?}", e);
            error!(AdapterError::CpiFailed)
        })
}

/// CPI `remove_liquidity2(lp_amount_in, min_amount_out = 0)`.
pub fn remove_liquidity<'info>(
    a: &LiquidityAccounts<'info>,
    lp_amount_in: u64,
    aum_accounts: &[AccountInfo<'info>],
) -> Result<()> {
    let (mut metas, mut infos) = metas_and_infos(a);
    append_extra(&mut metas, &mut infos, aum_accounts);
    let mut data = JUP_REMOVE_LIQUIDITY2.to_vec();
    data.extend_from_slice(&lp_amount_in.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes()); // min_amount_out
    invoke(&Instruction { program_id: a.program.key(), accounts: metas, data }, &infos)
        .map_err(|e| {
            msg!("jup remove_liquidity2 failed: {:?}", e);
            error!(AdapterError::CpiFailed)
        })
}
