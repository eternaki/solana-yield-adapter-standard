use anchor_lang::prelude::*;

#[event]
pub struct AdapterDepositEvent {
    pub position: Pubkey,
    pub amount_in: u64,
    pub shares_minted: u64,
}

#[event]
pub struct AdapterWithdrawEvent {
    pub position: Pubkey,
    pub shares_burned: u64,
    pub amount_out: u64,
}

#[event]
pub struct AdapterValueEvent {
    pub position: Pubkey,
    pub current_value_usdc: u64,
    pub as_of_slot: u64,
}
