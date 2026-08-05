/**
 * Tour of the swap surface a self-hosted raze-router serves.
 * Nothing is submitted on-chain — the router returns unsigned transactions
 * that you sign and send yourself.
 *
 *   npx tsx examples/full-surface.ts
 */
import { RazeTrading } from "../src/index.js";

async function main() {
  // Your license key. It travels as X-Api-Key, which the router checks first.
  const client = new RazeTrading("rzl_your_license_key", {
    baseUrl: "http://localhost:8082",
  });

  const health = await client.health();
  console.log("health", health.data.status, "slot", health.data.slot);

  // Exact-in quote: 1 SOL into the mint, 0.5% slippage.
  const quote = await client.quote("TokenMintPubkey", {
    amount: 1_000_000_000,
    slippageBps: 50,
  });
  console.log("quote out", quote.data?.amountOut, "via", quote.data?.platform);

  // Buy. Send the key: an unauthenticated call falls back to the compiled
  // public fee tier, whose recipient is not yours.
  const buy = await client.buy({
    walletAddresses: ["YourWalletPubkey"],
    tokenAddress: "TokenMintPubkey",
    solAmount: 0.1,
  });
  // Entries are index-aligned with walletAddresses; a wallet the router could
  // not build for comes back as an empty string, so skip those.
  const built = (buy.transactions ?? []).filter((t) => t !== "").length;
  console.log("buy txs", built, "of", buy.transactions?.length ?? 0);

  // Sell the whole position.
  const sell = await client.sell({
    walletAddresses: ["YourWalletPubkey"],
    tokenAddress: "TokenMintPubkey",
    percentage: 100,
  });
  console.log("sell entries", sell.transactions?.length ?? 0);

  // Raw instructions, for callers assembling their own transaction.
  const ix = await client.instructions({
    walletAddress: "YourWalletPubkey",
    inputMint: "So11111111111111111111111111111111111111112",
    outputMint: "TokenMintPubkey",
    amount: 1_000_000_000,
    slippageBps: 50,
  });
  console.log("instructions ok", ix.success);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
