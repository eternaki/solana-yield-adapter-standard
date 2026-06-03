//! Solana Yield Adapter Standard — shared interface crate.
//!
//! One source of truth for the contract between the **dispatcher** and any
//! **adapter** program: the interface version, the canonical adapter-state
//! header, the four instruction discriminators with their Borsh arg encoders,
//! the dispatcher-caller verification helper, and the return-data codec.
//!
//! Both the dispatcher (to build CPIs and read results) and every adapter (to
//! verify the caller and lay out its state) depend on this crate, so the two
//! sides can never drift. See `docs/SPEC.md`.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::{
    load_current_index_checked, load_instruction_at_checked,
};

// ============================================================================
// Versioning & limits
// ============================================================================

/// Interface version. The registry stores this per adapter; the dispatcher
/// refuses to route to an adapter whose version differs. Bump on any breaking
/// change to instruction args, account order, or the state header.
pub const ADAPTER_INTERFACE_VERSION: u8 = 1;

/// PDA seed for an adapter's per-position state account:
/// `seeds = [ADAPTER_STATE_SEED, adapter_program_id, position]`.
pub const ADAPTER_STATE_SEED: &[u8] = b"adapter_state";

/// Upper bound on the opaque `adapter_data` arg carried by every instruction.
pub const MAX_ADAPTER_DATA_LEN: usize = 256;

/// A value derived from oracle/reserve state older than this many slots is
/// considered stale (see `adapter_current_value` in the spec).
pub const MAX_VALUE_STALENESS_SLOTS: u64 = 60;

/// The dispatcher program ID. Every adapter verifies the CPI caller against this.
/// Synced from `target/deploy/dispatcher-keypair.json` via `anchor keys sync`.
pub const DISPATCHER_ID: Pubkey = pubkey!("CDit4o9LeqFaxEMkS7mHDKkUxrhhr8K9kH4CYfqZxEok");

// ============================================================================
// Canonical adapter-state header
// ============================================================================

/// The fixed prefix every adapter's state account MUST begin with, so any
/// off-chain reader can decode position, ownership, and share count without
/// knowing the adapter-specific tail. Embed as the FIRST field of the adapter's
/// `#[account]` state struct: `pub struct AdapterState { pub header: AdapterStateHeader, .. }`.
#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Default, Debug)]
pub struct AdapterStateHeader {
    /// The dispatcher `Position` PDA this state backs (the CPI signer).
    pub position: Pubkey,
    /// This adapter's own program ID.
    pub adapter_program: Pubkey,
    /// Unix timestamp of initialization.
    pub created_at: i64,
    /// Adapter-internal shares attributable to this position (NOT the source's
    /// global supply). Units are adapter-defined (cTokens, LP, stake shares…).
    pub total_shares: u64,
}

// ============================================================================
// Instruction discriminators — Anchor scheme: sha256("global:<name>")[..8]
// ============================================================================

/// `adapter_initialize` discriminator.
pub const ADAPTER_INITIALIZE: [u8; 8] = [125, 160, 35, 249, 117, 179, 167, 76];
/// `adapter_deposit` discriminator.
pub const ADAPTER_DEPOSIT: [u8; 8] = [190, 207, 72, 186, 232, 106, 46, 72];
/// `adapter_withdraw` discriminator.
pub const ADAPTER_WITHDRAW: [u8; 8] = [121, 55, 72, 46, 185, 100, 173, 236];
/// `adapter_current_value` discriminator.
pub const ADAPTER_CURRENT_VALUE: [u8; 8] = [67, 200, 59, 238, 163, 138, 170, 179];

fn encode_vec(disc: [u8; 8], data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 + 4 + data.len());
    buf.extend_from_slice(&disc);
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
    buf.extend_from_slice(data);
    buf
}

fn encode_u64_vec(disc: [u8; 8], n: u64, data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 + 8 + 4 + data.len());
    buf.extend_from_slice(&disc);
    buf.extend_from_slice(&n.to_le_bytes());
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
    buf.extend_from_slice(data);
    buf
}

