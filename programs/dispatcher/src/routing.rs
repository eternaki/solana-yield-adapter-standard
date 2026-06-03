//! Builds and issues the dispatcher → adapter CPIs.
//!
//! The dispatcher controls a fixed account prefix per instruction (instructions
//! sysvar, position, pool, adapter_state); everything after is forwarded
//! verbatim from `remaining_accounts`, so the dispatcher never needs to know an
//! adapter's underlying protocol layout. Results are read back via the
//! interface crate's return-data codec.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
};

use adapter_interface::{encode_current_value, encode_deposit, encode_initialize, encode_withdraw, get_u64_return};

use crate::error::DispatcherError;

fn forward<'a>(
    metas: &mut Vec<AccountMeta>,
    infos: &mut Vec<AccountInfo<'a>>,
    remaining: &[AccountInfo<'a>],
) {
    for acc in remaining {
        metas.push(AccountMeta {
            pubkey: acc.key(),
            is_signer: acc.is_signer,
            is_writable: acc.is_writable,
        });
        infos.push(acc.clone());
    }
}

/// CPI `adapter_initialize`. Signed by the `position` PDA so an adapter whose
/// underlying init needs the position as a signing authority (e.g. MarginFi's
/// account PDA) works; the rent payer (top-level owner) covers account creation.
/// Prefix: [instructions_sysvar, position(signer), adapter_state, rent_payer, system_program].
#[allow(clippy::too_many_arguments)]
pub fn route_initialize<'info>(
    adapter_program: &AccountInfo<'info>,
    instructions_sysvar: &AccountInfo<'info>,
    position: &AccountInfo<'info>,
    adapter_state: &AccountInfo<'info>,
    rent_payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    remaining: &[AccountInfo<'info>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut metas = vec![
        AccountMeta::new_readonly(instructions_sysvar.key(), false),
        AccountMeta::new_readonly(position.key(), true),
        AccountMeta::new(adapter_state.key(), false),
        AccountMeta::new(rent_payer.key(), true),
        AccountMeta::new_readonly(system_program.key(), false),
    ];
    let mut infos = vec![
        adapter_program.clone(),
        instructions_sysvar.clone(),
        position.clone(),
        adapter_state.clone(),
        rent_payer.clone(),
        system_program.clone(),
    ];
    forward(&mut metas, &mut infos, remaining);
    let ix = Instruction {
        program_id: adapter_program.key(),
        accounts: metas,
        data: encode_initialize(&[]),
    };
    invoke_signed(&ix, &infos, signer_seeds).map_err(|e| {
        msg!("adapter_initialize CPI failed: {:?}", e);
        error!(DispatcherError::AdapterMismatch)
    })
}

/// Shared builder for the deposit/withdraw prefix:
/// [instructions_sysvar, position(signer), position_usdc_pool, adapter_state].
fn fund_movement_ix<'info>(
    data: Vec<u8>,
    adapter_program: &AccountInfo<'info>,
    instructions_sysvar: &AccountInfo<'info>,
    position: &AccountInfo<'info>,
    position_usdc_pool: &AccountInfo<'info>,
    adapter_state: &AccountInfo<'info>,
    remaining: &[AccountInfo<'info>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut metas = vec![
        AccountMeta::new_readonly(instructions_sysvar.key(), false),
        AccountMeta::new(position.key(), true), // signs the underlying CPI
        AccountMeta::new(position_usdc_pool.key(), false),
        AccountMeta::new(adapter_state.key(), false),
    ];
    let mut infos = vec![
        adapter_program.clone(),
        instructions_sysvar.clone(),
        position.clone(),
        position_usdc_pool.clone(),
        adapter_state.clone(),
    ];
    forward(&mut metas, &mut infos, remaining);
    let ix = Instruction {
        program_id: adapter_program.key(),
        accounts: metas,
        data,
    };
    invoke_signed(&ix, &infos, signer_seeds).map_err(|e| {
        msg!("adapter fund-movement CPI failed: {:?}", e);
        error!(DispatcherError::AdapterMismatch)
    })
}

/// CPI `adapter_deposit`, returning the adapter's reported `shares_minted`.
#[allow(clippy::too_many_arguments)]
pub fn route_deposit<'info>(
    adapter_program: &AccountInfo<'info>,
    instructions_sysvar: &AccountInfo<'info>,
    position: &AccountInfo<'info>,
    position_usdc_pool: &AccountInfo<'info>,
    adapter_state: &AccountInfo<'info>,
    remaining: &[AccountInfo<'info>],
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<u64> {
    fund_movement_ix(
        encode_deposit(amount, &[]),
        adapter_program,
        instructions_sysvar,
        position,
        position_usdc_pool,
        adapter_state,
        remaining,
        signer_seeds,
    )?;
    get_u64_return(&adapter_program.key()).ok_or(error!(DispatcherError::MissingReturnData))
}

/// CPI `adapter_withdraw`, returning the adapter's reported `amount_out`.
#[allow(clippy::too_many_arguments)]
pub fn route_withdraw<'info>(
    adapter_program: &AccountInfo<'info>,
    instructions_sysvar: &AccountInfo<'info>,
    position: &AccountInfo<'info>,
    position_usdc_pool: &AccountInfo<'info>,
    adapter_state: &AccountInfo<'info>,
    remaining: &[AccountInfo<'info>],
    shares: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<u64> {
    fund_movement_ix(
        encode_withdraw(shares, &[]),
        adapter_program,
        instructions_sysvar,
        position,
        position_usdc_pool,
        adapter_state,
        remaining,
        signer_seeds,
    )?;
    get_u64_return(&adapter_program.key()).ok_or(error!(DispatcherError::MissingReturnData))
}

/// CPI `adapter_current_value` (read-only), returning the adapter's USDC value.
/// Prefix: [instructions_sysvar, position, adapter_state].
pub fn route_current_value<'info>(
    adapter_program: &AccountInfo<'info>,
    instructions_sysvar: &AccountInfo<'info>,
    position: &AccountInfo<'info>,
    adapter_state: &AccountInfo<'info>,
    remaining: &[AccountInfo<'info>],
) -> Result<u64> {
    let mut metas = vec![
        AccountMeta::new_readonly(instructions_sysvar.key(), false),
        AccountMeta::new_readonly(position.key(), false),
        AccountMeta::new_readonly(adapter_state.key(), false),
    ];
    let mut infos = vec![
        adapter_program.clone(),
        instructions_sysvar.clone(),
        position.clone(),
        adapter_state.clone(),
    ];
    forward(&mut metas, &mut infos, remaining);
    let ix = Instruction {
        program_id: adapter_program.key(),
        accounts: metas,
        data: encode_current_value(&[]),
    };
    invoke(&ix, &infos).map_err(|e| {
        msg!("adapter_current_value CPI failed: {:?}", e);
        error!(DispatcherError::AdapterMismatch)
    })?;
    get_u64_return(&adapter_program.key()).ok_or(error!(DispatcherError::MissingReturnData))
}
