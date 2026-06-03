use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;

use adapter_interface::{
    set_u64_return, verify_caller_is_dispatcher, AdapterError, ADAPTER_STATE_SEED, DISPATCHER_ID,
    MAX_ADAPTER_DATA_LEN,
};

use crate::events::AdapterValueEvent;
use crate::marginfi_cpi;
use crate::state::AdapterState;

/// Return the position's USDC value: `asset_shares × bank.asset_share_value`.
///
/// Standard prefix (0–2) then MarginFi tail (3–4):
/// 0 instructions_sysvar, 1 position, 2 adapter_state, 3 marginfi_account, 4 bank.
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

    /// CHECK: validated == adapter_state.marginfi_account; read for asset shares.
    #[account(address = adapter_state.marginfi_account @ AdapterError::Unauthorized)]
    pub marginfi_account: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.bank; read for asset_share_value.
    #[account(address = adapter_state.bank @ AdapterError::Unauthorized)]
    pub bank: AccountInfo<'info>,
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

    let bank = &ctx.accounts.adapter_state.bank;
    let value = {
        let acct = ctx.accounts.marginfi_account.try_borrow_data()?;
        let bank_data = ctx.accounts.bank.try_borrow_data()?;
        let shares = marginfi_cpi::account_asset_shares(&acct, bank)?;
        let share_value = marginfi_cpi::bank_asset_share_value(&bank_data)?;
        marginfi_cpi::shares_to_usdc(shares, share_value)?
    };

    set_u64_return(value);
    let clock = Clock::get()?;
    emit!(AdapterValueEvent {
        position: ctx.accounts.adapter_state.header.position,
        current_value_usdc: value,
        as_of_slot: clock.slot,
    });
    msg!("marginfi value: {} USDC", value);
    Ok(())
}
