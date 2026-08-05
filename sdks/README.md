# SDKs

Four clients for the router you are running — Go, Python, Rust and TypeScript.
All four cover the same five routes and default to `http://localhost:8082`, so
out of the box they talk to your own instance and nowhere else.

| | package | install |
|---|---|---|
| **Go** | `raze.bot/trading` | `go get raze.bot/trading` (or a local `replace`) |
| **Python** | `raze_trading` | `pip install ./python` — needs `httpx` |
| **Rust** | `raze-trading` | path dependency on `sdks/rust` |
| **TypeScript** | `@raze/trading` | `npm install ./ts` — ESM, Node ≥ 18, no runtime deps |

## What they cover

`health`, `quote`, `buy`, `sell`, `instructions`.

That is the whole surface a self-hosted router serves, so it is the whole
surface these clients expose. Two things you may have seen elsewhere are
deliberately absent:

- **Perps.** The router does not mount `/perp/*` outside our fleet, so those
  calls would 404 against your instance.
- **Utilities** (transfer, burn, launch, fee claim, consolidate). Those live in
  a different service, not in this binary — they would 404 against any router.

Three routes the router *does* serve are not wrapped yet: `/swap/sol/buy-sell`,
`/swap/sol/sell-buy` and the `/swap/sol/quote/stream` WebSocket. Call them
directly over HTTP until they land here.

## The credential

Your license key. Every client sends it as `X-Api-Key`, the header the router
checks first.

`health` needs no credential, and `buy`/`sell` will answer without one — but an
unauthenticated buy or sell falls back to the router's compiled public fee tier,
whose recipient is not yours. Send the key on every call.

## The one response shape worth knowing

`buy` and `sell` take a list of wallets and return a list of transactions
**index-aligned with it**. A wallet the router could not build for comes back as
an **empty string**, not an error, and the response is still HTTP 200 with
`success: true` — `success` only goes false when *every* wallet failed. Skip the
empty entries and never try to decode them.

Nothing here signs or sends anything: you get unsigned transactions back and
submit them yourself.
