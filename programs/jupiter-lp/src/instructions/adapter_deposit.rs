use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::token::TokenAccount;

use adapter_interface::{
    set_u64_return, verify_caller_is_dispatcher, AdapterError, ADAPTER_STATE_SEED, DISPATCHER_ID,
    MAX_ADAPTER_DATA_LEN,
};

use crate::events::AdapterDepositEvent;
use crate::jup_cpi::{self, LiquidityAccounts};
use crate::state::AdapterState;

/// Add `amount` USDC of liquidity to the JLP pool, receiving JLP. Returns JLP minted.
///
/// Standard prefix (0–3) then Jupiter add_liquidity2 tail (4–15):
/// 4 jupiter_program, 5 transfer_authority, 6 perpetuals, 7 pool, 8 custody,
/// 9 doves_price, 10 pythnet_price, 11 custody_token_account, 12 lp_token_mint,
/// 13 lp_token_account, 14 token_program, 15 event_authority.
#[derive(Accounts)]
pub struct AdapterDeposit<'info> {
    /// CHECK: instructions sysvar.
    #[account(address = sysvar::instructions::ID)]
    pub instructions_sysvar: AccountInfo<'info>,
    /// CHECK: dispatcher Position PDA; signs the Jupiter CPI (propagated).
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

    // ----- Jupiter add_liquidity2 tail -----
    /// CHECK: Jupiter Perps program — CPI target.
    #[account(executable)]
    pub jupiter_program: AccountInfo<'info>,
    /// CHECK: Perps transfer authority PDA; validated by Jupiter.
    pub transfer_authority: AccountInfo<'info>,
    /// CHECK: Perpetuals account; validated by Jupiter.
    pub perpetuals: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.pool.
    #[account(mut, address = adapter_state.pool @ AdapterError::Unauthorized)]
    pub pool: AccountInfo<'info>,
    /// CHECK: validated == adapter_state.custody.
    #[account(mut, address = adapter_state.custody @ AdapterError::Unauthorized)]
    pub custody: AccountInfo<'info>,
    /// CHECK: custody doves price account.
    pub doves_price: AccountInfo<'info>,
    /// CHECK: custody pythnet price account.
    pub pythnet_price: AccountInfo<'info>,
    /// CHECK: custody token vault.
    #[account(mut)]
    pub custody_token_account: AccountInfo<'info>,
    /// CHECK: JLP mint.
    #[account(mut)]
    pub lp_token_mint: AccountInfo<'info>,
    #[account(mut, address = adapter_state.lp_token_account @ AdapterError::Unauthorized)]
    pub lp_token_account: Account<'info, TokenAccount>,
    /// CHECK: SPL token program.
    pub token_program: AccountInfo<'info>,
    /// CHECK: Jupiter event authority PDA.
    pub event_authority: AccountInfo<'info>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, AdapterDeposit<'info>>,
    amount: u64,
    adapter_data: Vec<u8>,
) -> Result<()> {
    require!(amount > 0, AdapterError::ZeroAmount);
    require!(adapter_data.len() <= MAX_ADAPTER_DATA_LEN, AdapterError::AdapterDataTooLarge);
    verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;

    let accs = LiquidityAccounts {
        program: ctx.accounts.jupiter_program.clone(),
        owner: ctx.accounts.position.clone(),
        token_account: ctx.accounts.position_usdc_pool.to_account_info(),
        lp_token_account: ctx.accounts.lp_token_account.to_account_info(),
        transfer_authority: ctx.accounts.transfer_authority.clone(),
        perpetuals: ctx.accounts.perpetuals.clone(),
        pool: ctx.accounts.pool.clone(),
        custody: ctx.accounts.custody.clone(),
        doves_price: ctx.accounts.doves_price.clone(),
        pythnet_price: ctx.accounts.pythnet_price.clone(),
        custody_token_account: ctx.accounts.custody_token_account.clone(),
        lp_token_mint: ctx.accounts.lp_token_mint.clone(),
        token_program: ctx.accounts.token_program.clone(),
        event_authority: ctx.accounts.event_authority.clone(),
    };

    let lp_before = ctx.accounts.lp_token_account.amount;
    jup_cpi::add_liquidity(&accs, amount, ctx.remaining_accounts)?;
    ctx.accounts.lp_token_account.reload()?;
    let shares_minted = ctx
        .accounts
        .lp_token_account
        .amount
        .checked_sub(lp_before)
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
    msg!("jupiter-lp deposit: {} USDC -> {} JLP", amount, shares_minted);
    Ok(())
}
