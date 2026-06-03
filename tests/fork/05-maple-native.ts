// Mainnet-fork end-to-end for maple-syrup on a native solana-test-validator.
// Swap-and-hold via the real Orca syrupUSDC/USDC whirlpool. Round-trip is lossy
// (two swap fees), so tolerances reflect that.
import * as anchor from "@coral-xyz/anchor";
import {
  Connection, Keypair, PublicKey, Transaction, SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY, ComputeBudgetProgram, LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync, getAccount,
} from "@solana/spl-token";
import { expect } from "chai";
import * as fs from "fs";

import { MAPLE } from "./maple-addresses";

const USDC_MINT = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const dispatcherIdl = require("../../target/idl/dispatcher.json");
const mapleIdl = require("../../target/idl/maple_syrup.json");

const DEPOSIT = 100_000_000n; // 100 USDC
const ro = (pubkey: PublicKey) => ({ pubkey, isSigner: false, isWritable: false });
const rw = (pubkey: PublicKey) => ({ pubkey, isSigner: false, isWritable: true });
const cu = ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 });

describe("maple-syrup · native mainnet-fork e2e", () => {
  const connection = new Connection("http://127.0.0.1:8899", "confirmed");
  const owner = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(`${__dirname}/fixtures/test-payer.json`, "utf8"))),
  );
  const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(owner), { commitment: "confirmed" });
  anchor.setProvider(provider);
  const dispatcher = new anchor.Program(dispatcherIdl, provider);
  const adapter = new anchor.Program(mapleIdl, provider);
  const aid = adapter.programId;

  const [registry] = PublicKey.findProgramAddressSync([Buffer.from("registry")], dispatcher.programId);
  const [adapterEntry] = PublicKey.findProgramAddressSync([Buffer.from("adapter_entry"), aid.toBuffer()], dispatcher.programId);
  const [position] = PublicKey.findProgramAddressSync([Buffer.from("position"), owner.publicKey.toBuffer(), aid.toBuffer()], dispatcher.programId);
  const [adapterState] = PublicKey.findProgramAddressSync([Buffer.from("adapter_state"), aid.toBuffer(), position.toBuffer()], aid);
  const positionPool = getAssociatedTokenAddressSync(USDC_MINT, position, true);
  const syrupAccount = getAssociatedTokenAddressSync(MAPLE.syrupMint, position, true);
  const ownerUsdcAta = getAssociatedTokenAddressSync(USDC_MINT, owner.publicKey);

  // Orca swap tail; tick arrays differ by direction (deposit B→A ticks up; withdraw A→B ticks down).
  const swapTail = (up: boolean) => [
    ro(MAPLE.orcaProgram), rw(MAPLE.whirlpool), rw(syrupAccount), rw(MAPLE.vaultA), rw(MAPLE.vaultB),
    rw(MAPLE.taCurrent),
    up ? rw(MAPLE.taUp1) : rw(MAPLE.taDown1),
    up ? rw(MAPLE.taUp2) : rw(MAPLE.taDown2),
    rw(MAPLE.oracle), ro(TOKEN_PROGRAM_ID),
  ];
  const bal = async (a: PublicKey): Promise<bigint> => {
    try { return (await getAccount(connection, a)).amount; } catch { return 0n; }
  };

  before(async () => {
    await connection.confirmTransaction(await connection.requestAirdrop(owner.publicKey, 100 * LAMPORTS_PER_SOL), "confirmed");
  });

  it("initializes registry + whitelists the adapter", async () => {
    try {
      await dispatcher.methods.initializeRegistry().accountsStrict({
        admin: owner.publicKey, registry, systemProgram: SystemProgram.programId,
      }).rpc();
    } catch (_) { /* shared registry already initialized */ }
    await dispatcher.methods.whitelistAdapter("Maple syrupUSDC", 1).accountsStrict({
      admin: owner.publicKey, registry, adapterEntry, adapterProgram: aid, systemProgram: SystemProgram.programId,
    }).rpc();
    expect((await (dispatcher.account as any).adapterEntry.fetch(adapterEntry)).isActive).to.equal(true);
  });

  it("opens a position (creates syrupUSDC account)", async () => {
    await dispatcher.methods.openPosition().accountsStrict({
      owner: owner.publicKey, adapterEntry, adapterProgram: aid, position,
      usdcMint: USDC_MINT, positionUsdcPool: positionPool, adapterState,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY, systemProgram: SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID, associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    }).remainingAccounts([
      ro(MAPLE.syrupMint), rw(syrupAccount), ro(MAPLE.whirlpool),
      ro(TOKEN_PROGRAM_ID), ro(ASSOCIATED_TOKEN_PROGRAM_ID),
    ]).preInstructions([cu]).rpc();
    expect((await (adapter.account as any).adapterState.fetch(adapterState)).whirlpool.toBase58())
      .to.equal(MAPLE.whirlpool.toBase58());
  });

  it("deposits 100 USDC (swaps into syrupUSDC)", async () => {
    const before = await bal(ownerUsdcAta);
    await dispatcher.methods.deposit(new anchor.BN(DEPOSIT.toString())).accountsStrict({
      owner: owner.publicKey, adapterEntry, adapterProgram: aid, position,
      positionUsdcPool: positionPool, ownerUsdcAta, adapterState,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY, tokenProgram: TOKEN_PROGRAM_ID,
    }).remainingAccounts(swapTail(true)).preInstructions([cu]).rpc();
    expect(before - (await bal(ownerUsdcAta))).to.equal(DEPOSIT);
    expect((await (adapter.account as any).adapterState.fetch(adapterState)).header.totalShares.toNumber())
      .to.be.greaterThan(0); // syrupUSDC received
  });

  it("reports current value ≈ deposit (via return data)", async () => {
    const ix = await dispatcher.methods.currentValue().accountsStrict({
      adapterEntry, adapterProgram: aid, position, adapterState, instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
    }).remainingAccounts([ro(syrupAccount), ro(MAPLE.whirlpool)]).instruction();
    const tx = new Transaction().add(cu, ix);
    tx.feePayer = owner.publicKey;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    const sim = await connection.simulateTransaction(tx, [owner]);
    const rd = sim.value.returnData;
    expect(rd, `no return data; logs:\n${(sim.value.logs ?? []).join("\n")}`).to.not.be.null;
    const value = Buffer.from(rd!.data[0], "base64").readBigUInt64LE(0);
    // value = syrupUSDC × spot price ≈ deposit minus the entry swap fee.
    expect(value > DEPOSIT - 2_000_000n && value < DEPOSIT + 2_000_000n, `value=${value}`).to.equal(true);
  });

  it("withdraws all syrupUSDC back to USDC", async () => {
    const st = await (adapter.account as any).adapterState.fetch(adapterState);
    const before = await bal(ownerUsdcAta);
    await dispatcher.methods.withdraw(new anchor.BN(st.header.totalShares.toString())).accountsStrict({
      owner: owner.publicKey, adapterEntry, adapterProgram: aid, position,
      positionUsdcPool: positionPool, ownerUsdcAta, adapterState,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY, tokenProgram: TOKEN_PROGRAM_ID,
    }).remainingAccounts(swapTail(false)).preInstructions([cu]).rpc();
    // two swap fees on the round-trip → most of the deposit back.
    expect((await bal(ownerUsdcAta)) - before > DEPOSIT - 3_000_000n).to.equal(true);
    expect((await (adapter.account as any).adapterState.fetch(adapterState)).header.totalShares.toString()).to.equal("0");
  });
});
