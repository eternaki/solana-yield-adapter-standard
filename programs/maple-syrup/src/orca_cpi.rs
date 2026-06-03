use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
};

use adapter_interface::AdapterError;

/// Orca Whirlpools `swap` discriminator (sha256("global:swap")[..8]).
pub const ORCA_SWAP: [u8; 8] = [248, 198, 158, 145, 225, 117, 135, 200];

/// Whirlpool tick price bounds (the swap won't cross these).
const MIN_SQRT_PRICE: u128 = 4_295_048_016;
const MAX_SQRT_PRICE: u128 = 79_226_673_515_401_279_992_447_579_055;

// Whirlpool.sqrt_price: u128 at byte 65. SPL TokenAccount.amount: u64 at byte 64.
const WP_SQRT_PRICE: usize = 65;
const TOKEN_AMOUNT: usize = 64;

fn read_u128(d: &[u8], off: usize) -> Result<u128> {
    let b: [u8; 16] = d.get(off..off + 16).ok_or(AdapterError::MathOverflow)?
        .try_into().map_err(|_| AdapterError::MathOverflow)?;
    Ok(u128::from_le_bytes(b))
}

pub fn token_amount(d: &[u8]) -> Result<u64> {
    let b: [u8; 8] = d.get(TOKEN_AMOUNT..TOKEN_AMOUNT + 8).ok_or(AdapterError::MathOverflow)?
        .try_into().map_err(|_| AdapterError::MathOverflow)?;
    Ok(u64::from_le_bytes(b))
}

/// USDC value of `syrup_balance` = balance × (sqrt_price / 2^64)² (both mints are
/// 6-decimal, so the price ratio is dimensionless → USDC base units). Staged shifts
/// keep the u128 product from overflowing.
pub fn whirlpool_value_usdc(wp_data: &[u8], syrup_balance: u64) -> Result<u64> {
    if syrup_balance == 0 {
        return Ok(0);
    }
    let sp = read_u128(wp_data, WP_SQRT_PRICE)?;
    let step1 = (syrup_balance as u128).checked_mul(sp).ok_or(AdapterError::MathOverflow)? >> 64;
    let value = step1.checked_mul(sp).ok_or(AdapterError::MathOverflow)? >> 64;
    u64::try_from(value).map_err(|_| AdapterError::MathOverflow.into())
}

/// The 11 accounts Orca's `swap` takes. `authority` (position) signs (propagated).
/// token_owner_a = syrupUSDC account (mint A); token_owner_b = USDC pool (mint B).
pub struct SwapAccounts<'info> {
    pub program: AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
    pub whirlpool: AccountInfo<'info>,
    pub token_owner_a: AccountInfo<'info>,
    pub vault_a: AccountInfo<'info>,
    pub token_owner_b: AccountInfo<'info>,
    pub vault_b: AccountInfo<'info>,
    pub tick_array_0: AccountInfo<'info>,
    pub tick_array_1: AccountInfo<'info>,
    pub tick_array_2: AccountInfo<'info>,
    pub oracle: AccountInfo<'info>,
}

/// CPI Orca `swap` with `amount` as the exact input. `a_to_b = false` swaps USDC→
/// syrupUSDC (deposit); `true` swaps syrupUSDC→USDC (withdraw).
pub fn swap(a: &SwapAccounts, amount: u64, a_to_b: bool) -> Result<()> {
    let sqrt_price_limit = if a_to_b { MIN_SQRT_PRICE } else { MAX_SQRT_PRICE };
    let metas = vec![
        AccountMeta::new_readonly(a.token_program.key(), false),
        AccountMeta::new_readonly(a.authority.key(), true),
        AccountMeta::new(a.whirlpool.key(), false),
        AccountMeta::new(a.token_owner_a.key(), false),
        AccountMeta::new(a.vault_a.key(), false),
        AccountMeta::new(a.token_owner_b.key(), false),
        AccountMeta::new(a.vault_b.key(), false),
        AccountMeta::new(a.tick_array_0.key(), false),
        AccountMeta::new(a.tick_array_1.key(), false),
        AccountMeta::new(a.tick_array_2.key(), false),
        AccountMeta::new(a.oracle.key(), false),
    ];
    let mut data = ORCA_SWAP.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes()); // other_amount_threshold (min out = 0)
    data.extend_from_slice(&sqrt_price_limit.to_le_bytes());
    data.push(1); // amount_specified_is_input = true
    data.push(if a_to_b { 1 } else { 0 });
    let infos = [
        a.program.clone(), a.token_program.clone(), a.authority.clone(), a.whirlpool.clone(),
        a.token_owner_a.clone(), a.vault_a.clone(), a.token_owner_b.clone(), a.vault_b.clone(),
        a.tick_array_0.clone(), a.tick_array_1.clone(), a.tick_array_2.clone(), a.oracle.clone(),
    ];
    invoke(&Instruction { program_id: a.program.key(), accounts: metas, data }, &infos)
        .map_err(|e| {
            msg!("orca swap failed: {:?}", e);
            error!(AdapterError::CpiFailed)
        })
}
