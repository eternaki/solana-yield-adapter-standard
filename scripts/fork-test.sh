#!/usr/bin/env bash
# One-command Kamino mainnet-fork e2e.
#
# Spins up a local solana-test-validator that clones the real klend USDC-reserve
# account set from mainnet, injects the reserve + Scope price accounts with their
# cached slots rewritten low (so klend's `clock.slot - last_update` math is sane
# WITHOUT warping — warping to a ~423M mainnet slot exceeds macOS's open-file
# limit during accounts-hash), deploys the two programs, and runs the e2e.
#
# Usage:  MAINNET_RPC_URL=<rpc> ./scripts/fork-test.sh
# Default RPC is public mainnet-beta (rate-limited; a private RPC is faster).
set -euo pipefail
cd "$(dirname "$0")/.."

RPC="${MAINNET_RPC_URL:-https://api.mainnet-beta.solana.com}"
U="http://127.0.0.1:8899"
LEDGER="/tmp/yas-fork-ledger"

echo "▸ refreshing patched reserve/scope fixtures from mainnet"
MAINNET_RPC_URL="$RPC" node scripts/gen-fork-fixtures.js

echo "▸ launching local fork validator (no warp; slots patched low)"
pkill -f solana-test-validator 2>/dev/null || true
sleep 1
rm -rf "$LEDGER"
solana-test-validator --reset --quiet --ledger "$LEDGER" --url "$RPC" \
  --clone-upgradeable-program KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD \
  --clone 7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF \
  --clone Bgq7trRgVMeq33yt235zM2onQ4bRDBsY5EWiTetF4qw6 \
  --clone B8V6WVjPxW1UGwVDfxH2d2r8SyT4cqn7dQRK6XneVa7D \
  --clone 3DzjXRfxRm6iejfyyMynR4tScddaanrePJ1NJU2XnPPL \
  --clone EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v \
  --clone-upgradeable-program MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA \
  --clone 4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8 \
  --clone 2s37akK2eyBbp8DZgCm7RtsaEz8eJP3Nxd4urLHQv7yB \
  --clone 7jaiZR5Sk8hdYN9MxTpczTcwbWpb5WEoxSANuUwveuat \
  --clone-upgradeable-program PERPHjGBqRHArX4DySjwM6UJHiR3sWAatqfdBS2qQJu \
  --clone H4ND9aYttUVLFmNypZqLjZ52FYiGvdEB45GmwNoKEjTj \
  --clone 5BUwFW4nRbftYTDMbgxykoFWqWHPzahFSNAaaaJtVKsq \
  --clone G18jKKXQwBbrHeiK3C9MRXhkHsLHf7XgCSisykV46EZa \
  --clone WzWUoCmtVv7eqAbU3BfKPU3fhLP6CXR8NCJH78UK9VS \
  --clone 27G8MtK7VtTcCHkpASjSDdkWWYfoqT6ggEuKidVJidD4 \
  --clone 7xS2gz2bTp3fwCC7knJvUWTEU9Tycczu6VhJYKgi1wdz \
  --clone AQCGyheWPLeo6Qp9WpYS9m3Qj479t7R636N9ey1rEjEn \
  --clone 5Pv3gM9JrFFH883SWAhvJC9RPYmo8UNxuFtv5bMMALkm \
  --clone 4vkNeXiYEUizLdrpdPS1eC2mccyM4NUPRtERrk6ZETkk \
  --account FYq2BWQ1V5P1WFBqr3qB2Kb5yHVvSv7upzKodgQE5zXh tests/fork/fixtures/dovesag-0-patched.json \
  --account AFZnHPzy4mvVCffrVwhewHbFc93uTHvDSFrVH7GtfXF1 tests/fork/fixtures/dovesag-1-patched.json \
  --account hUqAT1KQ7eW1i6Csp9CXYtpPfSAvi835V7wKi5fRfmC tests/fork/fixtures/dovesag-2-patched.json \
  --account 6Jp2xZUTWdDD2ZyUPRzeMdc6AFQ5K3pFgZxk2EijfjnM tests/fork/fixtures/dovesag-3-patched.json \
  --account Fgc93D641F8N2d1xLjQ4jmShuD3GE3BsCXA56KBQbF5u tests/fork/fixtures/dovesag-4-patched.json \
  --account D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59 tests/fork/fixtures/reserve-patched.json \
  --account 3t4JZcueEzTbVP6kLxXrL3VpWx45jDer4eqysweBchNH tests/fork/fixtures/scope-patched.json \
  --account Dpw1EAVrSB1ibxiDQyTAW6Zip3J4Btk2x4SgApQCeFbX tests/fork/fixtures/oracle-patched.json \
  --account A28T5pKtscnhDo6C1Sz786Tup88aTjt8uyKewjVvPrGk tests/fork/fixtures/doves-patched.json \
  --clone-upgradeable-program whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc \
  --clone 6fteKNvMdv7tYmBoJHhj1jx6rHcEwC6RdSEmVpyS613J \
  --clone FM2RuqFYo9umA1yc5FyQn6pSDZJZ1MXAdaekJZ4dQCvi \
  --clone Fw6Xr45rBBrXbWJd5ZbSg44kacrKRLef4rHkZ8gWC5Ab \
  --clone 4yRC9NUHB2dwxfZyrqA8dDqH8GkcUVKU5F7W3ZPnbQtd \
  --clone AdLyWhs7xrwkBFCYEo3n9BiwgXMZzXMefh8K9wMWoy1j \
  --clone AofDEAkfQxcyeochNwxyQehYm6SpL3qrtxm7ZEZtPptp \
  --clone 9qUH5rp6Xw7NqghvbR9eQu6xTjEu5QTCHMbjdiiDVd5S \
  --clone BQ95wDV5A7z4c9cExYMWE2KvcqhbdjoxXcoQ88erFtyH \
  --clone AvZZF1YaZDziPY2RCK4oJrRVrbN3mTD9NL24hPeaZeUj \
  --clone-upgradeable-program dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH \
  --clone 5zpq7DvB6UdFFvpmBPspGPNfUGoBRRCE2HHg5u3gxcsN \
  --clone GXWqPpjQpdz7KZw9p7f5PX2eGxHAhvpNXiviFkAB8zXg \
  --clone 2CqkQvYxp9Mq4PqLvAQ1eryYxebUh4Liyn5YMDtXsYci \
  --account 6gMq3mRCKf8aP3ttTyYhuijVZ2LGi14oDsBbkgubfLB3 tests/fork/fixtures/drift-spotmarket-patched.json \
  --account 96mBCxzaYW4nwyXac5oLGV6m16aEmXbwWZ4XsTq4AJBT tests/fork/fixtures/usdc-funded.json &
