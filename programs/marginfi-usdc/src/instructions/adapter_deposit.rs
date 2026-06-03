use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::token::TokenAccount;

use adapter_interface::{
    set_u64_return, verify_caller_is_dispatcher, AdapterError, ADAPTER_STATE_SEED, DISPATCHER_ID,
    MAX_ADAPTER_DATA_LEN,
};

use crate::events::AdapterDepositEvent;
use crate::marginfi_cpi;
use crate::state::AdapterState;

/// Deposit `amount` USDC from the position pool into the MarginFi bank.
///
/// Standard prefix (0–3) then MarginFi tail (4–9):
/// 0 instructions_sysvar, 1 position, 2 position_usdc_pool, 3 adapter_state,
/// 4 marginfi_program, 5 marginfi_group, 6 marginfi_account, 7 bank, 8 liquidity_vault,
/// 9 token_program.
#[derive(Accounts)]
pub struct AdapterDeposit<'info> {
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
    /// CHECK: bank liquidity vault; validated by MarginFi during CPI.
    #[account(mut)]
    pub liquidity_vault: AccountInfo<'info>,
    /// CHECK: SPL token program.
    pub token_program: AccountInfo<'info>,
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

    marginfi_cpi::deposit(
        &ctx.accounts.marginfi_program,
        &ctx.accounts.marginfi_group,
        &ctx.accounts.marginfi_account,
        &ctx.accounts.position,
        &ctx.accounts.bank,
        &ctx.accounts.position_usdc_pool.to_account_info(),
        &ctx.accounts.liquidity_vault,
        &ctx.accounts.token_program,
        amount,
    )?;

    let state = &mut ctx.accounts.adapter_state;
    // MarginFi shares are I80F48; we book the deposited principal (USDC) as shares.
    state.header.total_shares = state
        .header
        .total_shares
        .checked_add(amount)
        .ok_or(AdapterError::MathOverflow)?;

    set_u64_return(amount);
    emit!(AdapterDepositEvent {
        position: state.header.position,
        amount_in: amount,
        shares_minted: amount,
    });
    msg!("marginfi deposit: {} USDC", amount);
    Ok(())
}
