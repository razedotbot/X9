# raze-trading (Rust)

Rust SDK for a self-hosted `raze-router` (default port 8082).

```toml
[dependencies]
raze-trading = { path = "sdks/rust" }
tokio = { version = "1", features = ["rt", "macros"] }
```

## Quick start

```rust
use raze_trading::{RazeTrading, BuyOpts};

let client = RazeTrading::builder("rzl_your_license_key")
    .base_url("http://localhost:8082")
    .build()?;

let health = client.health().await?;
let buy = client.buy(&BuyOpts {
    wallet_addresses: vec!["Wallet...".into()],
    token_address: "Mint...".into(),
    sol_amount: Some(0.1),
    ..Default::default()
}).await?;
```

See `examples/full_surface.rs` for the whole surface.

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
checks first. The router also reads `?apiKey=…` as a fallback, but a credential
in a query string ends up in the access log of every proxy in front of you.

`health` needs no credential. `quote` and `instructions` require one. `buy` and
`sell` will answer *without* one — but an unauthenticated call falls back to the
router's compiled public fee tier, whose recipient is not yours, so send the key
on every call.

## Multi-wallet responses

`transactions` is index-aligned with the `wallet_addresses` you sent. Wallets the
router could not build for come back as **empty strings**. Skip them; never
base64-decode an empty entry. All of this is HTTP 200 — check the field, not the
status code.

## Retry / errors

5xx and connection errors are retried with exponential backoff
(`builder().max_retries(n)`, default 2). `429` is **not** retried — it returns
`RazeTradingError::RateLimit { retry_after, message }`; honor `retry_after`
(seconds, from the `Retry-After` header) before retrying yourself.