VPID=$!
trap 'kill $VPID 2>/dev/null || true' EXIT

echo "▸ waiting for validator health"
for _ in $(seq 1 30); do
  if curl -s -m 3 "$U" -X POST -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' | grep -q '"result":"ok"'; then
    break
  fi
  sleep 2
done

echo "▸ funding + deploying programs"
solana airdrop 1000 --url "$U" >/dev/null 2>&1 || true
solana airdrop 1000 --url "$U" >/dev/null 2>&1 || true
solana program deploy --url "$U" --program-id target/deploy/dispatcher-keypair.json target/deploy/dispatcher.so
solana program deploy --url "$U" --program-id target/deploy/kamino_usdc-keypair.json target/deploy/kamino_usdc.so
solana program deploy --url "$U" --program-id target/deploy/marginfi_usdc-keypair.json target/deploy/marginfi_usdc.so
solana program deploy --url "$U" --program-id target/deploy/jupiter_lp-keypair.json target/deploy/jupiter_lp.so
solana program deploy --url "$U" --program-id target/deploy/maple_syrup-keypair.json target/deploy/maple_syrup.so
solana program deploy --url "$U" --program-id target/deploy/drift_if-keypair.json target/deploy/drift_if.so

echo "▸ running e2e (all adapters)"
yarn run ts-mocha -p ./tsconfig.json -t 1000000 'tests/fork/*-native.ts'
