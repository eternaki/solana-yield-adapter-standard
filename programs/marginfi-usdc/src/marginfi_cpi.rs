use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
};

use adapter_interface::AdapterError;

// MarginFi v2 instruction discriminators (from the on-chain IDL).
pub const MFI_INIT_PDA: [u8; 8] = [87, 177, 91, 80, 218, 119, 245, 31];
pub const MFI_DEPOSIT: [u8; 8] = [171, 94, 235, 103, 82, 64, 212, 140];
pub const MFI_WITHDRAW: [u8; 8] = [36, 72, 74, 19, 210, 210, 192, 192];

// MarginfiAccount layout: 8 disc + group(32) + authority(32) = 72, then balances[16].
const BALANCES_OFF: usize = 72;
const BALANCE_SIZE: usize = 104;
const BAL_ACTIVE: usize = 0;
const BAL_BANK_PK: usize = 1;
const BAL_ASSET_SHARES: usize = 40; // WrappedI80F48 (i128 LE)
// Bank layout: asset_share_value (WrappedI80F48) at byte 80.
const BANK_ASSET_SHARE_VALUE: usize = 80;

fn read_i128(data: &[u8], off: usize) -> Result<i128> {
    let b: [u8; 16] = data
        .get(off..off + 16)
        .ok_or(AdapterError::MathOverflow)?
        .try_into()
        .map_err(|_| AdapterError::MathOverflow)?;
    Ok(i128::from_le_bytes(b))
}

/// `asset_shares` (I80F48) this MarginFi account holds in `bank`, or 0 if no active balance.
pub fn account_asset_shares(acct_data: &[u8], bank: &Pubkey) -> Result<i128> {
    for i in 0..16 {
        let base = BALANCES_OFF + i * BALANCE_SIZE;
        let active = *acct_data.get(base + BAL_ACTIVE).ok_or(AdapterError::MathOverflow)?;
        if active == 0 {
            continue;
        }
        let pk = Pubkey::try_from(
            acct_data
                .get(base + BAL_BANK_PK..base + BAL_BANK_PK + 32)
                .ok_or(AdapterError::MathOverflow)?,
        )
        .map_err(|_| AdapterError::MathOverflow)?;
        if &pk == bank {
            return read_i128(acct_data, base + BAL_ASSET_SHARES);
        }
    }
    Ok(0)
}

/// Bank `asset_share_value` (I80F48): USDC base units per share.
pub fn bank_asset_share_value(bank_data: &[u8]) -> Result<i128> {
    read_i128(bank_data, BANK_ASSET_SHARE_VALUE)
}

/// value (USDC base units) = asset_shares · asset_share_value, both I80F48 → `>> 96`.
/// Each operand is pre-shifted by 16 bits so the `u128` product never overflows
/// (precision loss is far below one base unit).
pub fn shares_to_usdc(asset_shares: i128, share_value: i128) -> Result<u64> {
    if asset_shares <= 0 || share_value <= 0 {
        return Ok(0);
    }
    let s = (asset_shares as u128) >> 16;
    let v = (share_value as u128) >> 16;
    let prod = s.checked_mul(v).ok_or(AdapterError::MathOverflow)?;
    Ok((prod >> 64) as u64)
}

/// CPI `marginfi_account_initialize_pda` (account_index = 0, third_party_id = None).
/// All signers (`authority` = position, `fee_payer`) propagate from the parent call.
pub fn init_account_pda<'info>(
    program: &AccountInfo<'info>,
    group: &AccountInfo<'info>,
    marginfi_account: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    fee_payer: &AccountInfo<'info>,
    instructions_sysvar: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
) -> Result<()> {
    let metas = vec![
        AccountMeta::new_readonly(group.key(), false),
        AccountMeta::new(marginfi_account.key(), false),
        AccountMeta::new_readonly(authority.key(), true),
        AccountMeta::new(fee_payer.key(), true),
        AccountMeta::new_readonly(instructions_sysvar.key(), false),
        AccountMeta::new_readonly(system_program.key(), false),
    ];
    let mut data = MFI_INIT_PDA.to_vec();
    data.extend_from_slice(&0u16.to_le_bytes()); // account_index
    data.push(0); // third_party_id: Option<u16> = None
    let infos = [
        program.clone(),
        group.clone(),
        marginfi_account.clone(),
        authority.clone(),
        fee_payer.clone(),
        instructions_sysvar.clone(),
        system_program.clone(),
    ];
    invoke(&Instruction { program_id: program.key(), accounts: metas, data }, &infos)
        .map_err(|e| {
            msg!("marginfi init_account_pda failed: {:?}", e);
            error!(AdapterError::CpiFailed)
        })
}

