use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::token::TokenAccount;

use adapter_interface::{
    set_u64_return, verify_caller_is_dispatcher, AdapterError, DISPATCHER_ID, MAX_ADAPTER_DATA_LEN,
    ADAPTER_STATE_SEED,
};

use crate::events::AdapterDepositEvent;
use crate::klend_cpi::{self, KlendAccounts};
use crate::state::AdapterState;

/// Deposit `amount` USDC from the position pool into the Kamino reserve, receiving
/// cTokens into the position-owned collateral account. Returns `shares_minted`.
///
/// Account layout — standard prefix (slots 0–3) then Kamino tail (slots 4–14):
/// 0 instructions_sysvar, 1 position, 2 position_usdc_pool, 3 adapter_state,
/// 4 reserve, 5 lending_market, 6 lending_market_authority, 7 reserve_liquidity_mint,
/// 8 reserve_liquidity_supply, 9 reserve_collateral_mint, 10 collateral_vault,
/// 11 collateral_token_program, 12 liquidity_token_program, 13 scope_prices, 14 klend_program.
#[derive(Accounts)]
pub struct AdapterDeposit<'info> {
    /// CHECK: instructions sysvar — address enforced; used for caller verification.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    /// CHECK: dispatcher Position PDA; signs the klend CPI as `owner` (flag
    /// propagates from the dispatcher's invoke_signed).
    pub position: AccountInfo<'info>,

    /// Source USDC token account owned by the Position PDA (klend `userSourceLiquidity`).
    #[account(mut)]
    pub position_usdc_pool: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [ADAPTER_STATE_SEED, crate::ID.as_ref(), position.key().as_ref()],
        bump = adapter_state.bump,
        constraint = adapter_state.header.position == position.key() @ AdapterError::Unauthorized,
    )]
    pub adapter_state: Account<'info, AdapterState>,

    // ----- Kamino-specific (slots 4+) -----
    /// CHECK: klend reserve; validated == adapter_state.kamino_reserve.
    #[account(mut, address = adapter_state.kamino_reserve @ AdapterError::Unauthorized)]
    pub reserve: AccountInfo<'info>,

    /// CHECK: klend lending market; validated by klend during CPI.
    pub lending_market: AccountInfo<'info>,
    /// CHECK: klend lending-market authority PDA; validated by klend during CPI.
    pub lending_market_authority: AccountInfo<'info>,
    /// CHECK: reserve liquidity mint (USDC); validated by klend during CPI.
    pub reserve_liquidity_mint: AccountInfo<'info>,
    /// CHECK: reserve liquidity supply vault; validated by klend during CPI.
    #[account(mut)]
    pub reserve_liquidity_supply: AccountInfo<'info>,
    /// CHECK: reserve collateral (cToken) mint; validated by klend during CPI.
    #[account(mut)]
    pub reserve_collateral_mint: AccountInfo<'info>,

    /// Position-owned collateral account (klend `userDestinationCollateral`).
    #[account(mut, address = adapter_state.collateral_vault @ AdapterError::Unauthorized)]
    pub collateral_vault: Account<'info, TokenAccount>,

    /// CHECK: SPL token program for the collateral mint.
    pub collateral_token_program: AccountInfo<'info>,
    /// CHECK: SPL token program for the liquidity (USDC) mint.
    pub liquidity_token_program: AccountInfo<'info>,
    /// CHECK: Scope price feed for the reserve (refresh_reserve).
    pub scope_prices: AccountInfo<'info>,
    /// CHECK: klend program — CPI target; validated as the owner of `reserve`.
    #[account(executable, constraint = reserve.owner == klend_program.key @ AdapterError::Unauthorized)]
    pub klend_program: AccountInfo<'info>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, AdapterDeposit<'info>>,
    amount: u64,
    adapter_data: Vec<u8>,
) -> Result<()> {
    require!(amount > 0, AdapterError::ZeroAmount);
    require!(
        adapter_data.len() <= MAX_ADAPTER_DATA_LEN,
        AdapterError::AdapterDataTooLarge
    );
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    let klend = KlendAccounts {
        klend_program: ctx.accounts.klend_program.clone(),
        owner: ctx.accounts.position.clone(),
        reserve: ctx.accounts.reserve.clone(),
        lending_market: ctx.accounts.lending_market.clone(),
        lending_market_authority: ctx.accounts.lending_market_authority.clone(),
        reserve_liquidity_mint: ctx.accounts.reserve_liquidity_mint.clone(),
        reserve_liquidity_supply: ctx.accounts.reserve_liquidity_supply.clone(),
        reserve_collateral_mint: ctx.accounts.reserve_collateral_mint.clone(),
        user_liquidity: ctx.accounts.position_usdc_pool.to_account_info(),
        user_collateral: ctx.accounts.collateral_vault.to_account_info(),
        collateral_token_program: ctx.accounts.collateral_token_program.clone(),
        liquidity_token_program: ctx.accounts.liquidity_token_program.clone(),
        instruction_sysvar: ctx.accounts.instructions_sysvar.clone(),
        scope_prices: ctx.accounts.scope_prices.clone(),
    };

    klend_cpi::refresh_reserve(
        &klend.klend_program,
        &klend.reserve,
        &klend.lending_market,
        &klend.scope_prices,
    )?;

    let shares_before = ctx.accounts.collateral_vault.amount;
    klend_cpi::deposit_reserve_liquidity(&klend, amount)?;
    ctx.accounts.collateral_vault.reload()?;
    let shares_after = ctx.accounts.collateral_vault.amount;

    let shares_minted = shares_after
        .checked_sub(shares_before)
        .ok_or(AdapterError::MathOverflow)?;

    let state = &mut ctx.accounts.adapter_state;
    state.header.total_shares = state
        .header
        .total_shares
        .checked_add(shares_minted)
        .ok_or(AdapterError::MathOverflow)?;

    // Canonical result for the dispatcher, plus an event for indexers.
    set_u64_return(shares_minted);
    emit!(AdapterDepositEvent {
        position: state.header.position,
        amount_in: amount,
        shares_minted,
    });
    msg!("kamino deposit: {} USDC -> {} cTokens", amount, shares_minted);
    Ok(())
}
