use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod routing;
pub mod state;

pub use instructions::*;

declare_id!("CDit4o9LeqFaxEMkS7mHDKkUxrhhr8K9kH4CYfqZxEok");

/// Yield Adapter Standard — dispatcher.
///
/// A thin router plus a governance-gated registry. Users open a `Position` into
/// any whitelisted adapter, then deposit/withdraw/read-value through this program,
/// which CPIs the adapter's standard interface (see `crates/adapter-interface`
/// and `docs/SPEC.md`). The dispatcher knows nothing about any underlying
/// protocol — adapters are added without ever upgrading this program.
#[program]
pub mod dispatcher {
    use super::*;

    /// One-time: create the registry, set the governance admin.
    pub fn initialize_registry(ctx: Context<InitializeRegistry>) -> Result<()> {
        instructions::initialize_registry::handler(ctx)
    }

    /// Admin-only: approve an adapter program (must be executable, matching version).
    pub fn whitelist_adapter(
        ctx: Context<WhitelistAdapter>,
        name: String,
        interface_version: u8,
    ) -> Result<()> {
        instructions::whitelist_adapter::handler(ctx, name, interface_version)
    }

    /// Admin-only: pause (`true`) or re-activate (`false`) an adapter.
    pub fn pause_adapter(ctx: Context<PauseAdapter>, paused: bool) -> Result<()> {
        instructions::pause_adapter::handler(ctx, paused)
    }

    /// Open the caller's position into an adapter (creates the pool + adapter state).
    pub fn open_position<'info>(
        ctx: Context<'_, '_, '_, 'info, OpenPosition<'info>>,
    ) -> Result<()> {
        instructions::open_position::handler(ctx)
    }

    /// Deposit `amount` USDC into the position's adapter.
    pub fn deposit<'info>(
        ctx: Context<'_, '_, '_, 'info, Deposit<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::deposit::handler(ctx, amount)
    }

    /// Redeem `shares` from the position's adapter back to USDC.
    pub fn withdraw<'info>(
        ctx: Context<'_, '_, '_, 'info, Withdraw<'info>>,
        shares: u64,
    ) -> Result<()> {
        instructions::withdraw::handler(ctx, shares)
    }

    /// Read the position's current USDC value (read-only CPI to the adapter).
    pub fn current_value<'info>(
        ctx: Context<'_, '_, '_, 'info, CurrentValue<'info>>,
    ) -> Result<()> {
        instructions::current_value::handler(ctx)
    }
}
