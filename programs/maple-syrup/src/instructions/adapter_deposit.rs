use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::token::TokenAccount;

use adapter_interface::{
    set_u64_return, verify_caller_is_dispatcher, AdapterError, ADAPTER_STATE_SEED, DISPATCHER_ID,
    MAX_ADAPTER_DATA_LEN,
};

use crate::events::AdapterDepositEvent;
use crate::orca_cpi::{self, SwapAccounts};
use crate::state::AdapterState;

/// Swap `amount` USDC → syrupUSDC via the whirlpool. Returns syrupUSDC received.
///
/// Standard prefix (0–3) then Orca swap tail (4–13):
/// 4 orca_program, 5 whirlpool, 6 syrup_token_account (vault-side A owner),
/// 7 vault_a, 8 vault_b, 9 tick_array_0, 10 tick_array_1, 11 tick_array_2,
/// 12 oracle, 13 token_program.
#[derive(Accounts)]
pub struct AdapterDeposit<'info> {
    /// CHECK: instructions sysvar.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
    /// CHECK: dispatcher Position PDA; signs the swap (propagated).
    pub position: AccountInfo<'info>,
    /// USDC pool (Orca token_owner_account_b).
    #[account(mut)]
    pub position_usdc_pool: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [ADAPTER_STATE_SEED, crate::ID.as_ref(), position.key().as_ref()],
        bump = adapter_state.bump,
        constraint = adapter_state.header.position == position.key() @ AdapterError::Unauthorized,
    )]
    pub adapter_state: Account<'info, AdapterState>,

    /// CHECK: Orca whirlpools program — CPI target.
    #[account(executable)]
    pub orca_program: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.whirlpool.
    #[account(mut, address = adapter_state.whirlpool @ AdapterError::Unauthorized)]
    pub whirlpool: AccountInfo<'info>,
    /// syrupUSDC account (Orca token_owner_account_a).
    #[account(mut, address = adapter_state.syrup_token_account @ AdapterError::Unauthorized)]
    pub syrup_token_account: Account<'info, TokenAccount>,
    /// CHECK: whirlpool vault A; validated by Orca.
    #[account(mut)]
    pub vault_a: AccountInfo<'info>,
    /// CHECK: whirlpool vault B; validated by Orca.
    #[account(mut)]
    pub vault_b: AccountInfo<'info>,
    /// CHECK: tick array; validated by Orca.
    #[account(mut)]
    pub tick_array_0: AccountInfo<'info>,
    /// CHECK: tick array; validated by Orca.
    #[account(mut)]
    pub tick_array_1: AccountInfo<'info>,
    /// CHECK: tick array; validated by Orca.
    #[account(mut)]
    pub tick_array_2: AccountInfo<'info>,
    /// CHECK: whirlpool oracle; validated by Orca.
    #[account(mut)]
    pub oracle: AccountInfo<'info>,
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

    let accs = SwapAccounts {
        program: ctx.accounts.orca_program.clone(),
        token_program: ctx.accounts.token_program.clone(),
        authority: ctx.accounts.position.clone(),
        whirlpool: ctx.accounts.whirlpool.clone(),
        token_owner_a: ctx.accounts.syrup_token_account.to_account_info(),
        vault_a: ctx.accounts.vault_a.clone(),
        token_owner_b: ctx.accounts.position_usdc_pool.to_account_info(),
        vault_b: ctx.accounts.vault_b.clone(),
        tick_array_0: ctx.accounts.tick_array_0.clone(),
        tick_array_1: ctx.accounts.tick_array_1.clone(),
        tick_array_2: ctx.accounts.tick_array_2.clone(),
        oracle: ctx.accounts.oracle.clone(),
    };

    let before = ctx.accounts.syrup_token_account.amount;
    orca_cpi::swap(&accs, amount, false)?; // USDC (B) -> syrupUSDC (A)
    ctx.accounts.syrup_token_account.reload()?;
    let shares_minted = ctx
        .accounts
        .syrup_token_account
        .amount
        .checked_sub(before)
        .ok_or(AdapterError::MathOverflow)?;

    let state = &mut ctx.accounts.adapter_state;
    state.header.total_shares = state
        .header
        .total_shares
        .checked_add(shares_minted)
        .ok_or(AdapterError::MathOverflow)?;

    set_u64_return(shares_minted);
    emit!(AdapterDepositEvent {
        position: state.header.position,
        amount_in: amount,
        shares_minted,
    });
    msg!("maple deposit: {} USDC -> {} syrupUSDC", amount, shares_minted);
    Ok(())
}
