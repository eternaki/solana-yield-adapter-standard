// Generates patched mainnet account fixtures for the no-warp native fork test.
//
// klend's refresh does `clock.slot - reserve.last_update.slot`, which underflows
// on a non-warped fork (validator starts near slot 0 while the cloned reserve
// carries a ~423M mainnet slot). Warping to the mainnet slot blows past macOS's
// kern.maxfilesperproc during accounts-hash. So instead we clone the reserve and
// the Scope price account and rewrite their cached slots to a low value — the
// validator reaches it within a few slots and klend's math stays sane.
//
//   MAINNET_RPC_URL=... node scripts/gen-fork-fixtures.js
//
// Writes tests/fork/fixtures/{reserve,scope}-patched.json (solana --account format).
const { Connection, PublicKey } = require("@solana/web3.js");
const fs = require("fs");

const RESERVE = new PublicKey("D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59");
const SCOPE = new PublicKey("3t4JZcueEzTbVP6kLxXrL3VpWx45jDer4eqysweBchNH");
// Shared USDC Pyth pull oracle (PriceUpdateV2) — used by both the MarginFi bank
// and the Jupiter custody. posted_slot @125, publish_time @93, prev_publish @101.
const PYTH_ORACLE = new PublicKey("Dpw1EAVrSB1ibxiDQyTAW6Zip3J4Btk2x4SgApQCeFbX");
const PYTH_POSTED_SLOT_OFF = 125;
const PYTH_PUBLISH_TIME_OFF = 93;
const PYTH_PREV_PUBLISH_OFF = 101;
// Jupiter's "doves" oracle for USDC; publish timestamp (i64) at byte 82.
const DOVES_ORACLE = new PublicKey("A28T5pKtscnhDo6C1Sz786Tup88aTjt8uyKewjVvPrGk");
const DOVES_TS_OFF = 82;
// Far-future publish time so the validator's wall clock is always within max-age
// (Jupiter's USDC custody allows only 5s). Pyth `get_price_no_older_than` uses a
// saturating sub, so a future timestamp reads as fresh (age 0), never rejected.
const FAR_FUTURE = 4_000_000_000n; // ~year 2096
const RPC = process.env.MAINNET_RPC_URL ?? "https://api.mainnet-beta.solana.com";

const LOW_SLOT = 1n;
const SCOPE_HEADER = 40; // 8 disc + 32 oracle_mappings
const SCOPE_ENTRY = 56; // DatedPrice: price(16) + last_updated_slot(8) + unix_ts(8) + generic(24)
const SCOPE_ENTRIES = 512;
const SLOT_OFF_IN_ENTRY = 16; // after the 16-byte Price

function toFixture(pubkey, info) {
  return {
    pubkey: pubkey.toBase58(),
    account: {
      lamports: info.lamports,
      data: [info.data.toString("base64"), "base64"],
      owner: info.owner.toBase58(),
      executable: info.executable,
      rentEpoch: 0,
    },
  };
}

(async () => {
  const c = new Connection(RPC, "confirmed");
  // Drift USDC spot market — its insurance_fund.unstaking_period (i64 @384) is the
  // unstake cooldown; patch to 0 so request_remove + remove settle in one tx on the fork.
  const DRIFT_SPOT_MARKET = new PublicKey("6gMq3mRCKf8aP3ttTyYhuijVZ2LGi14oDsBbkgubfLB3");
  const SM_UNSTAKING_PERIOD_OFF = 384;

  const [reserve, scope, oracle, doves, spotMarket] = await c.getMultipleAccountsInfo([
    RESERVE, SCOPE, PYTH_ORACLE, DOVES_ORACLE, DRIFT_SPOT_MARKET,
  ]);

  // Reserve: last_update.slot at byte 16.
  reserve.data.writeBigUInt64LE(LOW_SLOT, 16);

  // Scope: rewrite last_updated_slot of every price entry.
  for (let i = 0; i < SCOPE_ENTRIES; i++) {
    const off = SCOPE_HEADER + i * SCOPE_ENTRY + SLOT_OFF_IN_ENTRY;
    if (off + 8 <= scope.data.length) scope.data.writeBigUInt64LE(LOW_SLOT, off);
  }

  // Pyth oracle: posted_slot low + publish_time far-future (fresh for both the
  // MarginFi health pulse and Jupiter's 5s custody max-age).
  oracle.data.writeBigUInt64LE(LOW_SLOT, PYTH_POSTED_SLOT_OFF);
  oracle.data.writeBigInt64LE(FAR_FUTURE, PYTH_PUBLISH_TIME_OFF);
  oracle.data.writeBigInt64LE(FAR_FUTURE, PYTH_PREV_PUBLISH_OFF);
  // Doves oracle (USDC): publish timestamp far-future.
  doves.data.writeBigInt64LE(FAR_FUTURE, DOVES_TS_OFF);
  // Drift: zero the IF unstake cooldown so request_remove + remove settle in one tx.
  spotMarket.data.writeBigInt64LE(0n, SM_UNSTAKING_PERIOD_OFF);

  const dir = `${__dirname}/../tests/fork/fixtures`;
  fs.writeFileSync(`${dir}/drift-spotmarket-patched.json`, JSON.stringify(toFixture(DRIFT_SPOT_MARKET, spotMarket), null, 2));

  // Jupiter values AUM with each custody's "doves aggregator" oracle (custody
  // field @384), a different feed (DoVEsk-owned, len 394, timestamp @177) than
  // the per-custody doves@320. Patch all five (SOL/ETH/BTC/USDC/USDT order).
  const DOVES_AG = [
    "FYq2BWQ1V5P1WFBqr3qB2Kb5yHVvSv7upzKodgQE5zXh",
    "AFZnHPzy4mvVCffrVwhewHbFc93uTHvDSFrVH7GtfXF1",
    "hUqAT1KQ7eW1i6Csp9CXYtpPfSAvi835V7wKi5fRfmC",
    "6Jp2xZUTWdDD2ZyUPRzeMdc6AFQ5K3pFgZxk2EijfjnM",
    "Fgc93D641F8N2d1xLjQ4jmShuD3GE3BsCXA56KBQbF5u",
  ];
  const DOVES_AG_TS_OFF = 177;
  const keys = DOVES_AG.map((k) => new PublicKey(k));
  const accs = await c.getMultipleAccountsInfo(keys);
  accs.forEach((a, i) => {
    a.data.writeBigInt64LE(FAR_FUTURE, DOVES_AG_TS_OFF);
    fs.writeFileSync(`${dir}/dovesag-${i}-patched.json`, JSON.stringify(toFixture(keys[i], a), null, 2));
  });

  fs.writeFileSync(`${dir}/reserve-patched.json`, JSON.stringify(toFixture(RESERVE, reserve), null, 2));
  fs.writeFileSync(`${dir}/scope-patched.json`, JSON.stringify(toFixture(SCOPE, scope), null, 2));
  fs.writeFileSync(`${dir}/oracle-patched.json`, JSON.stringify(toFixture(PYTH_ORACLE, oracle), null, 2));
  fs.writeFileSync(`${dir}/doves-patched.json`, JSON.stringify(toFixture(DOVES_ORACLE, doves), null, 2));
  console.log("wrote reserve/scope/oracle/doves patched fixtures");
})().catch((e) => {
  console.error(e.message);
  process.exit(1);
});
