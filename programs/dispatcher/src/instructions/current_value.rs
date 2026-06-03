use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::set_return_data, sysvar};

use crate::constants::*;
use crate::routing::route_current_value;
use crate::error::DispatcherError;
use crate::events::ValueRead;
use crate::state::{AdapterEntry, Position};

/// Read the current USDC value of a position by CPI-ing the adapter's read-only
/// `adapter_current_value`. Re-emits the value via `set_return_data` so this
/// instruction is itself composable for an outer caller.
#[derive(Accounts)]
pub struct CurrentValue<'info> {
    #[account(
        seeds = [ENTRY_SEED, adapter_program.key().as_ref()],
        bump = adapter_entry.bump,
    )]
    pub adapter_entry: Account<'info, AdapterEntry>,

    /// CHECK: CPI target; executable + must match the position's adapter.
    #[account(
        executable,
        constraint = adapter_program.key() == position.adapter_program @ DispatcherError::AdapterMismatch,
    )]
    pub adapter_program: AccountInfo<'info>,

    #[account(
        seeds = [POSITION_SEED, position.owner.as_ref(), adapter_program.key().as_ref()],
        bump = position.bump,
    )]
    pub position: Account<'info, Position>,

    /// CHECK: the adapter's state PDA — passed through to the adapter.
    pub adapter_state: AccountInfo<'info>,

    /// CHECK: instructions sysvar — forwarded for caller verification.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
}

pub fn handler<'info>(ctx: Context<'_, '_, '_, 'info, CurrentValue<'info>>) -> Result<()> {
    let value = route_current_value(
        &ctx.accounts.adapter_program,
        &ctx.accounts.instructions_sysvar,
        &ctx.accounts.position.to_account_info(),
        &ctx.accounts.adapter_state,
        ctx.remaining_accounts,
    )?;

    set_return_data(&value.to_le_bytes());
    emit!(ValueRead {
        position: ctx.accounts.position.key(),
        value_usdc: value,
    });
    Ok(())
}
