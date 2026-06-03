use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
};

use adapter_interface::AdapterError;

// ============================================================================
// Kamino Lend (klend) instruction discriminators — sha256("global:<name>")[..8].
// Verified against the on-chain klend IDL.
// ============================================================================
pub const KLEND_REFRESH_RESERVE: [u8; 8] = [2, 218, 138, 235, 79, 201, 25, 102];
pub const KLEND_DEPOSIT_RESERVE_LIQUIDITY: [u8; 8] = [169, 201, 30, 126, 6, 205, 102, 68];
pub const KLEND_REDEEM_RESERVE_COLLATERAL: [u8; 8] = [234, 117, 181, 125, 185, 142, 220, 29];

// ============================================================================
// klend Reserve account field offsets (bytes from account start, incl. the
// 8-byte Anchor discriminator), from the klend IDL struct layout; cross-checked
// in the mainnet-fork test.
// ============================================================================
const OFF_LAST_UPDATE_SLOT: usize = 16; // last_update.slot : u64
const OFF_TOTAL_AVAILABLE: usize = 224; // liquidity.total_available_amount : u64
const OFF_BORROWED_SF: usize = 232; // liquidity.borrowed_amount_sf : u128 (scaled 2^60)
const OFF_COLLATERAL_SUPPLY: usize = 2592; // collateral.mint_total_supply : u64
/// klend `Fraction` fixed-point scale: integer part is `sf >> 60`.
const SF_SHIFT: u32 = 60;

/// Slot at which the reserve's cached state was last refreshed on-chain.
pub fn reserve_last_update_slot(reserve_data: &[u8]) -> Result<u64> {
    read_u64(reserve_data, OFF_LAST_UPDATE_SLOT)
}

fn read_u64(data: &[u8], off: usize) -> Result<u64> {
    let bytes: [u8; 8] = data
        .get(off..off + 8)
        .ok_or(AdapterError::MathOverflow)?
        .try_into()
        .map_err(|_| AdapterError::MathOverflow)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_u128(data: &[u8], off: usize) -> Result<u128> {
    let bytes: [u8; 16] = data
        .get(off..off + 16)
        .ok_or(AdapterError::MathOverflow)?
        .try_into()
        .map_err(|_| AdapterError::MathOverflow)?;
    Ok(u128::from_le_bytes(bytes))
}

/// USDC value of `shares` cTokens from a refreshed klend reserve.
///
/// value = shares × total_liquidity / collateral_supply, where
/// total_liquidity = total_available_amount + (borrowed_amount_sf >> 60).
/// Reserve MUST be refreshed first so accrued interest is reflected in `borrowed`.
pub fn collateral_value_usdc(reserve_data: &[u8], shares: u64) -> Result<u64> {
    let available = read_u64(reserve_data, OFF_TOTAL_AVAILABLE)? as u128;
    let borrowed_sf = read_u128(reserve_data, OFF_BORROWED_SF)?;
    let borrowed = borrowed_sf >> SF_SHIFT;
    let collateral_supply = read_u64(reserve_data, OFF_COLLATERAL_SUPPLY)? as u128;

    if shares == 0 {
        return Ok(0);
    }
    // Holding shares against a zero-supply reserve is a corrupt state — fail loudly.
    if collateral_supply == 0 {
        return Err(AdapterError::MathOverflow.into());
    }
    let total_liquidity = available
        .checked_add(borrowed)
        .ok_or(AdapterError::MathOverflow)?;
    let value = (shares as u128)
        .checked_mul(total_liquidity)
        .ok_or(AdapterError::MathOverflow)?
        .checked_div(collateral_supply)
        .ok_or(AdapterError::MathOverflow)?;
    u64::try_from(value).map_err(|_| AdapterError::MathOverflow.into())
}

fn u64_data(disc: [u8; 8], amount: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(16);
    buf.extend_from_slice(&disc);
    buf.extend_from_slice(&amount.to_le_bytes());
    buf
}

/// Bundle of accounts forwarded to klend. Holds owned `AccountInfo` clones (cheap —
/// they wrap shared refs) so the caller avoids lifetime tangles.
pub struct KlendAccounts<'info> {
    pub klend_program: AccountInfo<'info>,
    pub owner: AccountInfo<'info>, // position PDA — signer propagates from dispatcher
    pub reserve: AccountInfo<'info>,
    pub lending_market: AccountInfo<'info>,
    pub lending_market_authority: AccountInfo<'info>,
    pub reserve_liquidity_mint: AccountInfo<'info>,
    pub reserve_liquidity_supply: AccountInfo<'info>,
    pub reserve_collateral_mint: AccountInfo<'info>,
    pub user_liquidity: AccountInfo<'info>,  // position_usdc_pool
    pub user_collateral: AccountInfo<'info>, // position-owned cToken account
    pub collateral_token_program: AccountInfo<'info>,
    pub liquidity_token_program: AccountInfo<'info>,
    pub instruction_sysvar: AccountInfo<'info>,
    pub scope_prices: AccountInfo<'info>,
}

