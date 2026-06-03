// Drift v2 mainnet addresses for the USDC insurance fund (market index 0).
import { PublicKey } from "@solana/web3.js";

export const DRIFT = {
  program: new PublicKey("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH"),
  state: new PublicKey("5zpq7DvB6UdFFvpmBPspGPNfUGoBRRCE2HHg5u3gxcsN"),
  driftSigner: new PublicKey("JCNCMFXo5M5qwUPg2Utu1u6YWp3MbygxqBsBeXXJfrw"),
  spotMarket: new PublicKey("6gMq3mRCKf8aP3ttTyYhuijVZ2LGi14oDsBbkgubfLB3"),
  spotMarketVault: new PublicKey("GXWqPpjQpdz7KZw9p7f5PX2eGxHAhvpNXiviFkAB8zXg"),
  insuranceFundVault: new PublicKey("2CqkQvYxp9Mq4PqLvAQ1eryYxebUh4Liyn5YMDtXsYci"),
} as const;
