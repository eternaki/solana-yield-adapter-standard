// Kamino Lend (klend) mainnet account addresses for the main-market USDC reserve.
// Every address here is cloned into the bankrun fork so the local test can CPI
// into real Kamino state. Stable for the lifetime of the reserve.
import { PublicKey } from "@solana/web3.js";

export const USDC_MINT = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

export const KAMINO = {
  klendProgram: new PublicKey("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD"),
  /// klend programData (upgradeable-loader executable bytes) — must be cloned too.
  klendProgramData: new PublicKey("9uSbGW1y9H5Av6H5TKxQ1wnFApSq2t3oEpfF2YfjDQGA"),
  reserve: new PublicKey("D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59"),
  lendingMarket: new PublicKey("7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF"),
  lendingMarketAuthority: new PublicKey("9DrvZvyWh1HuAoZxvYWMvkf2XCzryCpGgHqrMjyDWpmo"),
  liquidityMint: USDC_MINT,
  liquiditySupplyVault: new PublicKey("Bgq7trRgVMeq33yt235zM2onQ4bRDBsY5EWiTetF4qw6"),
  collateralMint: new PublicKey("B8V6WVjPxW1UGwVDfxH2d2r8SyT4cqn7dQRK6XneVa7D"),
  collateralSupplyVault: new PublicKey("3DzjXRfxRm6iejfyyMynR4tScddaanrePJ1NJU2XnPPL"),
  /// USDC reserve prices via Scope; pyth/switchboard unset.
  scopePrices: new PublicKey("3t4JZcueEzTbVP6kLxXrL3VpWx45jDer4eqysweBchNH"),
} as const;

/// Reserve `last_update.slot` lives at byte offset 16 (after the 8-byte Anchor
/// discriminator + the 8-byte LastUpdate header start). Used to set the bankrun
/// clock at/after the reserve's cached slot so klend's `clock.slot - last_update`
/// math doesn't underflow.
export const RESERVE_LAST_UPDATE_SLOT_OFFSET = 16;
