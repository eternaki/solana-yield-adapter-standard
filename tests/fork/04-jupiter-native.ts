// Mainnet-fork end-to-end for jupiter-lp on a native solana-test-validator.
// Drives the full dispatcher flow against real Jupiter Perps JLP-pool state.
// add/remove liquidity charge a fee, so the round-trip is intentionally lossy
// (deposit 100 → ~99.x value → ~98.x out); tolerances reflect that.
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

import { JUPITER, JUP_CUSTODIES, JUP_DOVES } from "./jupiter-addresses";

const USDC_MINT = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const dispatcherIdl = require("../../target/idl/dispatcher.json");
const jupiterIdl = require("../../target/idl/jupiter_lp.json");

const DEPOSIT = 100_000_000n; // 100 USDC
const ro = (pubkey: PublicKey) => ({ pubkey, isSigner: false, isWritable: false });
const rw = (pubkey: PublicKey) => ({ pubkey, isSigner: false, isWritable: true });
const cu = ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 });

describe("jupiter-lp · native mainnet-fork e2e", () => {
  const connection = new Connection("http://127.0.0.1:8899", "confirmed");
  const owner = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(`${__dirname}/fixtures/test-payer.json`, "utf8"))),
  );
  const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(owner), { commitment: "confirmed" });
  anchor.setProvider(provider);
  const dispatcher = new anchor.Program(dispatcherIdl, provider);
  const adapter = new anchor.Program(jupiterIdl, provider);
  const aid = adapter.programId;

  const [registry] = PublicKey.findProgramAddressSync([Buffer.from("registry")], dispatcher.programId);
  const [adapterEntry] = PublicKey.findProgramAddressSync([Buffer.from("adapter_entry"), aid.toBuffer()], dispatcher.programId);
  const [position] = PublicKey.findProgramAddressSync([Buffer.from("position"), owner.publicKey.toBuffer(), aid.toBuffer()], dispatcher.programId);
  const [adapterState] = PublicKey.findProgramAddressSync([Buffer.from("adapter_state"), aid.toBuffer(), position.toBuffer()], aid);
  const positionPool = getAssociatedTokenAddressSync(USDC_MINT, position, true);
  const lpTokenAccount = getAssociatedTokenAddressSync(JUPITER.jlpMint, position, true);
  const ownerUsdcAta = getAssociatedTokenAddressSync(USDC_MINT, owner.publicKey);

  // add_liquidity2 / remove_liquidity2 tail (forwarded as remaining accounts).
  const liqTail = () => [
    ro(JUPITER.program), ro(JUPITER.transferAuthority), ro(JUPITER.perpetuals),
    rw(JUPITER.pool), rw(JUPITER.usdcCustody), ro(JUPITER.dovesOracle), ro(JUPITER.pythnetOracle),
    rw(JUPITER.custodyTokenAccount), rw(JUPITER.jlpMint), rw(lpTokenAccount),
    ro(TOKEN_PROGRAM_ID), ro(JUPITER.eventAuthority),
    // AUM accounts (forwarded to the CPI): every custody + its doves oracle.
    ...JUP_CUSTODIES.map(ro), ...JUP_DOVES.map(ro),
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
    await dispatcher.methods.whitelistAdapter("Jupiter LP", 1).accountsStrict({
      admin: owner.publicKey, registry, adapterEntry, adapterProgram: aid, systemProgram: SystemProgram.programId,
    }).rpc();
    expect((await (dispatcher.account as any).adapterEntry.fetch(adapterEntry)).isActive).to.equal(true);
  });

  it("opens a position (creates JLP token account)", async () => {
    await dispatcher.methods.openPosition().accountsStrict({
      owner: owner.publicKey, adapterEntry, adapterProgram: aid, position,
      usdcMint: USDC_MINT, positionUsdcPool: positionPool, adapterState,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY, systemProgram: SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID, associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    }).remainingAccounts([
      ro(JUPITER.jlpMint), rw(lpTokenAccount), ro(JUPITER.pool), ro(JUPITER.usdcCustody),
      ro(TOKEN_PROGRAM_ID), ro(ASSOCIATED_TOKEN_PROGRAM_ID),
    ]).preInstructions([cu]).rpc();
    expect((await (adapter.account as any).adapterState.fetch(adapterState)).pool.toBase58())
      .to.equal(JUPITER.pool.toBase58());
  });

  it("deposits 100 USDC into the JLP pool", async () => {
    const before = await bal(ownerUsdcAta);
    await dispatcher.methods.deposit(new anchor.BN(DEPOSIT.toString())).accountsStrict({
      owner: owner.publicKey, adapterEntry, adapterProgram: aid, position,
      positionUsdcPool: positionPool, ownerUsdcAta, adapterState,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY, tokenProgram: TOKEN_PROGRAM_ID,
    }).remainingAccounts(liqTail()).preInstructions([cu]).rpc();
    expect(before - (await bal(ownerUsdcAta))).to.equal(DEPOSIT);
    expect((await (adapter.account as any).adapterState.fetch(adapterState)).header.totalShares.toNumber())
      .to.be.greaterThan(0); // JLP minted
  });

  it("reports current value ≈ deposit (via return data)", async () => {
    const ix = await dispatcher.methods.currentValue().accountsStrict({
      adapterEntry, adapterProgram: aid, position, adapterState, instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
    }).remainingAccounts([ro(lpTokenAccount), ro(JUPITER.pool), ro(JUPITER.jlpMint)]).instruction();
    const tx = new Transaction().add(cu, ix);
    tx.feePayer = owner.publicKey;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    const sim = await connection.simulateTransaction(tx, [owner]);
    const rd = sim.value.returnData;
    expect(rd, `no return data; logs:\n${(sim.value.logs ?? []).join("\n")}`).to.not.be.null;
    const value = Buffer.from(rd!.data[0], "base64").readBigUInt64LE(0);
    // within ~3 USDC of the deposit (add-liquidity fee + AUM rounding)
    expect(value > DEPOSIT - 3_000_000n && value < DEPOSIT + 3_000_000n, `value=${value}`).to.equal(true);
  });

  it("withdraws all JLP back to USDC", async () => {
    const st = await (adapter.account as any).adapterState.fetch(adapterState);
    const before = await bal(ownerUsdcAta);
    await dispatcher.methods.withdraw(new anchor.BN(st.header.totalShares.toString())).accountsStrict({
      owner: owner.publicKey, adapterEntry, adapterProgram: aid, position,
      positionUsdcPool: positionPool, ownerUsdcAta, adapterState,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY, tokenProgram: TOKEN_PROGRAM_ID,
    }).remainingAccounts(liqTail()).preInstructions([cu]).rpc();
    // two fee legs (add + remove) → expect most of the deposit back.
    expect((await bal(ownerUsdcAta)) - before > DEPOSIT - 4_000_000n).to.equal(true);
    expect((await (adapter.account as any).adapterState.fetch(adapterState)).header.totalShares.toString()).to.equal("0");
  });
});
