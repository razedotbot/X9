# raze-trading (Go)

Go SDK for a self-hosted `raze-router` (default port 8082). Standard library
only — no external dependencies.

```go
import razetrading "raze.bot/trading"
```

## Quick start

```go
client := razetrading.New("rzl_your_license_key",
    razetrading.WithBaseURL("http://localhost:8082"))

health, err := client.Health(ctx)

solAmount := 0.1
buy, err := client.Buy(ctx, razetrading.BuyOpts{
    WalletAddresses: []string{"Wallet..."},
    TokenAddress:    "Mint...",
    SolAmount:       &solAmount,
})
```

See `examples/fullsurface/main.go` for the whole surface.

## Surface

`Health`, `Quote`, `Buy`, `Sell`, `Instructions` — the five routes a
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

`Health` needs no credential. `Quote` and `Instructions` require one. `Buy` and
`Sell` will answer *without* one — but an unauthenticated call falls back to the
router's compiled public fee tier, whose recipient is not yours, so send the key
on every call.

## Multi-wallet responses

`Transactions` is index-aligned with the `WalletAddresses` you sent. Wallets the
router could not build for come back as **empty strings**. Skip them; never
base64-decode an empty entry. All of this is HTTP 200 — check the field, not the
status code.

## Retry / errors

5xx and network errors are retried with exponential backoff
(`WithMaxRetries(n)`, default 2). `429` returns `*RateLimitError` (not retried)
with `RetryAfter` seconds parsed from the `Retry-After` header. Type-check with
`IsAuthError`, `IsValidationError`, `IsRateLimitError`, `IsServerError`,
`IsRazeError`.
