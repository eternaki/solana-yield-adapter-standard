// Whitelist an adapter program in the registry (admin only).
//
//   ANCHOR_PROVIDER_URL=https://api.devnet.solana.com ANCHOR_WALLET=~/.config/solana/id.json \
//   yarn run ts-node scripts/whitelist-adapter.ts <ADAPTER_PROGRAM_ID> "<Name>"
import * as anchor from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";

const idl = require("../target/idl/dispatcher.json");
const INTERFACE_VERSION = 1;

(async () => {
  const [, , adapterIdArg, name] = process.argv;
  if (!adapterIdArg || !name) {
    console.error('usage: whitelist-adapter.ts <ADAPTER_PROGRAM_ID> "<Name>"');
    process.exit(1);
  }
  const adapterProgram = new PublicKey(adapterIdArg);
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = new anchor.Program(idl, provider);
  const [registry] = PublicKey.findProgramAddressSync([Buffer.from("registry")], program.programId);
  const [adapterEntry] = PublicKey.findProgramAddressSync(
    [Buffer.from("adapter_entry"), adapterProgram.toBuffer()],
    program.programId,
  );

  const sig = await program.methods
    .whitelistAdapter(name, INTERFACE_VERSION)
    .accountsStrict({
      admin: provider.wallet.publicKey,
      registry,
      adapterEntry,
      adapterProgram,
      systemProgram: SystemProgram.programId,
    })
    .rpc();
  console.log(`whitelisted "${name}" (${adapterProgram.toBase58()})`);
  console.log(`entry: ${adapterEntry.toBase58()}`);
  console.log(`tx: ${sig}`);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
