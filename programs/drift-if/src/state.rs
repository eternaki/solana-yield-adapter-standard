use anchor_lang::prelude::*;

use adapter_interface::AdapterStateHeader;

/// Per-position adapter state.
///
/// `seeds = [ADAPTER_STATE_SEED, drift_if_program_id, position]`
///
/// `header.total_shares` tracks the position's Drift IF shares.
#[account]
#[derive(InitSpace)]
pub struct AdapterState {
    pub header: AdapterStateHeader,
    pub spot_market: Pubkey,
    pub insurance_fund_stake: Pubkey,
    pub user_stats: Pubkey,
    pub bump: u8,
}
