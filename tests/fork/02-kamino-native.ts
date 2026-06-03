// Mainnet-fork end-to-end for kamino-usdc on a NATIVE solana-test-validator.
//
// bankrun can't execute klend's heavy refresh+deposit within its deadline, so the
// fund-moving steps run against a real (JIT) validator warped to the reserve's slot
// (see scripts/run-fork-validator.sh). This drives the full dispatcher flow and
// asserts USDC round-trips through real Kamino CPIs.
//
// Prereq: the validator is up on :8899 with klend cloned + the test-payer USDC
// account injected, and both programs are deployed. Run via scripts/fork-test.sh.
import * as anchor from "@coral-xyz/anchor";
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  ComputeBudgetProgram,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  getAccount,
} from "@solana/spl-token";
import { expect } from "chai";
import * as fs from "fs";

import { KAMINO, USDC_MINT } from "./kamino-addresses";

const dispatcherIdl = require("../../target/idl/dispatcher.json");
const kaminoIdl = require("../../target/idl/kamino_usdc.json");

const DEPOSIT = 100_000_000n; // 100 USDC
const ro = (pubkey: PublicKey) => ({ pubkey, isSigner: false, isWritable: false });
const rw = (pubkey: PublicKey) => ({ pubkey, isSigner: false, isWritable: true });
const cu = ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 });

