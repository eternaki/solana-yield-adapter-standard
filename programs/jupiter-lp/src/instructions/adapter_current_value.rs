use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;

use adapter_interface::{
    set_u64_return, verify_caller_is_dispatcher, AdapterError, ADAPTER_STATE_SEED, DISPATCHER_ID,
    MAX_ADAPTER_DATA_LEN,
};

use crate::events::AdapterValueEvent;
use crate::jup_cpi;
use crate::state::AdapterState;

/// Return the position's USDC value: JLP balance × pool.aumUsd / JLP supply.
///
/// Standard prefix (0–2) then Jupiter tail (3–5):
/// 0 instructions_sysvar, 1 position, 2 adapter_state, 3 lp_token_account, 4 pool,
/// 5 lp_token_mint.
#[derive(Accounts)]
pub struct AdapterCurrentValue<'info> {
    /// CHECK: instructions sysvar.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
    /// CHECK: dispatcher Position PDA. Validated against adapter_state.
    pub position: AccountInfo<'info>,
    #[account(
        seeds = [ADAPTER_STATE_SEED, crate::ID.as_ref(), position.key().as_ref()],
        bump = adapter_state.bump,
        constraint = adapter_state.header.position == position.key() @ AdapterError::Unauthorized,
    )]
    pub adapter_state: Account<'info, AdapterState>,
    /// CHECK: validated == adapter_state.lp_token_account; read for JLP balance.
    #[account(address = adapter_state.lp_token_account @ AdapterError::Unauthorized)]
    pub lp_token_account: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.pool; read for aumUsd.
    #[account(address = adapter_state.pool @ AdapterError::Unauthorized)]
    pub pool: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.lp_mint; read for total supply.
    #[account(address = adapter_state.lp_mint @ AdapterError::Unauthorized)]
    pub lp_token_mint: AccountInfo<'info>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, AdapterCurrentValue<'info>>,
    adapter_data: Vec<u8>,
) -> Result<()> {
    require!(adapter_data.len() <= MAX_ADAPTER_DATA_LEN, AdapterError::AdapterDataTooLarge);
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    let value = {
        let lp_acct = ctx.accounts.lp_token_account.try_borrow_data()?;
        let pool = ctx.accounts.pool.try_borrow_data()?;
        let mint = ctx.accounts.lp_token_mint.try_borrow_data()?;
        let lp_balance = jup_cpi::token_amount(&lp_acct)?;
        jup_cpi::lp_value_usdc(&pool, &mint, lp_balance)?
    };

    set_u64_return(value);
    let clock = Clock::get()?;
    emit!(AdapterValueEvent {
        position: ctx.accounts.adapter_state.header.position,
        current_value_usdc: value,
        as_of_slot: clock.slot,
    });
    msg!("jupiter-lp value: {} USDC", value);
    Ok(())
}
