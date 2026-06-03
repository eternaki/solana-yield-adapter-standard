use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;

use adapter_interface::{
    set_u64_return, verify_caller_is_dispatcher, AdapterError, ADAPTER_STATE_SEED, DISPATCHER_ID,
    MAX_ADAPTER_DATA_LEN,
};

use crate::drift_cpi;
use crate::events::AdapterValueEvent;
use crate::state::AdapterState;

/// Return the position's USDC value: IF shares × fund vault / total IF shares.
///
/// Standard prefix (0–2) then Drift tail (3–5):
/// 0 instructions_sysvar, 1 position, 2 adapter_state, 3 insurance_fund_stake,
/// 4 spot_market, 5 insurance_fund_vault.
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
    /// CHECK: validated == adapter_state.insurance_fund_stake; read for shares.
    #[account(address = adapter_state.insurance_fund_stake @ AdapterError::Unauthorized)]
    pub insurance_fund_stake: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.spot_market; read for total IF shares.
    #[account(address = adapter_state.spot_market @ AdapterError::Unauthorized)]
    pub spot_market: AccountInfo<'info>,
    /// CHECK: insurance fund vault; read for USDC balance.
    pub insurance_fund_vault: AccountInfo<'info>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, AdapterCurrentValue<'info>>,
    adapter_data: Vec<u8>,
) -> Result<()> {
    require!(adapter_data.len() <= MAX_ADAPTER_DATA_LEN, AdapterError::AdapterDataTooLarge);
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    let value = {
        let stake = ctx.accounts.insurance_fund_stake.try_borrow_data()?;
        let sm = ctx.accounts.spot_market.try_borrow_data()?;
        let vault = ctx.accounts.insurance_fund_vault.try_borrow_data()?;
        drift_cpi::stake_value_usdc(&stake, &sm, &vault)?
    };

    set_u64_return(value);
    let clock = Clock::get()?;
    emit!(AdapterValueEvent {
        position: ctx.accounts.adapter_state.header.position,
        current_value_usdc: value,
        as_of_slot: clock.slot,
    });
    msg!("drift-if value: {} USDC", value);
    Ok(())
}
