use anchor_lang::prelude::*;

use adapter_interface::AdapterStateHeader;

/// Per-position adapter state.
///
/// `seeds = [ADAPTER_STATE_SEED, marginfi_usdc_program_id, position]`
///
/// `header.total_shares` tracks the booked USDC principal (MarginFi shares are
/// I80F48 and don't fit a u64); the live value is read from the MarginFi account.
#[account]
#[derive(InitSpace)]
pub struct AdapterState {
    pub header: AdapterStateHeader,
    pub marginfi_group: Pubkey,
    pub marginfi_account: Pubkey,
    pub bank: Pubkey,
    pub bump: u8,
}
