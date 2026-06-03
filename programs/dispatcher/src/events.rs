use anchor_lang::prelude::*;

#[event]
pub struct AdapterWhitelisted {
    pub adapter_program: Pubkey,
    pub name: String,
    pub interface_version: u8,
}

#[event]
pub struct AdapterPaused {
    pub adapter_program: Pubkey,
    pub is_active: bool,
}

#[event]
pub struct PositionOpened {
    pub owner: Pubkey,
    pub adapter_program: Pubkey,
    pub position: Pubkey,
}

#[event]
pub struct Deposited {
    pub position: Pubkey,
    pub amount_in: u64,
    pub shares_minted: u64,
}

#[event]
pub struct Withdrawn {
    pub position: Pubkey,
    pub shares_burned: u64,
    pub amount_out: u64,
}

#[event]
pub struct ValueRead {
    pub position: Pubkey,
    pub value_usdc: u64,
}
