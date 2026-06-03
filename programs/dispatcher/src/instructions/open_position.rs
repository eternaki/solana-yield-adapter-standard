use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount};

use adapter_interface::ADAPTER_INTERFACE_VERSION;

use crate::constants::*;
use crate::routing::route_initialize;
use crate::error::DispatcherError;
use crate::events::PositionOpened;
use crate::state::{AdapterEntry, Position};

/// Open the caller's routing position into one whitelisted adapter: create the
/// `Position` PDA + its USDC pool, then CPI the adapter's `adapter_initialize`
/// (forwarding adapter-specific accounts via `remaining_accounts`).
#[derive(Accounts)]
pub struct OpenPosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [ENTRY_SEED, adapter_program.key().as_ref()],
        bump = adapter_entry.bump,
        constraint = adapter_entry.is_active @ DispatcherError::AdapterInactive,
    )]
    pub adapter_entry: Account<'info, AdapterEntry>,

    /// CHECK: CPI target; executable. The adapter validates its own routed accounts.
    #[account(executable)]
    pub adapter_program: AccountInfo<'info>,

    #[account(
        init,
        payer = owner,
        space = 8 + Position::INIT_SPACE,
        seeds = [POSITION_SEED, owner.key().as_ref(), adapter_program.key().as_ref()],
        bump,
    )]
    pub position: Account<'info, Position>,

    pub usdc_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = owner,
        associated_token::mint = usdc_mint,
        associated_token::authority = position,
    )]
    pub position_usdc_pool: Account<'info, TokenAccount>,

    /// CHECK: the adapter's state PDA — created by the adapter inside the CPI.
    #[account(mut)]
    pub adapter_state: AccountInfo<'info>,

    /// CHECK: instructions sysvar — forwarded to the adapter for caller verification.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handler<'info>(ctx: Context<'_, '_, '_, 'info, OpenPosition<'info>>) -> Result<()> {
    require!(
        ctx.accounts.adapter_entry.interface_version == ADAPTER_INTERFACE_VERSION,
        DispatcherError::VersionMismatch
    );

    let position_key = ctx.accounts.position.key();
    let owner_key = ctx.accounts.owner.key();
    let adapter_key = ctx.accounts.adapter_program.key();
    let bump = ctx.bumps.position;
    let clock = Clock::get()?;
    {
        let p = &mut ctx.accounts.position;
        p.owner = owner_key;
        p.adapter_program = adapter_key;
        p.usdc_pool = ctx.accounts.position_usdc_pool.key();
        p.usdc_mint = ctx.accounts.usdc_mint.key();
        p.created_at = clock.unix_timestamp;
        p.bump = bump;
    }

    let bump_arr = [bump];
    let seeds: &[&[u8]] = &[POSITION_SEED, owner_key.as_ref(), adapter_key.as_ref(), &bump_arr];
    route_initialize(
        &ctx.accounts.adapter_program,
        &ctx.accounts.instructions_sysvar,
        &ctx.accounts.position.to_account_info(),
        &ctx.accounts.adapter_state,
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.remaining_accounts,
        &[seeds],
    )?;

    emit!(PositionOpened {
        owner: ctx.accounts.owner.key(),
        adapter_program: ctx.accounts.adapter_program.key(),
        position: position_key,
    });
    Ok(())
}
