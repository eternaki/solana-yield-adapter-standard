// Initialize the on-chain adapter Registry (governance root) for the dispatcher.
// Idempotent: skips if already initialized.
//
//   ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
//   ANCHOR_WALLET=~/.config/solana/id.json \
//   yarn run ts-node scripts/init-registry.ts
import * as anchor from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";

const idl = require("../target/idl/dispatcher.json");

(async () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = new anchor.Program(idl, provider);
  const [registry] = PublicKey.findProgramAddressSync([Buffer.from("registry")], program.programId);

  const existing = await provider.connection.getAccountInfo(registry);
  if (existing) {
    const acc = await (program.account as any).registry.fetch(registry);
    console.log(`registry already initialized: ${registry.toBase58()} (admin ${acc.admin.toBase58()}, ${acc.adapterCount} adapters)`);
    return;
  }

  const sig = await program.methods
    .initializeRegistry()
    .accountsStrict({
      admin: provider.wallet.publicKey,
      registry,
      systemProgram: SystemProgram.programId,
    })
    .rpc();
  console.log(`registry initialized: ${registry.toBase58()}`);
  console.log(`admin: ${provider.wallet.publicKey.toBase58()}`);
  console.log(`tx: ${sig}`);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