/// CPI `lending_account_deposit(amount, deposit_up_to_limit = None)`.
#[allow(clippy::too_many_arguments)]
pub fn deposit<'info>(
    program: &AccountInfo<'info>,
    group: &AccountInfo<'info>,
    marginfi_account: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    bank: &AccountInfo<'info>,
    signer_token_account: &AccountInfo<'info>,
    liquidity_vault: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    let metas = vec![
        AccountMeta::new_readonly(group.key(), false),
        AccountMeta::new(marginfi_account.key(), false),
        AccountMeta::new_readonly(authority.key(), true),
        AccountMeta::new(bank.key(), false),
        AccountMeta::new(signer_token_account.key(), false),
        AccountMeta::new(liquidity_vault.key(), false),
        AccountMeta::new_readonly(token_program.key(), false),
    ];
    let mut data = MFI_DEPOSIT.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(0); // deposit_up_to_limit: Option<bool> = None
    let infos = [
        program.clone(),
        group.clone(),
        marginfi_account.clone(),
        authority.clone(),
        bank.clone(),
        signer_token_account.clone(),
        liquidity_vault.clone(),
        token_program.clone(),
    ];
    invoke(&Instruction { program_id: program.key(), accounts: metas, data }, &infos)
        .map_err(|e| {
            msg!("marginfi deposit failed: {:?}", e);
            error!(AdapterError::CpiFailed)
        })
}

/// CPI `lending_account_withdraw(amount, withdraw_all)`. The trailing `bank`+`oracle`
/// are the health-pulse remaining accounts MarginFi expects after the fixed set.
#[allow(clippy::too_many_arguments)]
pub fn withdraw<'info>(
    program: &AccountInfo<'info>,
    group: &AccountInfo<'info>,
    marginfi_account: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    bank: &AccountInfo<'info>,
    destination_token_account: &AccountInfo<'info>,
    bank_liquidity_vault_authority: &AccountInfo<'info>,
    liquidity_vault: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    oracle: &AccountInfo<'info>,
    amount: u64,
    withdraw_all: bool,
) -> Result<()> {
    let metas = vec![
        AccountMeta::new_readonly(group.key(), false),
        AccountMeta::new(marginfi_account.key(), false),
        AccountMeta::new_readonly(authority.key(), true),
        AccountMeta::new(bank.key(), false),
        AccountMeta::new(destination_token_account.key(), false),
        AccountMeta::new_readonly(bank_liquidity_vault_authority.key(), false),
        AccountMeta::new(liquidity_vault.key(), false),
        AccountMeta::new_readonly(token_program.key(), false),
        // health-pulse remaining accounts:
        AccountMeta::new_readonly(bank.key(), false),
        AccountMeta::new_readonly(oracle.key(), false),
    ];
    let mut data = MFI_WITHDRAW.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(1); // withdraw_all: Option<bool> = Some(_)
    data.push(if withdraw_all { 1 } else { 0 });
    let infos = [
        program.clone(),
        group.clone(),
        marginfi_account.clone(),
        authority.clone(),
        bank.clone(),
        destination_token_account.clone(),
        bank_liquidity_vault_authority.clone(),
        liquidity_vault.clone(),
        token_program.clone(),
        oracle.clone(),
    ];
    invoke(&Instruction { program_id: program.key(), accounts: metas, data }, &infos)
        .map_err(|e| {
            msg!("marginfi withdraw failed: {:?}", e);
            error!(AdapterError::CpiFailed)
        })
}
