use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::token::TokenAccount;

use adapter_interface::{
    set_u64_return, verify_caller_is_dispatcher, AdapterError, ADAPTER_STATE_SEED, DISPATCHER_ID,
    MAX_ADAPTER_DATA_LEN,
};

use crate::drift_cpi;
use crate::events::AdapterDepositEvent;
use crate::state::AdapterState;

/// Stake `amount` USDC into the Drift insurance fund. Returns IF shares minted.
///
/// Standard prefix (0–3) then Drift add tail (4–12):
/// 4 drift_program, 5 state, 6 spot_market, 7 insurance_fund_stake, 8 user_stats,
/// 9 spot_market_vault, 10 insurance_fund_vault, 11 drift_signer, 12 token_program.
#[derive(Accounts)]
pub struct AdapterDeposit<'info> {
    /// CHECK: instructions sysvar.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
    /// CHECK: dispatcher Position PDA; signs the Drift CPI (propagated).
    pub position: AccountInfo<'info>,
    /// USDC source (Drift user_token_account).
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
    /// CHECK: spot market vault; validated by Drift.
    #[account(mut)]
    pub spot_market_vault: AccountInfo<'info>,
    /// CHECK: insurance fund vault; validated by Drift.
    #[account(mut)]
    pub insurance_fund_vault: AccountInfo<'info>,
    /// CHECK: Drift signer PDA; validated by Drift.
    pub drift_signer: AccountInfo<'info>,
    /// CHECK: SPL token program.
    pub token_program: AccountInfo<'info>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, AdapterDeposit<'info>>,
    amount: u64,
    adapter_data: Vec<u8>,
) -> Result<()> {
    require!(amount > 0, AdapterError::ZeroAmount);
    require!(adapter_data.len() <= MAX_ADAPTER_DATA_LEN, AdapterError::AdapterDataTooLarge);
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    let before = drift_cpi::stake_shares(&ctx.accounts.insurance_fund_stake.try_borrow_data()?)?;
    drift_cpi::add_stake(
        &ctx.accounts.drift_program, &ctx.accounts.state, &ctx.accounts.spot_market,
        &ctx.accounts.insurance_fund_stake, &ctx.accounts.user_stats, &ctx.accounts.position,
        &ctx.accounts.spot_market_vault, &ctx.accounts.insurance_fund_vault, &ctx.accounts.drift_signer,
        &ctx.accounts.position_usdc_pool.to_account_info(), &ctx.accounts.token_program, amount,
    )?;
    let after = drift_cpi::stake_shares(&ctx.accounts.insurance_fund_stake.try_borrow_data()?)?;
    let shares_minted = u64::try_from(after.checked_sub(before).ok_or(AdapterError::MathOverflow)?)
        .map_err(|_| AdapterError::MathOverflow)?;

    let s = &mut ctx.accounts.adapter_state;
    s.header.total_shares = s.header.total_shares.checked_add(shares_minted).ok_or(AdapterError::MathOverflow)?;

    set_u64_return(shares_minted);
    emit!(AdapterDepositEvent {
        position: s.header.position,
        amount_in: amount,
        shares_minted,
    });
    msg!("drift-if deposit: {} USDC -> {} IF shares", amount, shares_minted);
    Ok(())
}
