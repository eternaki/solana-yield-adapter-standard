use anchor_lang::prelude::*;

use adapter_interface::AdapterStateHeader;

/// Per-position adapter state PDA.
///
/// `seeds = [ADAPTER_STATE_SEED, kamino_usdc_program_id, position]`
///
/// Begins with the standard `AdapterStateHeader` (so any reader can decode
/// position/ownership/shares without knowing this adapter), followed by the
/// Kamino-specific tail.
#[account]
#[derive(InitSpace)]
pub struct AdapterState {
    /// Standard header — MUST stay first (spec §Adapter State Account).
    pub header: AdapterStateHeader,

    // ----- Kamino-specific tail -----
    /// The klend reserve this position deposits into (e.g. main-market USDC).
    pub kamino_reserve: Pubkey,
    /// Position-owned token account holding the reserve collateral mint (cUSDC).
    pub collateral_vault: Pubkey,
    /// PDA bump.
    pub bump: u8,
}
