# @raze/trading (TypeScript)

TypeScript SDK for a self-hosted `raze-router` (default port 8082). ESM,
Node ≥ 18, zero runtime dependencies (uses global `fetch`).

## Quick start

```ts
import { RazeTrading } from "@raze/trading";

const client = new RazeTrading("rzl_your_license_key", {
  baseUrl: "http://localhost:8082",
});

const health = await client.health();
const buy = await client.buy({
  walletAddresses: ["Wallet..."],
  tokenAddress: "Mint...",
  solAmount: 0.1,
});
```

See `examples/full-surface.ts` for the whole surface.

## Surface

`health`, `quote`, `buy`, `sell`, `instructions` — the five routes a
self-hosted router serves. Perp and utility endpoints are not part of this SDK:
the router does not mount `/perp/*` in self-host mode, and `/utils/*` belongs to
a different service.

## Authentication

Your license key is the credential. The transport picks a header by prefix:

| Credential prefix | Sent as |
|---|---|
| `sk_…` / `eyJ…` | `Authorization: Bearer …` |
| anything else (including `rzl_…`) | `X-Api-Key` |

A license key therefore travels as `X-Api-Key`, which is the header the router
checks first.

`health` needs no credential. `quote` and `instructions` require one. `buy` and
`sell` will answer *without* one — but an unauthenticated call falls back to the
router's compiled public fee tier, whose recipient is not yours, so send the key
on every call.

## Multi-wallet responses

`transactions` is index-aligned with the `walletAddresses` you sent. Wallets the
router could not build for come back as **empty strings**. Skip them; never
base64-decode an empty entry. All of this is HTTP 200 — check the field, not the
status code.

## Retry / errors

5xx and network errors are retried with exponential backoff (`maxRetries`,
default 2). `429` throws `RateLimitError` (not retried) carrying `retryAfter`
(seconds, from the `Retry-After` header) and the parsed body; honor it before
retrying. Other errors: `AuthError` (401), `ValidationError` (400/422),
`ServerError` (5xx exhausted), `RazeError` (other).