/// `adapter_initialize(adapter_data: Vec<u8>)`.
pub fn encode_initialize(adapter_data: &[u8]) -> Vec<u8> {
    encode_vec(ADAPTER_INITIALIZE, adapter_data)
}
/// `adapter_deposit(amount: u64, adapter_data: Vec<u8>)`.
pub fn encode_deposit(amount: u64, adapter_data: &[u8]) -> Vec<u8> {
    encode_u64_vec(ADAPTER_DEPOSIT, amount, adapter_data)
}
/// `adapter_withdraw(shares: u64, adapter_data: Vec<u8>)`.
pub fn encode_withdraw(shares: u64, adapter_data: &[u8]) -> Vec<u8> {
    encode_u64_vec(ADAPTER_WITHDRAW, shares, adapter_data)
}
/// `adapter_current_value(adapter_data: Vec<u8>)`.
pub fn encode_current_value(adapter_data: &[u8]) -> Vec<u8> {
    encode_vec(ADAPTER_CURRENT_VALUE, adapter_data)
}

// ============================================================================
// Return-data codec — the canonical result channel (spec §Return values)
// ============================================================================

/// Adapters write their u64 result (shares / amount_out / value_usdc) here; the
/// dispatcher reads it synchronously after the CPI returns.
pub fn set_u64_return(value: u64) {
    anchor_lang::solana_program::program::set_return_data(&value.to_le_bytes());
}

/// Read a u64 the just-CPI'd `expected_program` wrote via `set_u64_return`.
/// Returns `None` if no return data, the wrong program set it, or it isn't 8 bytes.
pub fn get_u64_return(expected_program: &Pubkey) -> Option<u64> {
    let (program, data) = anchor_lang::solana_program::program::get_return_data()?;
    if &program != expected_program || data.len() != 8 {
        return None;
    }
    let bytes: [u8; 8] = data.try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

// ============================================================================
// Caller verification (spec §Security)
// ============================================================================

/// Verify the program that invoked this adapter is `dispatcher`. Reads the
/// currently-executing instruction from the instructions sysvar (not a fixed
/// index — ComputeBudget or other preamble instructions may precede the call).
pub fn verify_caller_is_dispatcher(
    instructions_sysvar: &AccountInfo,
    dispatcher: &Pubkey,
) -> Result<()> {
    let idx = load_current_index_checked(instructions_sysvar)?;
    let parent = load_instruction_at_checked(idx as usize, instructions_sysvar)?;
    require_keys_eq!(parent.program_id, *dispatcher, AdapterError::Unauthorized);
    Ok(())
}

/// Canonical adapter error codes — shared by every adapter so a given code means
/// the same thing across the whole standard (spec §Canonical Error Codes).
///
/// Variant order is LOAD-BEARING: Anchor assigns codes from 6000 in declaration
/// order. Do NOT reorder or insert before existing variants.
#[error_code]
pub enum AdapterError {
    /// 6000 — CPI caller is not the registered dispatcher.
    #[msg("Unauthorized: caller is not the registered dispatcher")]
    Unauthorized,
    /// 6001 — `amount == 0` or `shares == 0`.
    #[msg("Amount or shares must be greater than zero")]
    ZeroAmount,
    /// 6002 — withdraw exceeds the adapter's tracked share balance.
    #[msg("Withdraw exceeds adapter share balance")]
    InsufficientShares,
    /// 6003 — checked arithmetic returned `None`.
    #[msg("Arithmetic overflow")]
    MathOverflow,
    /// 6004 — `adapter_data` version byte does not match ADAPTER_INTERFACE_VERSION.
    #[msg("adapter_data version mismatch")]
    VersionMismatch,
    /// 6005 — oracle / reserve state older than MAX_VALUE_STALENESS_SLOTS.
    #[msg("Oracle data is stale")]
    StaleOracle,
    /// 6006 — `adapter_state` not yet initialized via `adapter_initialize`.
    #[msg("adapter_state is not initialized")]
    AdapterStateUninit,
    /// 6007 — `adapter_data` exceeds MAX_ADAPTER_DATA_LEN bytes.
    #[msg("adapter_data exceeds the maximum length")]
    AdapterDataTooLarge,
    /// 6008 — an underlying protocol CPI failed (see program logs).
    #[msg("Underlying protocol CPI failed")]
    CpiFailed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::solana_program::hash::hash;

    fn disc(name: &str) -> [u8; 8] {
        let mut out = [0u8; 8];
        out.copy_from_slice(&hash(format!("global:{name}").as_bytes()).to_bytes()[..8]);
        out
    }

    #[test]
    fn discriminators_match_anchor_scheme() {
        assert_eq!(ADAPTER_INITIALIZE, disc("adapter_initialize"));
        assert_eq!(ADAPTER_DEPOSIT, disc("adapter_deposit"));
        assert_eq!(ADAPTER_WITHDRAW, disc("adapter_withdraw"));
        assert_eq!(ADAPTER_CURRENT_VALUE, disc("adapter_current_value"));
    }
}
