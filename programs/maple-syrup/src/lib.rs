use anchor_lang::prelude::*;

pub mod events;
pub mod instructions;
pub mod orca_cpi;
pub mod state;

pub use events::*;
pub use instructions::*;
pub use state::*;

declare_id!("3Y1AddXDAnzw14UYpjLByWSJYTncMj7uFNJN9Egwzqh9");

/// Reference yield adapter for **Maple syrupUSDC** (a yield-bearing, price-accruing
/// stablecoin). On Solana syrupUSDC arrives via Chainlink CCIP — there is no native
/// permissionless deposit program — so "depositing into Maple" means acquiring
/// syrupUSDC, which this adapter does by swapping USDC↔syrupUSDC through the deepest
/// Orca Whirlpool. It proves the standard also covers **swap-and-hold** RWA-style
/// sources, not just protocols with their own deposit instruction.
///
/// deposit = swap USDC→syrupUSDC; withdraw = swap back; value = syrupUSDC balance ×
/// the pool's spot price (from `sqrt_price`).
#[program]
pub mod maple_syrup {
    use super::*;

    /// Create the `AdapterState` PDA + the position-owned syrupUSDC token account.
    pub fn adapter_initialize(ctx: Context<AdapterInitialize>, adapter_data: Vec<u8>) -> Result<()> {
        instructions::adapter_initialize::handler(ctx, adapter_data)
    }

    /// Swap `amount` USDC → syrupUSDC; returns syrupUSDC received.
    pub fn adapter_deposit<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterDeposit<'info>>,
        amount: u64,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_deposit::handler(ctx, amount, adapter_data)
    }

    /// Swap `shares` syrupUSDC → USDC; returns USDC received.
    pub fn adapter_withdraw<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterWithdraw<'info>>,
        shares: u64,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_withdraw::handler(ctx, shares, adapter_data)
    }

    /// Return the position's USDC value (syrupUSDC balance × pool spot price).
    pub fn adapter_current_value<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterCurrentValue<'info>>,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_current_value::handler(ctx, adapter_data)
    }
}
