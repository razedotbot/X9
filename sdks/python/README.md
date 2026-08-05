# raze-trading (Python)

Python SDK for a self-hosted `raze-router` (default port 8082). Sync **and**
async transports. Requires `httpx`.

## Quick start

```python
from raze_trading import RazeTrading

# async (default)
async with RazeTrading("rzl_your_license_key", base_url="http://localhost:8082") as raze:
    health = await raze.health()
    buy = await raze.buy({
        "walletAddresses": ["Wallet..."],
        "tokenAddress": "Mint...",
        "solAmount": 0.1,
    })

# sync
with RazeTrading("rzl_your_license_key", sync=True) as raze:
    health = raze.health()
```

See `examples/full_surface.py` for the whole surface.

## Surface

`health`, `quote`, `buy`, `sell`, `instructions` — the five routes a
self-hosted router serves. Perp and utility endpoints are not part of this
SDK: the router does not mount `/perp/*` in self-host mode, and `/utils/*`
belongs to a different service.

## Authentication

Your license key is the credential. The transport picks a header by prefix:

| Credential prefix | Sent as |
|---|---|
| `sk_…` / `eyJ…` | `Authorization: Bearer …` |
| anything else (including `rzl_…`) | `X-Api-Key` |

A license key therefore travels as `X-Api-Key`, which is the header the router
checks first.

`GET /health` needs no credential. `quote` and `instructions` require one.
`buy` and `sell` will answer *without* one — but an unauthenticated call falls
back to the router's compiled public fee tier, whose recipient is not yours, so
send the key on every call.

## Multi-wallet responses

`transactions` is index-aligned with the `walletAddresses` you sent. Wallets the
router could not build for come back as **empty strings** (and their entry in
`amountsOut` is `null`). Skip the empty ones; never base64-decode them. All of
this is HTTP 200 — check the field, not the status code.

## Lifecycle note

Use `async with` for async-mode clients. Using a sync `with` block on an
async-mode client emits a `RuntimeWarning` (the async `close()` cannot be
awaited from `__exit__`); call `await raze.close()` explicitly if you do not
use a context manager.

## Retry / errors

5xx and network errors are retried with exponential backoff (`max_retries`,
default 2). `429` raises `RateLimitError` (not retried) with `retry_after`
seconds (from the `Retry-After` header). Other errors: `AuthError` (401),
`ValidationError` (400/422), `ServerError` (5xx exhausted), `RazeError`.
