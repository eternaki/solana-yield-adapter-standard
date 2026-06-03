// Mainnet-fork end-to-end for marginfi-usdc on a native solana-test-validator.
// Drives the full dispatcher flow against real MarginFi v2 USDC-bank state.
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

import { MARGINFI, MARGINFI_ACCOUNT_SEED, LIQUIDITY_VAULT_AUTH_SEED } from "./marginfi-addresses";

const USDC_MINT = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const dispatcherIdl = require("../../target/idl/dispatcher.json");
const marginfiIdl = require("../../target/idl/marginfi_usdc.json");

const DEPOSIT = 100_000_000n; // 100 USDC
const ro = (pubkey: PublicKey) => ({ pubkey, isSigner: false, isWritable: false });
const rw = (pubkey: PublicKey) => ({ pubkey, isSigner: false, isWritable: true });
const cu = ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 });
const u16le = (n: number) => Buffer.from([n & 0xff, (n >> 8) & 0xff]);

describe("marginfi-usdc · native mainnet-fork e2e", () => {
  const connection = new Connection("http://127.0.0.1:8899", "confirmed");
  const owner = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(`${__dirname}/fixtures/test-payer.json`, "utf8"))),
  );
  const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(owner), { commitment: "confirmed" });
  anchor.setProvider(provider);
  const dispatcher = new anchor.Program(dispatcherIdl, provider);
  const adapter = new anchor.Program(marginfiIdl, provider);
  const aid = adapter.programId;

  const [registry] = PublicKey.findProgramAddressSync([Buffer.from("registry")], dispatcher.programId);
  const [adapterEntry] = PublicKey.findProgramAddressSync([Buffer.from("adapter_entry"), aid.toBuffer()], dispatcher.programId);
  const [position] = PublicKey.findProgramAddressSync([Buffer.from("position"), owner.publicKey.toBuffer(), aid.toBuffer()], dispatcher.programId);
  const [adapterState] = PublicKey.findProgramAddressSync([Buffer.from("adapter_state"), aid.toBuffer(), position.toBuffer()], aid);
  const [marginfiAccount] = PublicKey.findProgramAddressSync(
    [Buffer.from(MARGINFI_ACCOUNT_SEED), MARGINFI.group.toBuffer(), position.toBuffer(), u16le(0), u16le(0)],
    MARGINFI.program,
  );
  const [vaultAuthority] = PublicKey.findProgramAddressSync(
    [Buffer.from(LIQUIDITY_VAULT_AUTH_SEED), MARGINFI.usdcBank.toBuffer()], MARGINFI.program,
  );
  const positionPool = getAssociatedTokenAddressSync(USDC_MINT, position, true);
  const ownerUsdcAta = getAssociatedTokenAddressSync(USDC_MINT, owner.publicKey);

  const fundMove = () => [
    ro(MARGINFI.program), ro(MARGINFI.group), rw(marginfiAccount), rw(MARGINFI.usdcBank),
    rw(MARGINFI.liquidityVault), ro(TOKEN_PROGRAM_ID),
  ];
  const bal = async (a: PublicKey): Promise<bigint> => {
    try { return (await getAccount(connection, a)).amount; } catch { return 0n; }
  };

  before(async () => {
    await connection.confirmTransaction(await connection.requestAirdrop(owner.publicKey, 100 * LAMPORTS_PER_SOL), "confirmed");
  });

  it("initializes registry + whitelists the adapter", async () => {
    // The registry is shared across adapters; another suite may have created it.
    try {
      await dispatcher.methods.initializeRegistry().accountsStrict({
        admin: owner.publicKey, registry, systemProgram: SystemProgram.programId,
      }).rpc();
    } catch (_) { /* already initialized */ }
    await dispatcher.methods.whitelistAdapter("MarginFi USDC", 1).accountsStrict({
      admin: owner.publicKey, registry, adapterEntry, adapterProgram: aid, systemProgram: SystemProgram.programId,
    }).rpc();
    expect((await (dispatcher.account as any).adapterEntry.fetch(adapterEntry)).isActive).to.equal(true);
  });

  it("opens a position (creates MarginFi account via CPI)", async () => {
    await dispatcher.methods.openPosition().accountsStrict({
      owner: owner.publicKey, adapterEntry, adapterProgram: aid, position,
      usdcMint: USDC_MINT, positionUsdcPool: positionPool, adapterState,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY, systemProgram: SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID, associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    }).remainingAccounts([
      ro(MARGINFI.program), ro(MARGINFI.group), rw(marginfiAccount), ro(MARGINFI.usdcBank),
    ]).preInstructions([cu]).rpc();
    expect((await (adapter.account as any).adapterState.fetch(adapterState)).bank.toBase58())
      .to.equal(MARGINFI.usdcBank.toBase58());
  });

  it("deposits 100 USDC into MarginFi", async () => {
    const before = await bal(ownerUsdcAta);
    await dispatcher.methods.deposit(new anchor.BN(DEPOSIT.toString())).accountsStrict({
      owner: owner.publicKey, adapterEntry, adapterProgram: aid, position,
      positionUsdcPool: positionPool, ownerUsdcAta, adapterState,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY, tokenProgram: TOKEN_PROGRAM_ID,
    }).remainingAccounts(fundMove()).preInstructions([cu]).rpc();
    expect(before - (await bal(ownerUsdcAta))).to.equal(DEPOSIT);
    expect((await (adapter.account as any).adapterState.fetch(adapterState)).header.totalShares.toNumber())
      .to.equal(Number(DEPOSIT));
    expect(Number(await bal(positionPool))).to.be.lessThan(10);
  });

  it("reports current value ≈ deposit (via return data)", async () => {
    const ix = await dispatcher.methods.currentValue().accountsStrict({
      adapterEntry, adapterProgram: aid, position, adapterState, instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
    }).remainingAccounts([ro(marginfiAccount), ro(MARGINFI.usdcBank)]).instruction();
    const tx = new Transaction().add(cu, ix);
    tx.feePayer = owner.publicKey;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    const sim = await connection.simulateTransaction(tx, [owner]);
    const rd = sim.value.returnData;
    expect(rd, `no return data; logs:\n${(sim.value.logs ?? []).join("\n")}`).to.not.be.null;
    const value = Buffer.from(rd!.data[0], "base64").readBigUInt64LE(0);
    expect(value > DEPOSIT - 1_000_000n && value < DEPOSIT + 1_000_000n, `value=${value}`).to.equal(true);
  });

  it("withdraws all back to USDC", async () => {
    const st = await (adapter.account as any).adapterState.fetch(adapterState);
    const before = await bal(ownerUsdcAta);
    await dispatcher.methods.withdraw(new anchor.BN(st.header.totalShares.toString())).accountsStrict({
      owner: owner.publicKey, adapterEntry, adapterProgram: aid, position,
      positionUsdcPool: positionPool, ownerUsdcAta, adapterState,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY, tokenProgram: TOKEN_PROGRAM_ID,
    }).remainingAccounts([
      ro(MARGINFI.program), ro(MARGINFI.group), rw(marginfiAccount), rw(MARGINFI.usdcBank),
      ro(vaultAuthority), rw(MARGINFI.liquidityVault), ro(TOKEN_PROGRAM_ID), ro(MARGINFI.oracle),
    ]).preInstructions([cu]).rpc();
    expect((await bal(ownerUsdcAta)) - before > DEPOSIT - 1_000_000n).to.equal(true);
    expect((await (adapter.account as any).adapterState.fetch(adapterState)).header.totalShares.toString()).to.equal("0");
  });
});
