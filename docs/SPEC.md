# Solana Yield Adapter Standard — v1

A minimal, composable interface that lets any yield source (a lending reserve, an
LP, a staking pool, a tokenized RWA) plug into a single **dispatcher** through a
governance-gated **registry**, without ever upgrading the dispatcher.

- **Dispatcher** — a thin router. Users open a `Position` into a whitelisted adapter,
  then `deposit` / `withdraw` / read `current_value` through the dispatcher, which
  CPIs the adapter's standard instructions.
- **Registry** — an on-chain whitelist of approved adapter programs (admin-gated).
- **Adapter** — an independent Anchor program implementing four instructions against
  one underlying protocol.

```
user ──▶ dispatcher.deposit ──▶ (registry check) ──CPI──▶ adapter.adapter_deposit ──CPI──▶ protocol
                                                              │
                                          set_return_data(shares) + emit event
```

The dispatcher knows nothing about any protocol. An adapter knows nothing about the
user — it trusts only the dispatcher (verified on every call).

## Versioning

Every adapter declares `ADAPTER_INTERFACE_VERSION: u8 = 1`. The registry stores it per
adapter; the dispatcher refuses to route to a version it doesn't implement. Bump on any
breaking change to instruction args, account order, or the state header.

## Adapter interface

An adapter MUST expose exactly these four instructions, with the **exact account order**
below. Accounts `0..N` are the *standard prefix* (supplied by the dispatcher); accounts
after it are *adapter-specific* and forwarded verbatim from the dispatcher's
`remaining_accounts`. Anchor instruction discriminators are the usual
`sha256("global:<name>")[..8]`.

All four take a trailing `adapter_data: Vec<u8>` (≤ `MAX_ADAPTER_DATA_LEN` = 256) for
optional adapter-specific config; pass empty when unused.

### `adapter_initialize(adapter_data)`

Creates the adapter-owned `AdapterState` PDA and any token accounts the adapter needs.
Idempotent per position. No funds move.

| # | account | notes |
|---|---|---|
| 0 | `instructions_sysvar` | for caller verification |
| 1 | `position` | the dispatcher `Position` PDA (state seed + funds authority) |
| 2 | `adapter_state` | `init`, the adapter's PDA |
| 3 | `rent_payer` | signer, pays rent |
| 4 | `system_program` | |
| 5+ | adapter-specific | e.g. reserve, collateral mint, collateral vault |

### `adapter_deposit(amount: u64, adapter_data)`

Pulls `amount` (USDC base units) from `position_usdc_pool`, deposits into the source.
**Returns `shares_minted: u64`** via `set_return_data` and emits a deposit event.

| # | account | notes |
|---|---|---|
| 0 | `instructions_sysvar` | |
| 1 | `position` | **signer** — signs the underlying CPI (the dispatcher `invoke_signed`s its seeds) |
| 2 | `position_usdc_pool` | source USDC token account, owned by `position` |
| 3 | `adapter_state` | |
| 4+ | adapter-specific | the protocol's deposit accounts |

### `adapter_withdraw(shares: u64, adapter_data)`

Redeems `shares` (adapter-native units) from the source, returns USDC to
`position_usdc_pool`. **Returns `amount_out: u64`**. Account layout identical to deposit.

### `adapter_current_value(adapter_data)`

Read-only. **Returns `value_usdc: u64`** — the USDC value of this position's holdings.

| # | account | notes |
|---|---|---|
| 0 | `instructions_sysvar` | |
| 1 | `position` | |
| 2 | `adapter_state` | |
| 3+ | adapter-specific | oracle / reserve state needed to price the holdings |

### Return values

The canonical result channel is **`set_return_data`** (Borsh `u64`, little-endian): the
dispatcher reads it synchronously after the CPI, so the standard composes like an
ERC-4626 call. Each instruction ALSO emits an event for off-chain indexers — return data
for programs, events for humans.

## Adapter state

The adapter owns one PDA per position:

```
seeds = [ADAPTER_STATE_SEED, adapter_program_id, position]   // ADAPTER_STATE_SEED = b"adapter_state"
```

Its account MUST begin with the standard header, so any reader can decode
position/ownership/shares without knowing the adapter-specific tail:

```rust
pub struct AdapterStateHeader {
    pub position: Pubkey,          // dispatcher Position this state backs
    pub adapter_program: Pubkey,   // this adapter's program id
    pub created_at: i64,
    pub total_shares: u64,         // adapter-native units (cTokens, LP, stake shares…)
}
// pub struct AdapterState { pub header: AdapterStateHeader, /* adapter-specific … */ }
```

## Security requirements

Every adapter MUST:

1. **Verify the caller is the dispatcher** — read the currently-executing instruction
   from the instructions sysvar (`load_current_index_checked` +
   `load_instruction_at_checked`) and require its `program_id == DISPATCHER_ID`. A fixed
   index is wrong: ComputeBudget or other preamble instructions may precede the call.
2. Use **checked arithmetic** (`checked_add/_sub/_mul/_div`) everywhere.
3. Reject `amount == 0` / `shares == 0`.
4. **Validate `remaining_accounts`** against the expected underlying layout (bind the
   reserve/pool/mint to `adapter_state`; check program ownership / `executable`).
5. Sign the underlying CPI with the **`position` PDA** (its signer flag propagates from
   the dispatcher's `invoke_signed`) — funds never touch the dispatcher operator.
6. Declare and honor `ADAPTER_INTERFACE_VERSION`.

## Canonical error codes

Shared across all adapters (so a code means the same thing everywhere). Anchor numbers
them from 6000 in order:

| code | name | meaning |
|---|---|---|
| 6000 | `Unauthorized` | caller is not the registered dispatcher |
| 6001 | `ZeroAmount` | `amount`/`shares` is zero |
| 6002 | `InsufficientShares` | withdraw exceeds tracked shares |
| 6003 | `MathOverflow` | checked arithmetic overflowed |
| 6004 | `VersionMismatch` | `adapter_data` version mismatch |
| 6005 | `StaleOracle` | priced from stale oracle/reserve state |
| 6006 | `AdapterStateUninit` | `adapter_state` not initialized |
| 6007 | `AdapterDataTooLarge` | `adapter_data` > 256 bytes |
| 6008 | `CpiFailed` | underlying protocol CPI failed |

## Registry (governance)

PDAs in the dispatcher program:

```rust
Registry    { admin, adapter_count, bump }                 // seeds = [b"registry"]
AdapterEntry{ adapter_program, interface_version, name,    // seeds = [b"adapter_entry", adapter_program]
              is_active, added_at, bump }
```

- `initialize_registry()` — one-time; sets `admin` (swap for a multisig in production).
- `whitelist_adapter(name, interface_version)` — admin-only; requires the adapter program
  be `executable` and the version match.
- `pause_adapter(paused)` — admin toggles `is_active`; the entry is never removed, so
  `adapter_count` is monotonic and history is preserved.

Routing requires a live entry: the dispatcher checks `is_active`, the version, and that
the supplied `adapter_program` matches the position's.

## Dispatcher entrypoints

`initialize_registry`, `whitelist_adapter`, `pause_adapter`, `open_position`,
`deposit(amount)`, `withdraw(shares)`, `current_value`. The last four CPI the adapter's
matching instruction with the standard prefix + forwarded `remaining_accounts`, and read
the adapter's `u64` return value.

See `crates/adapter-interface` for the shared constants, discriminators, arg encoders,
return-data codec, header, and `verify_caller_is_dispatcher` — both sides depend on it,
so they can't drift. Reference implementation: `programs/kamino-usdc`.
