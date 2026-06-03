// Maple syrupUSDC swap-and-hold via the Orca syrupUSDC/USDC whirlpool (mainnet).
import { PublicKey } from "@solana/web3.js";

export const MAPLE = {
  orcaProgram: new PublicKey("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"),
  syrupMint: new PublicKey("AvZZF1YaZDziPY2RCK4oJrRVrbN3mTD9NL24hPeaZeUj"),
  whirlpool: new PublicKey("6fteKNvMdv7tYmBoJHhj1jx6rHcEwC6RdSEmVpyS613J"),
  vaultA: new PublicKey("FM2RuqFYo9umA1yc5FyQn6pSDZJZ1MXAdaekJZ4dQCvi"), // syrupUSDC (mint A)
  vaultB: new PublicKey("Fw6Xr45rBBrXbWJd5ZbSg44kacrKRLef4rHkZ8gWC5Ab"), // USDC (mint B)
  oracle: new PublicKey("H7j5FQpwTUMwxrWeuyrLr5Z9oHsPFiaRqNaERVsuE1c8"),
  // tick arrays around the current tick (1543; spacing 1; 88 ticks/array).
  taCurrent: new PublicKey("4yRC9NUHB2dwxfZyrqA8dDqH8GkcUVKU5F7W3ZPnbQtd"), // start 1496
  taUp1: new PublicKey("AdLyWhs7xrwkBFCYEo3n9BiwgXMZzXMefh8K9wMWoy1j"), // start 1584
  taUp2: new PublicKey("AofDEAkfQxcyeochNwxyQehYm6SpL3qrtxm7ZEZtPptp"), // start 1672
  taDown1: new PublicKey("9qUH5rp6Xw7NqghvbR9eQu6xTjEu5QTCHMbjdiiDVd5S"), // start 1408
  taDown2: new PublicKey("BQ95wDV5A7z4c9cExYMWE2KvcqhbdjoxXcoQ88erFtyH"), // start 1320
} as const;
