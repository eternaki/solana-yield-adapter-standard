use anchor_lang::prelude::*;

pub mod events;
pub mod instructions;
pub mod jup_cpi;
pub mod state;

pub use events::*;
pub use instructions::*;
pub use state::*;

declare_id!("DwqPJD68SpERitR6NNp9AaizEn5W7ERbDArCkPJqwmzq");

/// Reference yield adapter for the **Jupiter Perpetuals JLP pool**.
///
/// Implements the Solana Yield Adapter Standard v1 (see `docs/SPEC.md`). Deposit
/// adds USDC liquidity (`add_liquidity2`) and receives JLP into a position-owned
/// account; withdraw burns JLP (`remove_liquidity2`) back to USDC; value =
/// JLP balance × pool AUM / JLP supply. Shows the standard generalizes beyond
/// lending to an LP/AMM-style source.
#[program]
pub mod jupiter_lp {
    use super::*;

    /// Create the `AdapterState` PDA + the position-owned JLP token account.
    pub fn adapter_initialize(ctx: Context<AdapterInitialize>, adapter_data: Vec<u8>) -> Result<()> {
        instructions::adapter_initialize::handler(ctx, adapter_data)
    }

    /// Add `amount` USDC of liquidity to the JLP pool; returns JLP minted.
    pub fn adapter_deposit<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterDeposit<'info>>,
        amount: u64,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_deposit::handler(ctx, amount, adapter_data)
    }

    /// Remove `shares` JLP of liquidity; returns USDC out.
    pub fn adapter_withdraw<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterWithdraw<'info>>,
        shares: u64,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_withdraw::handler(ctx, shares, adapter_data)
    }

    /// Return the position's USDC value (JLP balance × pool.aumUsd / JLP supply).
    pub fn adapter_current_value<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterCurrentValue<'info>>,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_current_value::handler(ctx, adapter_data)
    }
}
