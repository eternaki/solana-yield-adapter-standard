# Build your own adapter — in under a day

This walks you from zero to a whitelisted, fork-tested adapter for any Solana yield
source. You only ever write **one new program**; the dispatcher and registry never
change. Copy `programs/kamino-usdc` as your template — it implements the full standard.

Budget: ~2–6 hours depending on how exotic your protocol's CPI is.

## 0. Prerequisites

- Anchor `0.31.1`, Solana `2.2.20`, Rust edition 2021 (the repo pins matching deps —
  `anchor build` "just works" from a clean checkout).
- Know your protocol's deposit / withdraw / value instructions and their accounts
  (from its IDL or SDK).

## 1. Scaffold (10 min)

```bash
cp -r programs/kamino-usdc programs/<your-adapter>
# rename in Cargo.toml ([package].name + [lib].name) and add it as a workspace member
solana-keygen new -s --no-bip39-passphrase -o target/deploy/<your_adapter>-keypair.json
anchor keys sync     # writes the real id into declare_id! + Anchor.toml
```

Add your program to `Anchor.toml` `[programs.localnet]`/`[programs.devnet]`.

## 2. Define your state tail (10 min)

Keep `header: AdapterStateHeader` first (the standard requires it), then add whatever
your protocol needs to track the position:

```rust
#[account]
#[derive(InitSpace)]
pub struct AdapterState {
    pub header: AdapterStateHeader,   // standard — do not move
    pub pool: Pubkey,                 // e.g. your reserve / vault / market
    pub deposit_account: Pubkey,      // adapter-owned token account that holds shares
    pub bump: u8,
}
```

## 3. Implement the four instructions (1–3 hrs)

The contract is fixed (see `SPEC.md`). Reuse the shared helpers from
`adapter-interface` so you can't drift from the dispatcher:

```rust
use adapter_interface::{
    verify_caller_is_dispatcher, set_u64_return, AdapterError, AdapterStateHeader,
    ADAPTER_STATE_SEED, DISPATCHER_ID, MAX_ADAPTER_DATA_LEN,
};
```

Every handler starts the same way:

```rust
require!(amount > 0, AdapterError::ZeroAmount);
require!(adapter_data.len() <= MAX_ADAPTER_DATA_LEN, AdapterError::AdapterDataTooLarge);
verify_caller_is_dispatcher(&ctx.accounts.instructions_sysvar, &DISPATCHER_ID)?;
```

- **`adapter_initialize`** — `init` the `AdapterState` PDA
  (`seeds = [ADAPTER_STATE_SEED, crate::ID.as_ref(), position.key().as_ref()]`) + any
  token accounts (authority = `position`). Set the header.
- **`adapter_deposit`** — measure your shares balance before, CPI your protocol's deposit
  (signed by `position`), reload, `shares_minted = after - before`, add to
  `header.total_shares`, then `set_u64_return(shares_minted)` + `emit!`.
- **`adapter_withdraw`** — symmetric: CPI redeem, `amount_out = usdc_after - usdc_before`,
  subtract shares, `set_u64_return(amount_out)` + `emit!`.
- **`adapter_current_value`** — read your protocol's exchange rate / oracle, compute
  `total_shares × rate`, `set_u64_return(value)` + `emit!`.

Put the raw protocol CPI in its own module (see `kamino-usdc/src/klend_cpi.rs`): build
the `Instruction` with the protocol's discriminator + account metas and `invoke`. The
`position` signer propagates from the dispatcher — you don't pass seeds yourself.

**Account order matters.** Your `#[derive(Accounts)]` MUST place the standard prefix
first (see `SPEC.md`), then your protocol accounts. The dispatcher forwards the latter
via `remaining_accounts` in exactly the order you declare them.

### Checklist (the gotchas)

- [ ] Caller verified via the instructions sysvar (not a fixed index).
- [ ] `checked_*` math only; reject zero amounts.
- [ ] Bind protocol accounts to `adapter_state` (`address = adapter_state.pool @ …`) and
      check `executable` / ownership on the CPI target program.
- [ ] Token-2022 vs classic SPL: pick the right token program + ATA derivation.
- [ ] `set_u64_return` on deposit/withdraw/value (the dispatcher reads it).

## 4. Fork-test it (1 hr)

Copy `tests/fork/02-kamino-native.ts` and `scripts/fork-test.sh`; swap in your program,
your protocol's account addresses, and the `remaining_accounts` your instructions expect.

If your protocol caches a slot (like klend's `last_update.slot`) and would underflow or
go stale on a fork, reuse the trick in `scripts/gen-fork-fixtures.js`: clone the account
and rewrite its cached slot low, then inject it with `--account` instead of `--clone`
(no validator warp — warping to a mainnet slot can exceed the OS open-file limit). Fund
the test wallet with a fabricated token account (you can't mint a real stablecoin).

```bash
anchor build
MAINNET_RPC_URL=<rpc> ./scripts/fork-test.sh
```

Green means deposit → value → withdraw round-trips through real mainnet state.

## 5. Ship it

```bash
anchor deploy --provider.cluster devnet --program-name <your-adapter>
# governance whitelists it (admin only):
#   dispatcher.whitelist_adapter("<Name>", 1) with the adapter program account
```

Once the registry entry is active, users can `open_position` + `deposit` into your
source through the dispatcher. Done — no dispatcher upgrade, no coordination.
