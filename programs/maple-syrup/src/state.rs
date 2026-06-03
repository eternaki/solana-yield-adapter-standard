use anchor_lang::prelude::*;

use adapter_interface::AdapterStateHeader;

/// Per-position adapter state.
///
/// `seeds = [ADAPTER_STATE_SEED, maple_syrup_program_id, position]`
///
/// `header.total_shares` tracks syrupUSDC held (the adapter's native share unit).
#[account]
#[derive(InitSpace)]
pub struct AdapterState {
    pub header: AdapterStateHeader,
    /// The Orca whirlpool used to swap USDC↔syrupUSDC.
    pub whirlpool: Pubkey,
    /// Position-owned syrupUSDC token account.
    pub syrup_token_account: Pubkey,
    pub bump: u8,
}
