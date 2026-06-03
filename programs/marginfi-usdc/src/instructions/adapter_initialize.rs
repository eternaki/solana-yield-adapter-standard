use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;

use adapter_interface::{
    verify_caller_is_dispatcher, AdapterError, AdapterStateHeader, ADAPTER_STATE_SEED,
    DISPATCHER_ID, MAX_ADAPTER_DATA_LEN,
};

use crate::marginfi_cpi;
use crate::state::AdapterState;

/// Create the `AdapterState` PDA and the MarginFi account PDA (authority = position).
///
/// Standard prefix (0–4) then MarginFi tail (5–8):
/// 0 instructions_sysvar, 1 position, 2 adapter_state, 3 rent_payer, 4 system_program,
/// 5 marginfi_program, 6 marginfi_group, 7 marginfi_account, 8 bank.
#[derive(Accounts)]
pub struct AdapterInitialize<'info> {
    /// CHECK: instructions sysvar — address enforced; caller verification.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    /// CHECK: dispatcher Position PDA — MarginFi account authority (signs via the
    /// dispatcher's invoke_signed) + adapter_state seed.
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

    // ----- MarginFi-specific -----
    /// CHECK: MarginFi program — CPI target.
    #[account(executable)]
    pub marginfi_program: AccountInfo<'info>,
    /// CHECK: MarginFi group; validated by MarginFi during init.
    pub marginfi_group: AccountInfo<'info>,
    /// CHECK: MarginFi account PDA, created by the MarginFi CPI.
    #[account(mut)]
    pub marginfi_account: AccountInfo<'info>,
    /// CHECK: the bank this position deposits into; recorded for later validation.
    pub bank: AccountInfo<'info>,
}

pub fn handler(ctx: Context<AdapterInitialize>, adapter_data: Vec<u8>) -> Result<()> {
    require!(
        adapter_data.len() <= MAX_ADAPTER_DATA_LEN,
        AdapterError::AdapterDataTooLarge
    );
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    marginfi_cpi::init_account_pda(
        &ctx.accounts.marginfi_program,
        &ctx.accounts.marginfi_group,
        &ctx.accounts.marginfi_account,
        &ctx.accounts.position,
        &ctx.accounts.rent_payer.to_account_info(),
        &ctx.accounts.instructions_sysvar,
        &ctx.accounts.system_program.to_account_info(),
    )?;

    let clock = Clock::get()?;
    let state = &mut ctx.accounts.adapter_state;
    state.header = AdapterStateHeader {
        position: ctx.accounts.position.key(),
        adapter_program: crate::ID,
        created_at: clock.unix_timestamp,
        total_shares: 0,
    };
    state.marginfi_group = ctx.accounts.marginfi_group.key();
    state.marginfi_account = ctx.accounts.marginfi_account.key();
    state.bank = ctx.accounts.bank.key();
    state.bump = ctx.bumps.adapter_state;

    msg!(
        "marginfi-usdc init: position={} mfi_account={} bank={}",
        state.header.position,
        state.marginfi_account,
        state.bank
    );
    Ok(())
}
