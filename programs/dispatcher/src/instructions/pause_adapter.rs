use anchor_lang::prelude::*;

use crate::constants::*;
use crate::error::DispatcherError;
use crate::events::AdapterPaused;
use crate::state::{AdapterEntry, Registry};

#[derive(Accounts)]
pub struct PauseAdapter<'info> {
    pub admin: Signer<'info>,

    #[account(
        seeds = [REGISTRY_SEED],
        bump = registry.bump,
        constraint = registry.admin == admin.key() @ DispatcherError::Unauthorized,
    )]
    pub registry: Account<'info, Registry>,

    #[account(
        mut,
        seeds = [ENTRY_SEED, adapter_entry.adapter_program.as_ref()],
        bump = adapter_entry.bump,
    )]
    pub adapter_entry: Account<'info, AdapterEntry>,
}

/// `paused = true` deactivates routing; `false` re-activates. The entry is never
/// removed, so `adapter_count` stays monotonic and history is preserved.
pub fn handler(ctx: Context<PauseAdapter>, paused: bool) -> Result<()> {
    let entry = &mut ctx.accounts.adapter_entry;
    entry.is_active = !paused;
    emit!(AdapterPaused {
        adapter_program: entry.adapter_program,
        is_active: entry.is_active,
    });
    Ok(())
}
