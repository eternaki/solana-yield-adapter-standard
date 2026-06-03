use anchor_lang::prelude::*;

use adapter_interface::ADAPTER_INTERFACE_VERSION;

use crate::constants::*;
use crate::error::DispatcherError;
use crate::events::AdapterWhitelisted;
use crate::state::{AdapterEntry, Registry};

#[derive(Accounts)]
#[instruction(name: String, interface_version: u8)]
pub struct WhitelistAdapter<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [REGISTRY_SEED],
        bump = registry.bump,
        constraint = registry.admin == admin.key() @ DispatcherError::Unauthorized,
    )]
    pub registry: Account<'info, Registry>,

    #[account(
        init,
        payer = admin,
        space = 8 + AdapterEntry::INIT_SPACE,
        seeds = [ENTRY_SEED, adapter_program.key().as_ref()],
        bump,
    )]
    pub adapter_entry: Account<'info, AdapterEntry>,

    /// CHECK: adapter program — must be executable; only its key is recorded.
    #[account(executable)]
    pub adapter_program: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WhitelistAdapter>, name: String, interface_version: u8) -> Result<()> {
    require!(name.len() <= MAX_NAME_LEN, DispatcherError::NameTooLong);
    require!(
        interface_version == ADAPTER_INTERFACE_VERSION,
        DispatcherError::VersionMismatch
    );
    require!(
        ctx.accounts.registry.adapter_count < MAX_ADAPTERS,
        DispatcherError::RegistryFull
    );

    let clock = Clock::get()?;
    let entry = &mut ctx.accounts.adapter_entry;
    entry.adapter_program = ctx.accounts.adapter_program.key();
    entry.interface_version = interface_version;
    entry.name = name.clone();
    entry.is_active = true;
    entry.added_at = clock.unix_timestamp;
    entry.bump = ctx.bumps.adapter_entry;

    let registry = &mut ctx.accounts.registry;
    registry.adapter_count = registry
        .adapter_count
        .checked_add(1)
        .ok_or(DispatcherError::MathOverflow)?;

    emit!(AdapterWhitelisted {
        adapter_program: entry.adapter_program,
        name,
        interface_version,
    });
    Ok(())
}
