use anchor_lang::prelude::*;

pub mod events;
pub mod instructions;
pub mod marginfi_cpi;
pub mod state;

pub use events::*;
pub use instructions::*;
pub use state::*;

declare_id!("5C6BQ9tJdyUjP8Dz1MxHXtYF8EuXX8nXZPLwZvwc6d93");

/// Reference yield adapter for a **MarginFi v2 USDC bank**.
///
/// Implements the Solana Yield Adapter Standard v1 (see `docs/SPEC.md`). The
/// position's funds live in a MarginFi account PDA owned (as authority) by the
/// dispatcher `Position`; deposit/withdraw CPI MarginFi's lending instructions,
/// and value is read from the account's asset shares × the bank's share value.
#[program]
pub mod marginfi_usdc {
    use super::*;

    /// Create the `AdapterState` PDA and the MarginFi account PDA (authority = position).
    pub fn adapter_initialize(ctx: Context<AdapterInitialize>, adapter_data: Vec<u8>) -> Result<()> {
        instructions::adapter_initialize::handler(ctx, adapter_data)
    }

    /// Deposit `amount` USDC into the MarginFi bank; returns the amount as shares.
    pub fn adapter_deposit<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterDeposit<'info>>,
        amount: u64,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_deposit::handler(ctx, amount, adapter_data)
    }

    /// Withdraw `shares` USDC base units (or all, if `shares >= booked`); returns USDC out.
    pub fn adapter_withdraw<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterWithdraw<'info>>,
        shares: u64,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_withdraw::handler(ctx, shares, adapter_data)
    }

    /// Return the position's USDC value (asset_shares × bank.asset_share_value).
    pub fn adapter_current_value<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterCurrentValue<'info>>,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_current_value::handler(ctx, adapter_data)
    }
}
