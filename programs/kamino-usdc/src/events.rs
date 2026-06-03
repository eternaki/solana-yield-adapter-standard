use anchor_lang::prelude::*;

/// Emitted by `adapter_deposit` (spec §adapter_deposit).
#[event]
pub struct AdapterDepositEvent {
    pub position: Pubkey,
    pub amount_in: u64,
    pub shares_minted: u64,
}

/// Emitted by `adapter_withdraw` (spec §adapter_withdraw).
#[event]
pub struct AdapterWithdrawEvent {
    pub position: Pubkey,
    pub shares_burned: u64,
    pub amount_out: u64,
}

/// Emitted by `adapter_current_value` (spec §adapter_current_value).
#[event]
pub struct AdapterValueEvent {
    pub position: Pubkey,
    pub current_value_usdc: u64,
    pub as_of_slot: u64,
}
