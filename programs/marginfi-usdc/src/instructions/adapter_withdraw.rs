use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::token::TokenAccount;

use adapter_interface::{
    set_u64_return, verify_caller_is_dispatcher, AdapterError, ADAPTER_STATE_SEED, DISPATCHER_ID,
    MAX_ADAPTER_DATA_LEN,
};

use crate::events::AdapterWithdrawEvent;
use crate::marginfi_cpi;
use crate::state::AdapterState;

/// Withdraw `shares` USDC base units from the MarginFi bank back to the position
/// pool. When `shares >= booked principal`, withdraws the entire balance (incl.
/// accrued interest) and closes it.
///
/// Standard prefix (0–3) then MarginFi tail (4–11):
/// 4 marginfi_program, 5 marginfi_group, 6 marginfi_account, 7 bank,
/// 8 bank_liquidity_vault_authority, 9 liquidity_vault, 10 token_program, 11 oracle.
#[derive(Accounts)]
pub struct AdapterWithdraw<'info> {
    /// CHECK: instructions sysvar.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    /// CHECK: dispatcher Position PDA; signs the MarginFi CPI (propagated).
    pub position: AccountInfo<'info>,

    #[account(mut)]
    pub position_usdc_pool: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [ADAPTER_STATE_SEED, crate::ID.as_ref(), position.key().as_ref()],
        bump = adapter_state.bump,
        constraint = adapter_state.header.position == position.key() @ AdapterError::Unauthorized,
    )]
    pub adapter_state: Account<'info, AdapterState>,

    // ----- MarginFi-specific -----
    /// CHECK: MarginFi program — CPI target.
    #[account(executable)]
    pub marginfi_program: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.marginfi_group.
    #[account(address = adapter_state.marginfi_group @ AdapterError::Unauthorized)]
    pub marginfi_group: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.marginfi_account.
    #[account(mut, address = adapter_state.marginfi_account @ AdapterError::Unauthorized)]
    pub marginfi_account: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.bank.
    #[account(mut, address = adapter_state.bank @ AdapterError::Unauthorized)]
    pub bank: AccountInfo<'info>,
    /// CHECK: bank liquidity vault authority PDA; validated by MarginFi during CPI.
    pub bank_liquidity_vault_authority: AccountInfo<'info>,
    /// CHECK: bank liquidity vault; validated by MarginFi during CPI.
    #[account(mut)]
    pub liquidity_vault: AccountInfo<'info>,
    /// CHECK: SPL token program.
    pub token_program: AccountInfo<'info>,
    /// CHECK: the bank's price oracle (health-pulse). Validated by MarginFi.
    pub oracle: AccountInfo<'info>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, AdapterWithdraw<'info>>,
    shares: u64,
    adapter_data: Vec<u8>,
) -> Result<()> {
    require!(shares > 0, AdapterError::ZeroAmount);
    require!(
        adapter_data.len() <= MAX_ADAPTER_DATA_LEN,
        AdapterError::AdapterDataTooLarge
    );
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    let booked = ctx.accounts.adapter_state.header.total_shares;
    let withdraw_all = shares >= booked;

    let usdc_before = ctx.accounts.position_usdc_pool.amount;
    marginfi_cpi::withdraw(
        &ctx.accounts.marginfi_program,
        &ctx.accounts.marginfi_group,
        &ctx.accounts.marginfi_account,
        &ctx.accounts.position,
        &ctx.accounts.bank,
        &ctx.accounts.position_usdc_pool.to_account_info(),
        &ctx.accounts.bank_liquidity_vault_authority,
        &ctx.accounts.liquidity_vault,
        &ctx.accounts.token_program,
        &ctx.accounts.oracle,
        shares,
        withdraw_all,
    )?;
    ctx.accounts.position_usdc_pool.reload()?;
    let amount_out = ctx
        .accounts
        .position_usdc_pool
        .amount
        .checked_sub(usdc_before)
        .ok_or(AdapterError::MathOverflow)?;

    let state = &mut ctx.accounts.adapter_state;
    state.header.total_shares = if withdraw_all {
        0
    } else {
        state.header.total_shares.checked_sub(shares).ok_or(AdapterError::MathOverflow)?
    };

    set_u64_return(amount_out);
    emit!(AdapterWithdrawEvent {
        position: state.header.position,
        shares_burned: shares,
        amount_out,
    });
    msg!("marginfi withdraw: {} -> {} USDC (all={})", shares, amount_out, withdraw_all);
    Ok(())
}
