use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use adapter_interface::ADAPTER_INTERFACE_VERSION;

use crate::constants::*;
use crate::routing::route_deposit;
use crate::error::DispatcherError;
use crate::events::Deposited;
use crate::state::{AdapterEntry, Position};

/// Route `amount` USDC from the owner into the position's adapter. Moves USDC
/// owner → position pool, then CPIs `adapter_deposit` signed by the position PDA;
/// reads `shares_minted` from the adapter's return data.
#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [ENTRY_SEED, adapter_program.key().as_ref()],
        bump = adapter_entry.bump,
        constraint = adapter_entry.is_active @ DispatcherError::AdapterInactive,
    )]
    pub adapter_entry: Account<'info, AdapterEntry>,

    /// CHECK: CPI target; executable + must match the position's adapter.
    #[account(
        executable,
        constraint = adapter_program.key() == position.adapter_program @ DispatcherError::AdapterMismatch,
    )]
    pub adapter_program: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [POSITION_SEED, owner.key().as_ref(), adapter_program.key().as_ref()],
        bump = position.bump,
        has_one = owner @ DispatcherError::Unauthorized,
    )]
    pub position: Account<'info, Position>,

    #[account(mut, address = position.usdc_pool @ DispatcherError::AdapterMismatch)]
    pub position_usdc_pool: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = position.usdc_mint,
        token::authority = owner,
    )]
    pub owner_usdc_ata: Account<'info, TokenAccount>,

    /// CHECK: the adapter's state PDA — passed through to the adapter.
    #[account(mut)]
    pub adapter_state: AccountInfo<'info>,

    /// CHECK: instructions sysvar — forwarded for caller verification.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler<'info>(ctx: Context<'_, '_, '_, 'info, Deposit<'info>>, amount: u64) -> Result<()> {
    require!(amount > 0, DispatcherError::ZeroAmount);
    require!(
        ctx.accounts.adapter_entry.interface_version == ADAPTER_INTERFACE_VERSION,
        DispatcherError::VersionMismatch
    );

    // Owner funds the position pool (owner signs).
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.owner_usdc_ata.to_account_info(),
                to: ctx.accounts.position_usdc_pool.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            },
        ),
        amount,
    )?;

    let owner_key = ctx.accounts.position.owner;
    let adapter_key = ctx.accounts.position.adapter_program;
    let bump = [ctx.accounts.position.bump];
    let seeds: &[&[u8]] = &[POSITION_SEED, owner_key.as_ref(), adapter_key.as_ref(), &bump];

    let shares = route_deposit(
        &ctx.accounts.adapter_program,
        &ctx.accounts.instructions_sysvar,
        &ctx.accounts.position.to_account_info(),
        &ctx.accounts.position_usdc_pool.to_account_info(),
        &ctx.accounts.adapter_state,
        ctx.remaining_accounts,
        amount,
        &[seeds],
    )?;

    emit!(Deposited {
        position: ctx.accounts.position.key(),
        amount_in: amount,
        shares_minted: shares,
    });
    Ok(())
}
