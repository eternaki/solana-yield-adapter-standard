use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;

use adapter_interface::{
    verify_caller_is_dispatcher, AdapterError, AdapterStateHeader, ADAPTER_STATE_SEED,
    DISPATCHER_ID, MAX_ADAPTER_DATA_LEN,
};

use crate::drift_cpi;
use crate::state::AdapterState;

/// Create `AdapterState`, the Drift user-stats account, and the IF-stake account
/// (both Drift PDAs, via CPI; authority = position, payer = rent_payer).
///
/// Standard prefix (0–4) then Drift tail (5–10):
/// 5 drift_program, 6 state, 7 spot_market, 8 user_stats, 9 insurance_fund_stake, 10 rent.
#[derive(Accounts)]
pub struct AdapterInitialize<'info> {
    /// CHECK: instructions sysvar.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
    /// CHECK: dispatcher Position PDA — Drift authority (signs via dispatcher) + state seed.
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

    // ----- Drift-specific -----
    /// CHECK: Drift program — CPI target.
    #[account(executable)]
    pub drift_program: AccountInfo<'info>,
    /// CHECK: Drift state; validated by Drift.
    #[account(mut)]
    pub state: AccountInfo<'info>,
    /// CHECK: USDC spot market; recorded.
    pub spot_market: AccountInfo<'info>,
    /// CHECK: Drift user-stats PDA, created by the CPI.
    #[account(mut)]
    pub user_stats: AccountInfo<'info>,
    /// CHECK: Drift IF-stake PDA, created by the CPI.
    #[account(mut)]
    pub insurance_fund_stake: AccountInfo<'info>,
    /// CHECK: rent sysvar.
    #[account(address = sysvar::rent::ID)]
    pub rent: AccountInfo<'info>,
}

pub fn handler(ctx: Context<AdapterInitialize>, adapter_data: Vec<u8>) -> Result<()> {
    require!(adapter_data.len() <= MAX_ADAPTER_DATA_LEN, AdapterError::AdapterDataTooLarge);
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    drift_cpi::init_user_stats(
        &ctx.accounts.drift_program, &ctx.accounts.user_stats, &ctx.accounts.state,
        &ctx.accounts.position, &ctx.accounts.rent_payer.to_account_info(),
        &ctx.accounts.rent, &ctx.accounts.system_program.to_account_info(),
    )?;
    drift_cpi::init_if_stake(
        &ctx.accounts.drift_program, &ctx.accounts.spot_market, &ctx.accounts.insurance_fund_stake,
        &ctx.accounts.user_stats, &ctx.accounts.state, &ctx.accounts.position,
        &ctx.accounts.rent_payer.to_account_info(), &ctx.accounts.rent,
        &ctx.accounts.system_program.to_account_info(),
    )?;

    let clock = Clock::get()?;
    let s = &mut ctx.accounts.adapter_state;
    s.header = AdapterStateHeader {
        position: ctx.accounts.position.key(),
        adapter_program: crate::ID,
        created_at: clock.unix_timestamp,
        total_shares: 0,
    };
    s.spot_market = ctx.accounts.spot_market.key();
    s.insurance_fund_stake = ctx.accounts.insurance_fund_stake.key();
    s.user_stats = ctx.accounts.user_stats.key();
    s.bump = ctx.bumps.adapter_state;
    msg!("drift-if init: position={} if_stake={}", s.header.position, s.insurance_fund_stake);
    Ok(())
}