describe("kamino-usdc · native mainnet-fork e2e", () => {
  const connection = new Connection("http://127.0.0.1:8899", "confirmed");
  const owner = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(`${__dirname}/fixtures/test-payer.json`, "utf8"))),
  );
  const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(owner), {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);
  const dispatcher = new anchor.Program(dispatcherIdl, provider);
  const kamino = new anchor.Program(kaminoIdl, provider);
  const kid = kamino.programId;

  const [registry] = PublicKey.findProgramAddressSync([Buffer.from("registry")], dispatcher.programId);
  const [adapterEntry] = PublicKey.findProgramAddressSync([Buffer.from("adapter_entry"), kid.toBuffer()], dispatcher.programId);
  const [position] = PublicKey.findProgramAddressSync([Buffer.from("position"), owner.publicKey.toBuffer(), kid.toBuffer()], dispatcher.programId);
  const [adapterState] = PublicKey.findProgramAddressSync([Buffer.from("adapter_state"), kid.toBuffer(), position.toBuffer()], kid);
  const positionPool = getAssociatedTokenAddressSync(USDC_MINT, position, true);
  const collateralVault = getAssociatedTokenAddressSync(KAMINO.collateralMint, position, true);
  const ownerUsdcAta = getAssociatedTokenAddressSync(USDC_MINT, owner.publicKey);

  const fundMoveRemaining = () => [
    rw(KAMINO.reserve),
    ro(KAMINO.lendingMarket),
    ro(KAMINO.lendingMarketAuthority),
    ro(USDC_MINT),
    rw(KAMINO.liquiditySupplyVault),
    rw(KAMINO.collateralMint),
    rw(collateralVault),
    ro(TOKEN_PROGRAM_ID),
    ro(TOKEN_PROGRAM_ID),
    ro(KAMINO.scopePrices),
    ro(KAMINO.klendProgram),
  ];

  const bal = async (ata: PublicKey): Promise<bigint> => {
    try {
      return (await getAccount(connection, ata)).amount;
    } catch {
      return 0n;
    }
  };

  before(async () => {
    const sig = await connection.requestAirdrop(owner.publicKey, 100 * LAMPORTS_PER_SOL);
    await connection.confirmTransaction(sig, "confirmed");
  });

  it("initializes registry + whitelists the adapter", async () => {
    await dispatcher.methods.initializeRegistry().accountsStrict({
      admin: owner.publicKey, registry, systemProgram: SystemProgram.programId,
    }).rpc();
    await dispatcher.methods.whitelistAdapter("Kamino USDC", 1).accountsStrict({
      admin: owner.publicKey, registry, adapterEntry, adapterProgram: kid, systemProgram: SystemProgram.programId,
    }).rpc();
    const entry = await (dispatcher.account as any).adapterEntry.fetch(adapterEntry);
    expect(entry.isActive).to.equal(true);
  });

  it("opens a position", async () => {
    await dispatcher.methods.openPosition().accountsStrict({
      owner: owner.publicKey, adapterEntry, adapterProgram: kid, position,
      usdcMint: USDC_MINT, positionUsdcPool: positionPool, adapterState,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY, systemProgram: SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID, associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    }).remainingAccounts([
      ro(KAMINO.reserve), ro(KAMINO.collateralMint), rw(collateralVault),
      ro(TOKEN_PROGRAM_ID), ro(ASSOCIATED_TOKEN_PROGRAM_ID),
    ]).preInstructions([cu]).rpc();
    const st = await (kamino.account as any).adapterState.fetch(adapterState);
    expect(st.kaminoReserve.toBase58()).to.equal(KAMINO.reserve.toBase58());
  });

  it("deposits 100 USDC into Kamino", async () => {
    const before = await bal(ownerUsdcAta);
    await dispatcher.methods.deposit(new anchor.BN(DEPOSIT.toString())).accountsStrict({
      owner: owner.publicKey, adapterEntry, adapterProgram: kid, position,
      positionUsdcPool: positionPool, ownerUsdcAta, adapterState,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY, tokenProgram: TOKEN_PROGRAM_ID,
    }).remainingAccounts(fundMoveRemaining()).preInstructions([cu]).rpc();
    expect((before - (await bal(ownerUsdcAta)))).to.equal(DEPOSIT);
    const st = await (kamino.account as any).adapterState.fetch(adapterState);
    expect(st.header.totalShares.toNumber()).to.be.greaterThan(0);
    // Effectively all routed into klend (allow sub-cent rounding dust in the pool).
    expect(Number(await bal(positionPool))).to.be.lessThan(10);
  });

  it("reports current value ≈ deposit (via return data)", async () => {
    const ix = await dispatcher.methods.currentValue().accountsStrict({
      adapterEntry, adapterProgram: kid, position, adapterState,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
    }).remainingAccounts([
      rw(KAMINO.reserve), ro(KAMINO.lendingMarket), ro(KAMINO.scopePrices), ro(KAMINO.klendProgram),
    ]).instruction();
    const tx = new Transaction().add(cu, ix);
    tx.feePayer = owner.publicKey;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    const sim = await connection.simulateTransaction(tx, [owner]);
    const rd = sim.value.returnData;
    expect(rd, `no return data; logs:\n${(sim.value.logs ?? []).join("\n")}`).to.not.be.null;
    const value = Buffer.from(rd!.data[0], "base64").readBigUInt64LE(0);
    expect(
      value > DEPOSIT - 1_000_000n && value < DEPOSIT + 1_000_000n,
      `value=${value} not within 1 USDC of ${DEPOSIT}`,
    ).to.equal(true);
  });

  it("withdraws all shares back to USDC", async () => {
    const st = await (kamino.account as any).adapterState.fetch(adapterState);
    const before = await bal(ownerUsdcAta);
    await dispatcher.methods.withdraw(new anchor.BN(st.header.totalShares.toString())).accountsStrict({
      owner: owner.publicKey, adapterEntry, adapterProgram: kid, position,
      positionUsdcPool: positionPool, ownerUsdcAta, adapterState,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY, tokenProgram: TOKEN_PROGRAM_ID,
    }).remainingAccounts(fundMoveRemaining()).preInstructions([cu]).rpc();
    expect((await bal(ownerUsdcAta)) - before > DEPOSIT - 1_000_000n).to.equal(true);
    const stAfter = await (kamino.account as any).adapterState.fetch(adapterState);
    expect(stAfter.header.totalShares.toString()).to.equal("0");
  });
});
