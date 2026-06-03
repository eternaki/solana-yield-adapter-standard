// MarginFi v2 mainnet addresses for the USDC bank. Cloned into the fork so the
// adapter can CPI into real MarginFi state.
import { PublicKey } from "@solana/web3.js";

export const MARGINFI = {
  program: new PublicKey("MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA"),
  /// upgradeable-loader programData (executable bytes).
  programData: new PublicKey("4Q8u2ny8YYgytJEncwZQfVWbd5axZtfgDxYKHehvPQR7"),
  group: new PublicKey("4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8"),
  usdcBank: new PublicKey("2s37akK2eyBbp8DZgCm7RtsaEz8eJP3Nxd4urLHQv7yB"),
  liquidityVault: new PublicKey("7jaiZR5Sk8hdYN9MxTpczTcwbWpb5WEoxSANuUwveuat"),
  /// PriceUpdateV2 (Pyth pull) feed for USDC; its posted_slot is patched low in the fork.
  oracle: new PublicKey("Dpw1EAVrSB1ibxiDQyTAW6Zip3J4Btk2x4SgApQCeFbX"),
} as const;

/// PriceUpdateV2 `posted_slot` byte offset (verified by scanning the live feed):
/// 8 disc + 32 write_authority + 1 verification_level + 84 PriceFeedMessage = 125.
export const ORACLE_POSTED_SLOT_OFFSET = 125;

/// MarginFi PDA seeds.
export const MARGINFI_ACCOUNT_SEED = "marginfi_account";
export const LIQUIDITY_VAULT_AUTH_SEED = "liquidity_vault_auth";