/// CPI `refresh_reserve` — updates the reserve's cached price/exchange-rate before
/// a deposit, redeem, or value read. The USDC reserve has no pyth/switchboard
/// oracle, so the klend program id is the None placeholder.
pub fn refresh_reserve<'info>(
    klend_program: &AccountInfo<'info>,
    reserve: &AccountInfo<'info>,
    lending_market: &AccountInfo<'info>,
    scope_prices: &AccountInfo<'info>,
) -> Result<()> {
    let metas = vec![
        AccountMeta::new(reserve.key(), false),
        AccountMeta::new_readonly(lending_market.key(), false),
        AccountMeta::new_readonly(klend_program.key(), false), // pyth_oracle = None
        AccountMeta::new_readonly(klend_program.key(), false), // switchboard_price = None
        AccountMeta::new_readonly(klend_program.key(), false), // switchboard_twap = None
        AccountMeta::new_readonly(scope_prices.key(), false),
    ];
    let infos = [
        reserve.clone(),
        lending_market.clone(),
        klend_program.clone(),
        scope_prices.clone(),
    ];
    let ix = Instruction {
        program_id: klend_program.key(),
        accounts: metas,
        data: KLEND_REFRESH_RESERVE.to_vec(),
    };
    invoke(&ix, &infos).map_err(|e| {
        msg!("klend refresh_reserve failed: {:?}", e);
        error!(AdapterError::CpiFailed)
    })
}

/// CPI `deposit_reserve_liquidity` — `liquidity_amount` USDC in, cTokens out to
/// `user_collateral`. `owner` (position PDA) authorizes the USDC transfer.
pub fn deposit_reserve_liquidity(a: &KlendAccounts, liquidity_amount: u64) -> Result<()> {
    let metas = vec![
        AccountMeta::new_readonly(a.owner.key(), true),
        AccountMeta::new(a.reserve.key(), false),
        AccountMeta::new_readonly(a.lending_market.key(), false),
        AccountMeta::new_readonly(a.lending_market_authority.key(), false),
        AccountMeta::new_readonly(a.reserve_liquidity_mint.key(), false),
        AccountMeta::new(a.reserve_liquidity_supply.key(), false),
        AccountMeta::new(a.reserve_collateral_mint.key(), false),
        AccountMeta::new(a.user_liquidity.key(), false),
        AccountMeta::new(a.user_collateral.key(), false),
        AccountMeta::new_readonly(a.collateral_token_program.key(), false),
        AccountMeta::new_readonly(a.liquidity_token_program.key(), false),
        AccountMeta::new_readonly(a.instruction_sysvar.key(), false),
    ];
    let infos = [
        a.owner.clone(),
        a.reserve.clone(),
        a.lending_market.clone(),
        a.lending_market_authority.clone(),
        a.reserve_liquidity_mint.clone(),
        a.reserve_liquidity_supply.clone(),
        a.reserve_collateral_mint.clone(),
        a.user_liquidity.clone(),
        a.user_collateral.clone(),
        a.collateral_token_program.clone(),
        a.liquidity_token_program.clone(),
        a.instruction_sysvar.clone(),
    ];
    let ix = Instruction {
        program_id: a.klend_program.key(),
        accounts: metas,
        data: u64_data(KLEND_DEPOSIT_RESERVE_LIQUIDITY, liquidity_amount),
    };
    invoke(&ix, &infos).map_err(|e| {
        msg!("klend deposit_reserve_liquidity failed: {:?}", e);
        error!(AdapterError::CpiFailed)
    })
}

/// CPI `redeem_reserve_collateral` — burn `collateral_amount` cTokens, USDC out to
/// `user_liquidity`. Symmetric to deposit; `owner` (position PDA) signs.
pub fn redeem_reserve_collateral(a: &KlendAccounts, collateral_amount: u64) -> Result<()> {
    let metas = vec![
        AccountMeta::new_readonly(a.owner.key(), true),
        AccountMeta::new_readonly(a.lending_market.key(), false),
        AccountMeta::new(a.reserve.key(), false),
        AccountMeta::new_readonly(a.lending_market_authority.key(), false),
        AccountMeta::new_readonly(a.reserve_liquidity_mint.key(), false),
        AccountMeta::new(a.reserve_collateral_mint.key(), false),
        AccountMeta::new(a.reserve_liquidity_supply.key(), false),
        AccountMeta::new(a.user_collateral.key(), false),
        AccountMeta::new(a.user_liquidity.key(), false),
        AccountMeta::new_readonly(a.collateral_token_program.key(), false),
        AccountMeta::new_readonly(a.liquidity_token_program.key(), false),
        AccountMeta::new_readonly(a.instruction_sysvar.key(), false),
    ];
    let infos = [
        a.owner.clone(),
        a.lending_market.clone(),
        a.reserve.clone(),
        a.lending_market_authority.clone(),
        a.reserve_liquidity_mint.clone(),
        a.reserve_collateral_mint.clone(),
        a.reserve_liquidity_supply.clone(),
        a.user_collateral.clone(),
        a.user_liquidity.clone(),
        a.collateral_token_program.clone(),
        a.liquidity_token_program.clone(),
        a.instruction_sysvar.clone(),
    ];
    let ix = Instruction {
        program_id: a.klend_program.key(),
        accounts: metas,
        data: u64_data(KLEND_REDEEM_RESERVE_COLLATERAL, collateral_amount),
    };
    invoke(&ix, &infos).map_err(|e| {
        msg!("klend redeem_reserve_collateral failed: {:?}", e);
        error!(AdapterError::CpiFailed)
    })
}
