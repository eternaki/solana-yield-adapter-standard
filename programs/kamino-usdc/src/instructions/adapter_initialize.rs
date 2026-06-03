use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount};

use adapter_interface::{
    verify_caller_is_dispatcher, AdapterError, AdapterStateHeader, ADAPTER_STATE_SEED,
    DISPATCHER_ID, MAX_ADAPTER_DATA_LEN,
};

use crate::state::AdapterState;

/// Create the per-position `AdapterState` PDA and the position-owned collateral
/// (cToken) account. CPI'd by the dispatcher's `open_position`.
///
/// Account layout — standard prefix (slots 0–4) then Kamino tail (slots 5–9):
/// 0 instructions_sysvar, 1 position, 2 adapter_state, 3 rent_payer, 4 system_program,
/// 5 reserve, 6 reserve_collateral_mint, 7 collateral_vault, 8 token_program,
/// 9 associated_token_program.
#[derive(Accounts)]
pub struct AdapterInitialize<'info> {
    /// CHECK: instructions sysvar — address enforced; used for caller verification.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    /// CHECK: dispatcher Position PDA — adapter_state seed + collateral authority.
    pub position: AccountInfo<'info>,

    #[account(
        init,
        payer = rent_payer,
        space = 8 + AdapterState::INIT_SPACE,
        seeds = [ADAPTER_STATE_SEED, crate::ID.as_ref(), position.key().as_ref()],
        bump,
    )]
    pub adapter_state: Account<'info, AdapterState>,

    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,

    // ----- Kamino-specific -----
    /// CHECK: the klend reserve; only its key is recorded. klend validates the
    /// reserve↔collateral-mint binding at deposit time.
    pub reserve: AccountInfo<'info>,

    /// Reserve collateral mint (cToken) — derives the position's collateral ATA.
    pub reserve_collateral_mint: Account<'info, Mint>,

    /// Position-owned collateral account; authority is the Position PDA so
    /// deposit/withdraw use one consistent signer.
    #[account(
        init,
        payer = rent_payer,
        associated_token::mint = reserve_collateral_mint,
        associated_token::authority = position,
    )]
    pub collateral_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handler(ctx: Context<AdapterInitialize>, adapter_data: Vec<u8>) -> Result<()> {
    require!(
        adapter_data.len() <= MAX_ADAPTER_DATA_LEN,
        AdapterError::AdapterDataTooLarge
    );
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    let clock = Clock::get()?;
    let state = &mut ctx.accounts.adapter_state;
    state.header = AdapterStateHeader {
        position: ctx.accounts.position.key(),
        adapter_program: crate::ID,
        created_at: clock.unix_timestamp,
        total_shares: 0,
    };
    state.kamino_reserve = ctx.accounts.reserve.key();
    state.collateral_vault = ctx.accounts.collateral_vault.key();
    state.bump = ctx.bumps.adapter_state;

    msg!(
        "kamino-usdc init: position={} reserve={} collateral_vault={}",
        state.header.position,
        state.kamino_reserve,
        state.collateral_vault
    );
    Ok(())
}
