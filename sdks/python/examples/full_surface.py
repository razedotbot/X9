"""Tour of the swap surface a self-hosted raze-router serves (async).

Nothing is submitted on-chain — the router returns unsigned transactions that
you sign and send yourself.

    python examples/full_surface.py
"""
import asyncio

from raze_trading import RazeTrading


async def main() -> None:
    # Your license key. It travels as X-Api-Key, which the router checks first.
    async with RazeTrading("rzl_example_key", base_url="http://localhost:8082") as raze:
        health = await raze.health()
        print("health", health["data"]["status"], "slot", health["data"]["slot"])

        # Exact-in quote: 1 SOL into the mint, 0.5% slippage.
        quote = await raze.quote(
            "TokenMintPubkey",
            amount=1_000_000_000,
            slippage_bps=50,
        )
        print("quote out", quote.get("data", {}).get("outAmount"))

        # Buy. Send the key: an unauthenticated call falls back to the compiled
        # public fee tier, whose recipient is not yours.
        buy = await raze.buy({
            "walletAddresses": ["YourWalletPubkey"],
            "tokenAddress": "TokenMintPubkey",
            "solAmount": 0.1,
        })
        # Entries are index-aligned with walletAddresses; a wallet the router
        # could not build for comes back as an empty string, so skip those.
        txs = [t for t in buy.get("transactions", []) if t]
        print("buy txs", len(txs))

        # Sell 100% of the position.
        sell = await raze.sell({
            "walletAddresses": ["YourWalletPubkey"],
            "tokenAddress": "TokenMintPubkey",
            "percentage": 100,
        })
        print("sell txs", len([t for t in sell.get("transactions", []) if t]))

        # Raw instructions, for callers assembling their own transaction.
        ix = await raze.instructions({
            "walletAddress": "YourWalletPubkey",
            "inputMint": "So11111111111111111111111111111111111111112",
            "outputMint": "TokenMintPubkey",
            "amount": 1_000_000_000,
            "slippageBps": 50,
        })
        print("instructions ok", ix.get("success"))


if __name__ == "__main__":
    asyncio.run(main())
