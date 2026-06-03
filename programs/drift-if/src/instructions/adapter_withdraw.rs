use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::token::TokenAccount;

use adapter_interface::{
    set_u64_return, verify_caller_is_dispatcher, AdapterError, ADAPTER_STATE_SEED, DISPATCHER_ID,
    MAX_ADAPTER_DATA_LEN,
};

use crate::drift_cpi;
use crate::events::AdapterWithdrawEvent;
use crate::state::AdapterState;

/// Request-remove then remove `shares` IF shares back to USDC (one tx). The Drift
/// unstake cooldown is enforced on-chain; the mainnet-fork patches the spot
/// market's `unstaking_period` to 0 so request+remove can settle in one call.
///
/// MAINNET LIMITATION: Drift's USDC Insurance Fund has a non-zero `unstaking_period`
/// (currently ~13 days). On a live deployment this single-tx flow reverts in
/// `remove_insurance_fund_stake` because the cooldown has not elapsed. A production
/// Drift adapter must split withdraw into two instructions — `request_remove` (stores
/// the request timestamp) and, after the cooldown, `remove` (settles USDC) — which is
/// an adapter-level extension of the standard's single `adapter_withdraw` for
/// time-locked sources. The one-tx form here is correct only where the cooldown is 0.
///
/// Standard prefix (0–3) then Drift tail (4–11):
/// 4 drift_program, 5 state, 6 spot_market, 7 insurance_fund_stake, 8 user_stats,
/// 9 insurance_fund_vault, 10 drift_signer, 11 token_program.
#[derive(Accounts)]
pub struct AdapterWithdraw<'info> {
    /// CHECK: instructions sysvar.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
    /// CHECK: dispatcher Position PDA; signs the Drift CPI (propagated).
    pub position: AccountInfo<'info>,
    #[account(mut)]
    pub position_usdc_pool: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [ADAPTER_STATE_SEED, crate::ID.as_ref(), position.key().as_ref()],
        bump = adapter_state.bump,
        constraint = adapter_state.header.position == position.key() @ AdapterError::Unauthorized,
    )]
    pub adapter_state: Account<'info, AdapterState>,

    /// CHECK: Drift program — CPI target.
    #[account(executable)]
    pub drift_program: AccountInfo<'info>,
    /// CHECK: Drift state.
    pub state: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.spot_market.
    #[account(mut, address = adapter_state.spot_market @ AdapterError::Unauthorized)]
    pub spot_market: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.insurance_fund_stake.
    #[account(mut, address = adapter_state.insurance_fund_stake @ AdapterError::Unauthorized)]
    pub insurance_fund_stake: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.user_stats.
    #[account(mut, address = adapter_state.user_stats @ AdapterError::Unauthorized)]
    pub user_stats: AccountInfo<'info>,
    /// CHECK: insurance fund vault; validated by Drift.
    #[account(mut)]
    pub insurance_fund_vault: AccountInfo<'info>,
    /// CHECK: Drift signer PDA; validated by Drift.
    pub drift_signer: AccountInfo<'info>,
    /// CHECK: SPL token program.
    pub token_program: AccountInfo<'info>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, AdapterWithdraw<'info>>,
    shares: u64,
    adapter_data: Vec<u8>,
) -> Result<()> {
    require!(shares > 0, AdapterError::ZeroAmount);
    require!(adapter_data.len() <= MAX_ADAPTER_DATA_LEN, AdapterError::AdapterDataTooLarge);
    require!(shares <= ctx.accounts.adapter_state.header.total_shares, AdapterError::InsufficientShares);
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    let before = ctx.accounts.position_usdc_pool.amount;
    drift_cpi::request_remove(
        &ctx.accounts.drift_program, &ctx.accounts.spot_market, &ctx.accounts.insurance_fund_stake,
        &ctx.accounts.user_stats, &ctx.accounts.position, &ctx.accounts.insurance_fund_vault, shares,
    )?;
    drift_cpi::remove_stake(
        &ctx.accounts.drift_program, &ctx.accounts.state, &ctx.accounts.spot_market,
        &ctx.accounts.insurance_fund_stake, &ctx.accounts.user_stats, &ctx.accounts.position,
        &ctx.accounts.insurance_fund_vault, &ctx.accounts.drift_signer,
        &ctx.accounts.position_usdc_pool.to_account_info(), &ctx.accounts.token_program,
    )?;
    ctx.accounts.position_usdc_pool.reload()?;
    let amount_out = ctx.accounts.position_usdc_pool.amount.checked_sub(before).ok_or(AdapterError::MathOverflow)?;

    let s = &mut ctx.accounts.adapter_state;
    s.header.total_shares = s.header.total_shares.checked_sub(shares).ok_or(AdapterError::MathOverflow)?;

    set_u64_return(amount_out);
    emit!(AdapterWithdrawEvent {
        position: s.header.position,
        shares_burned: shares,
        amount_out,
    });
    msg!("drift-if withdraw: {} IF shares -> {} USDC", shares, amount_out);
    Ok(())
}
