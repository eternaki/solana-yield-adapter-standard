use anchor_lang::prelude::*;

pub mod drift_cpi;
pub mod events;
pub mod instructions;
pub mod state;

pub use events::*;
pub use instructions::*;
pub use state::*;

declare_id!("CUa5e7HetWzXfn7czcFSsr8gd6SrfaK2dqzTC11jeGnF");

/// Reference yield adapter for the **Drift Insurance Fund** (USDC, market index 0).
///
/// Implements the Solana Yield Adapter Standard v1 (see `docs/SPEC.md`). Deposit
/// stakes USDC into Drift's insurance fund (earning a share of protocol revenue);
/// withdraw requests + removes the stake; value = the position's IF shares × the
/// fund's USDC per share. Shows the standard covers a staking source with an
/// unstake flow (request → remove) too.
#[program]
pub mod drift_if {
    use super::*;

    /// Create `AdapterState`, the Drift user-stats account, and the IF-stake account.
    pub fn adapter_initialize(ctx: Context<AdapterInitialize>, adapter_data: Vec<u8>) -> Result<()> {
        instructions::adapter_initialize::handler(ctx, adapter_data)
    }

    /// Stake `amount` USDC into the insurance fund; returns IF shares minted.
    pub fn adapter_deposit<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterDeposit<'info>>,
        amount: u64,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_deposit::handler(ctx, amount, adapter_data)
    }

    /// Request-remove + remove `shares` IF shares back to USDC; returns USDC out.
    pub fn adapter_withdraw<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterWithdraw<'info>>,
        shares: u64,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_withdraw::handler(ctx, shares, adapter_data)
    }

    /// Return the position's USDC value (IF shares × fund vault / total IF shares).
    pub fn adapter_current_value<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterCurrentValue<'info>>,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_current_value::handler(ctx, adapter_data)
    }
}
