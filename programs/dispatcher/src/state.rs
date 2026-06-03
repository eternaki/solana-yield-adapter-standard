use anchor_lang::prelude::*;

use crate::constants::MAX_NAME_LEN;

/// Governance root. `admin` approves/pauses adapters. Swap for a multisig in
/// production — nothing here assumes a single signer beyond the `admin` key.
#[account]
#[derive(InitSpace)]
pub struct Registry {
    pub admin: Pubkey,
    /// Monotonic count of whitelisted adapters (never decreases; pause ≠ remove).
    pub adapter_count: u32,
    pub bump: u8,
}

/// One whitelist entry per adapter program. PDA seeded by the adapter program id,
/// so its existence + `is_active` is the single source of routing truth.
#[account]
#[derive(InitSpace)]
pub struct AdapterEntry {
    pub adapter_program: Pubkey,
    pub interface_version: u8,
    #[max_len(MAX_NAME_LEN)]
    pub name: String,
    /// Paused adapters stay registered but cannot be routed to.
    pub is_active: bool,
    pub added_at: i64,
    pub bump: u8,
}

/// A user's routing position into a single adapter. The PDA is the authority over
/// `usdc_pool` and signs every adapter CPI, so funds never leave the user's
/// control to the dispatcher operator — only to the on-chain adapter logic.
#[account]
#[derive(InitSpace)]
pub struct Position {
    pub owner: Pubkey,
    pub adapter_program: Pubkey,
    /// USDC token account owned by this Position PDA (the adapter's source/sink).
    pub usdc_pool: Pubkey,
    pub usdc_mint: Pubkey,
    pub created_at: i64,
    pub bump: u8,
}
