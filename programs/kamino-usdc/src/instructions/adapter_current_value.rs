use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;

use adapter_interface::{
    set_u64_return, verify_caller_is_dispatcher, AdapterError, DISPATCHER_ID, MAX_ADAPTER_DATA_LEN,
    ADAPTER_STATE_SEED,
};

use crate::events::AdapterValueEvent;
use crate::klend_cpi;
use crate::state::AdapterState;

/// Return the current USDC value of this position's Kamino holdings.
///
/// Refreshes the reserve (so accrued interest is reflected), computes
/// `total_shares × exchange_rate`, writes the u64 via `set_u64_return` for the
/// dispatcher, and emits `AdapterValueEvent`. Returns ONLY this adapter's holdings.
///
/// Account layout — standard prefix (slots 0–2) then Kamino tail (slots 3–6):
/// 0 instructions_sysvar, 1 position, 2 adapter_state, 3 reserve, 4 lending_market,
/// 5 scope_prices, 6 klend_program.
#[derive(Accounts)]
pub struct AdapterCurrentValue<'info> {
    /// CHECK: instructions sysvar — address enforced; used for caller verification.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    /// CHECK: dispatcher Position PDA. Validated against adapter_state.header.position.
    pub position: AccountInfo<'info>,

    #[account(
        seeds = [ADAPTER_STATE_SEED, crate::ID.as_ref(), position.key().as_ref()],
        bump = adapter_state.bump,
        constraint = adapter_state.header.position == position.key() @ AdapterError::Unauthorized,
    )]
    pub adapter_state: Account<'info, AdapterState>,

    /// CHECK: klend reserve; refreshed then parsed. Validated == adapter_state.kamino_reserve.
    #[account(mut, address = adapter_state.kamino_reserve @ AdapterError::Unauthorized)]
    pub reserve: AccountInfo<'info>,

    /// CHECK: klend lending market; validated by klend during refresh CPI.
    pub lending_market: AccountInfo<'info>,
    /// CHECK: Scope price feed for the reserve (refresh_reserve).
    pub scope_prices: AccountInfo<'info>,
    /// CHECK: klend program — CPI target; validated as the owner of `reserve`.
    #[account(executable, constraint = reserve.owner == klend_program.key @ AdapterError::Unauthorized)]
    pub klend_program: AccountInfo<'info>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, AdapterCurrentValue<'info>>,
    adapter_data: Vec<u8>,
) -> Result<()> {
    require!(
        adapter_data.len() <= MAX_ADAPTER_DATA_LEN,
        AdapterError::AdapterDataTooLarge
    );
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    klend_cpi::refresh_reserve(
        &ctx.accounts.klend_program,
        &ctx.accounts.reserve,
        &ctx.accounts.lending_market,
        &ctx.accounts.scope_prices,
    )?;

    let shares = ctx.accounts.adapter_state.header.total_shares;
    let (value, reserve_slot) = {
        let data = ctx.accounts.reserve.try_borrow_data()?;
        (
            klend_cpi::collateral_value_usdc(&data, shares)?,
            klend_cpi::reserve_last_update_slot(&data)?,
        )
    };

    set_u64_return(value);

    let clock = Clock::get()?;
    let as_of_slot = clock.slot.min(reserve_slot);
    emit!(AdapterValueEvent {
        position: ctx.accounts.adapter_state.header.position,
        current_value_usdc: value,
        as_of_slot,
    });
    msg!("kamino value: {} cTokens = {} USDC", shares, value);
    Ok(())
}
