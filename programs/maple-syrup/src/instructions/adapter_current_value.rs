use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;

use adapter_interface::{
    set_u64_return, verify_caller_is_dispatcher, AdapterError, ADAPTER_STATE_SEED, DISPATCHER_ID,
    MAX_ADAPTER_DATA_LEN,
};

use crate::events::AdapterValueEvent;
use crate::orca_cpi;
use crate::state::AdapterState;

/// Return the position's USDC value: syrupUSDC balance × the whirlpool spot price.
///
/// Standard prefix (0–2) then tail (3–4):
/// 0 instructions_sysvar, 1 position, 2 adapter_state, 3 syrup_token_account, 4 whirlpool.
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
    /// CHECK: validated == adapter_state.syrup_token_account; read for balance.
    #[account(address = adapter_state.syrup_token_account @ AdapterError::Unauthorized)]
    pub syrup_token_account: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.whirlpool; read for spot price.
    #[account(address = adapter_state.whirlpool @ AdapterError::Unauthorized)]
    pub whirlpool: AccountInfo<'info>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, AdapterCurrentValue<'info>>,
    adapter_data: Vec<u8>,
) -> Result<()> {
    require!(adapter_data.len() <= MAX_ADAPTER_DATA_LEN, AdapterError::AdapterDataTooLarge);
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    let value = {
        let syrup = ctx.accounts.syrup_token_account.try_borrow_data()?;
        let wp = ctx.accounts.whirlpool.try_borrow_data()?;
        let balance = orca_cpi::token_amount(&syrup)?;
        orca_cpi::whirlpool_value_usdc(&wp, balance)?
    };

    set_u64_return(value);
    let clock = Clock::get()?;
    emit!(AdapterValueEvent {
        position: ctx.accounts.adapter_state.header.position,
        current_value_usdc: value,
        as_of_slot: clock.slot,
    });
    msg!("maple value: {} USDC", value);
    Ok(())
}
