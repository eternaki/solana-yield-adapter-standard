use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount};

use adapter_interface::{
    verify_caller_is_dispatcher, AdapterError, AdapterStateHeader, ADAPTER_STATE_SEED,
    DISPATCHER_ID, MAX_ADAPTER_DATA_LEN,
};

use crate::state::AdapterState;

/// Create the `AdapterState` PDA and the position-owned syrupUSDC token account.
///
/// Standard prefix (0–4) then tail (5–9):
/// 5 syrup_mint, 6 syrup_token_account, 7 whirlpool, 8 token_program,
/// 9 associated_token_program.
#[derive(Accounts)]
pub struct AdapterInitialize<'info> {
    /// CHECK: instructions sysvar.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
    /// CHECK: dispatcher Position PDA — syrupUSDC account authority + adapter_state seed.
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

    pub syrup_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = rent_payer,
        associated_token::mint = syrup_mint,
        associated_token::authority = position,
    )]
    pub syrup_token_account: Account<'info, TokenAccount>,
    /// CHECK: the Orca whirlpool used to swap; recorded for later validation.
    pub whirlpool: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handler(ctx: Context<AdapterInitialize>, adapter_data: Vec<u8>) -> Result<()> {
    require!(adapter_data.len() <= MAX_ADAPTER_DATA_LEN, AdapterError::AdapterDataTooLarge);
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    let clock = Clock::get()?;
    let state = &mut ctx.accounts.adapter_state;
    state.header = AdapterStateHeader {
        position: ctx.accounts.position.key(),
        adapter_program: crate::ID,
        created_at: clock.unix_timestamp,
        total_shares: 0,
    };
    state.whirlpool = ctx.accounts.whirlpool.key();
    state.syrup_token_account = ctx.accounts.syrup_token_account.key();
    state.bump = ctx.bumps.adapter_state;
    msg!("maple-syrup init: position={} whirlpool={}", state.header.position, state.whirlpool);
    Ok(())
}
