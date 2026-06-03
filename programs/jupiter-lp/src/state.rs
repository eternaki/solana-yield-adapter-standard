use anchor_lang::prelude::*;

use adapter_interface::AdapterStateHeader;

/// Per-position adapter state.
///
/// `seeds = [ADAPTER_STATE_SEED, jupiter_lp_program_id, position]`
///
/// `header.total_shares` tracks JLP held (the adapter's native share unit).
#[account]
#[derive(InitSpace)]
pub struct AdapterState {
    pub header: AdapterStateHeader,
    pub pool: Pubkey,
    pub custody: Pubkey,
    /// Position-owned JLP token account.
    pub lp_token_account: Pubkey,
    pub bump: u8,
}
