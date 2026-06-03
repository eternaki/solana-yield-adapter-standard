use anchor_lang::prelude::*;

pub mod events;
pub mod instructions;
pub mod klend_cpi;
pub mod state;

pub use events::*;
pub use instructions::*;
pub use state::*;

declare_id!("BSwbiwogeLmYLYXfYb3t4WWNDD3x5tJHCZyZJLVRLuf9");

/// Reference yield adapter for **Kamino Lend (klend) USDC reserve**.
///
/// Implements the Solana Yield Adapter Standard v1 (see `docs/SPEC.md`). The
/// dispatcher CPIs all four instructions; every one verifies the caller is the
/// dispatcher via the instructions sysvar (`adapter_interface::verify_caller_is_dispatcher`).
/// Use this program as the copy-paste template for new adapters
/// (`docs/BUILD-YOUR-OWN-ADAPTER.md`).
#[program]
pub mod kamino_usdc {
    use super::*;

    /// Create the per-position `AdapterState` PDA + the position-owned collateral
    /// (cToken) account. CPI'd by the dispatcher's `open_position`.
    pub fn adapter_initialize(ctx: Context<AdapterInitialize>, adapter_data: Vec<u8>) -> Result<()> {
        instructions::adapter_initialize::handler(ctx, adapter_data)
    }

    /// Deposit `amount` USDC into the Kamino reserve; returns `shares_minted`.
    pub fn adapter_deposit<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterDeposit<'info>>,
        amount: u64,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_deposit::handler(ctx, amount, adapter_data)
    }

    /// Redeem `shares` cTokens back into USDC; returns `amount_out`.
    pub fn adapter_withdraw<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterWithdraw<'info>>,
        shares: u64,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_withdraw::handler(ctx, shares, adapter_data)
    }

    /// Return the position's current USDC value via return data + event.
    pub fn adapter_current_value<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterCurrentValue<'info>>,
        adapter_data: Vec<u8>,
    ) -> Result<()> {
        instructions::adapter_current_value::handler(ctx, adapter_data)
    }
}
