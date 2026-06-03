use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::token::TokenAccount;

use adapter_interface::{
    set_u64_return, verify_caller_is_dispatcher, AdapterError, ADAPTER_STATE_SEED, DISPATCHER_ID,
    MAX_ADAPTER_DATA_LEN,
};

use crate::events::AdapterWithdrawEvent;
use crate::orca_cpi::{self, SwapAccounts};
use crate::state::AdapterState;

/// Swap `shares` syrupUSDC → USDC via the whirlpool. Returns USDC received.
/// Account layout identical to `adapter_deposit` (Orca swap tail).
#[derive(Accounts)]
pub struct AdapterWithdraw<'info> {
    /// CHECK: instructions sysvar.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
    /// CHECK: dispatcher Position PDA; signs the swap (propagated).
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

    /// CHECK: Orca whirlpools program — CPI target.
    #[account(executable)]
    pub orca_program: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.whirlpool.
    #[account(mut, address = adapter_state.whirlpool @ AdapterError::Unauthorized)]
    pub whirlpool: AccountInfo<'info>,
    #[account(mut, address = adapter_state.syrup_token_account @ AdapterError::Unauthorized)]
    pub syrup_token_account: Account<'info, TokenAccount>,
    /// CHECK: whirlpool vault A.
    #[account(mut)]
    pub vault_a: AccountInfo<'info>,
    /// CHECK: whirlpool vault B.
    #[account(mut)]
    pub vault_b: AccountInfo<'info>,
    /// CHECK: tick array.
    #[account(mut)]
    pub tick_array_0: AccountInfo<'info>,
    /// CHECK: tick array.
    #[account(mut)]
    pub tick_array_1: AccountInfo<'info>,
    /// CHECK: tick array.
    #[account(mut)]
    pub tick_array_2: AccountInfo<'info>,
    /// CHECK: whirlpool oracle.
    #[account(mut)]
    pub oracle: AccountInfo<'info>,
    /// CHECK: SPL token program.
    pub token_program: AccountInfo<'info>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, AdapterWithdraw<'info>>,
    shares: u64,
    adapter_data: Vec<u8>,
) -> Result<()> {
    require!(shares > 0, AdapterError::ZeroAmount);
    require!(adapter_data.len() <= MAX_ADAPTER_DATA_LEN, AdapterError::AdapterDataTooLarge);
    require!(
        shares <= ctx.accounts.adapter_state.header.total_shares,
        AdapterError::InsufficientShares
    );
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    let accs = SwapAccounts {
        program: ctx.accounts.orca_program.clone(),
        token_program: ctx.accounts.token_program.clone(),
        authority: ctx.accounts.position.clone(),
        whirlpool: ctx.accounts.whirlpool.clone(),
        token_owner_a: ctx.accounts.syrup_token_account.to_account_info(),
        vault_a: ctx.accounts.vault_a.clone(),
        token_owner_b: ctx.accounts.position_usdc_pool.to_account_info(),
        vault_b: ctx.accounts.vault_b.clone(),
        tick_array_0: ctx.accounts.tick_array_0.clone(),
        tick_array_1: ctx.accounts.tick_array_1.clone(),
        tick_array_2: ctx.accounts.tick_array_2.clone(),
        oracle: ctx.accounts.oracle.clone(),
    };

    let before = ctx.accounts.position_usdc_pool.amount;
    orca_cpi::swap(&accs, shares, true)?; // syrupUSDC (A) -> USDC (B)
    ctx.accounts.position_usdc_pool.reload()?;
    let amount_out = ctx
        .accounts
        .position_usdc_pool
        .amount
        .checked_sub(before)
        .ok_or(AdapterError::MathOverflow)?;

    let state = &mut ctx.accounts.adapter_state;
    state.header.total_shares = state.header.total_shares.saturating_sub(shares);

    set_u64_return(amount_out);
    emit!(AdapterWithdrawEvent {
        position: state.header.position,
        shares_burned: shares,
        amount_out,
    });
    msg!("maple withdraw: {} syrupUSDC -> {} USDC", shares, amount_out);
    Ok(())
}
