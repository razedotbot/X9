# RAZEX9 — self-hosted Solana swap router

A single binary that quotes and builds Solana swap transactions across ~17 venues
(Pump.fun / PumpSwap, Raydium AMM V4 / CPMM / CLMM / Launchpad, Meteora DLMM /
DAMM v1+v2 / DBC, Orca Whirlpool, plus several dark-AMMs), routed through the
on-chain RAZEX9 CPI router. You run it on your own machine, against your own RPC
and your own Yellowstone gRPC endpoint.

It **builds** transactions. It never holds a private key and never sends
anything on-chain — you sign and submit.

## What is in this repo

| | |
|---|---|
| `raze-router` | the binary, plus `raze-router.service` and `selfhost.example.env` |
| [`program/`](program/) | source of the on-chain CPI router the binary calls (Apache-2.0). You do not deploy it — it is already live at `RAZEX9pxDuRCrtwR5wxUPAX3pWwAkBzvM8hF2fKaRE9`. Read it to see what your transactions actually execute. |
| [`sdks/`](sdks/) | Go, Python, Rust and TypeScript clients for the routes this binary serves |

The mainnet program was built from commit `01bbff2`; `program/` is ahead of it
by work that is not deployed yet (`route_unified`, and the wire types that go
with it). Everything the binary calls today is in both.

---

## 1. What you need before you start

| | |
|---|---|
| **A license key** from Raze (`rzl_…`) | issued manually; it is shown once at creation |
| **Your own Solana RPC endpoint** | a real paid one — see [Sizing your RPC](#sizing-your-rpc) |
| **Your own Yellowstone gRPC endpoint** | Helius, Triton, erpc, or your own Geyser plugin |
| **Linux x86-64**, glibc ≥ 2.34 | Debian 12+, Ubuntu 22.04+, RHEL 9+ — all fine |

You do **not** need to give us a wallet, and you do not configure one here.

**There is no payer keypair, and no environment variable for one.** This binary
is a transaction *builder*: `POST /swap/sol/buy`, `POST /swap/sol/sell` and
`POST /swap/sol/instructions` take a wallet **address** in the request body and
hand back **unsigned** V0 transactions with that address as fee payer. Signing
and sending are entirely on your side. You still need a funded wallet — it pays
for the swap, the network fees and any token-account rent — but you keep it, and
the server never sees its key.

*(The only private key this binary can load at all is an optional ALT signer,
which never signs a trade — see [Reusing Raze's lookup tables](#optional-reuse-razes-address-lookup-tables-read-only).)*

---

## 2. Quick start

```bash
# 1. verify the binary you downloaded (see checksums.txt)
sha256sum -c checksums.txt

# 2. install
sudo install -m 755 raze-router /usr/local/bin/raze-router
sudo mkdir -p /var/lib/raze-router && sudo chown $USER /var/lib/raze-router

# 3. configure
cp selfhost.example.env raze.env
$EDITOR raze.env          # license key, RPC_URL, GRPC_ENDPOINT

# 4. run
set -a && . ./raze.env && set +a
raze-router

# 5. first call (the license key is the credential)
curl -H "X-Api-Key: $RAZE_SELF_HOST_LICENSE_KEY" \
  'http://127.0.0.1:8082/swap/sol/quote/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v?amount=1000000000&slippageBps=500'
```

**The port stays closed for the first minute or so.** By design: the binary does
not bind until it has loaded a pool snapshot and built its first route index
(`RAZE_READY_BIND_TIMEOUT_SECS`, default 120 s). A connection refused right after
start is normal, not a fault. `RAZE_SERVE_BEFORE_READY=1` binds immediately if
you would rather see the port up.

A ready node answers:

```bash
curl -s http://127.0.0.1:8082/health
# {"success":true,"data":{"status":"ok","slot":362…,"accounts":280000,"feed_stale_secs":0}}
```

`accounts` climbing into the hundreds of thousands and `feed_stale_secs` at 0–3
means your gRPC feed is healthy. `accounts` stuck near 0 means it is not.

---

## 3. Configuration

Full annotated template: [`selfhost.example.env`](selfhost.example.env).

### Required

```bash
RAZE_SELF_HOST_LICENSE_KEY=rzl_...      # activates self-host mode; also your API credential
RPC_URL=https://your-rpc-endpoint       # YOUR RPC
GRPC_ENDPOINT=https://your-yellowstone  # YOUR Yellowstone gRPC
GRPC_TOKEN=...                          # only if your gRPC provider requires one
```

If `RPC_URL` or `GRPC_ENDPOINT` is missing or empty, the binary prints what is
missing and **exits 78** rather than booting into a router that answers
`/health` and knows nothing. This is a presence check, not a reachability check:
a wrong-but-present URL still starts.

`RAZE_SELF_HOST_LICENSE_KEY` is read **once at startup** — after a key change or
renewal, restart the process. Setting it to the empty string is the same as not
setting it: the binary reverts to fleet mode, with no license gate.

### Strongly recommended

```bash
RAZE_CACHE_DIR=/var/lib/raze-router     # absolute; MUST exist and be writable
BIND_ADDR=127.0.0.1:8082                # see "Network exposure" below
RAZE_TICKET_SECRET=<any random string>  # enables route tickets; stays on your box
```

### Optional

| variable | default | meaning |
|---|---|---|
| `RAZE_HUB_URL` | `https://hub.raze.bot` | where the license is verified. You do not need to set it. Empty string = detach, which fails **closed** (permanent 403). |
| `RAZE_SELF_HOST_VERIFY_INTERVAL_SECS` | `300` | license re-check interval. `0`/non-numeric ⇒ back to 300. Read once at startup. |
| `WRAPPER_API_KEY` | *(unset)* | your own API credential. If set, it becomes the **only** accepted credential and the license key stops working as a header. |
| `PROMETHEUS_PORT` | `9093` | metrics port, bound to `127.0.0.1` in self-host mode. |
| `SNAPSHOT_INTERVAL_SECS` | `60` | how often the pool snapshot is written to disk. |

**Variables that do nothing here**: `API_BACKEND_URL`, `STREAMING_HTTP_URL`,
`MIGRATION_WS_URL`, `INTERNAL_SERVICE_SECRET` — their consumers (auth-key sync,
usage emitter, buyback poller, migration WS, `/internal/*`) are not started in
self-host mode.

**Two you should leave alone**: `RAZE_SNAPSHOT_SEED_URL` (the boot seeder does
run here, and points at a fleet-only endpoint you cannot reach — it will block
startup up to 120 s before giving up), and `JWT_SECRET` (it makes the auth layer
accept any HS256 bearer signed with it — a second door you did not ask for).

---

## 4. Running it as a service

```ini
# /etc/systemd/system/raze-router.service
[Unit]
Description=RAZEX9 self-hosted swap router
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=raze
EnvironmentFile=/etc/raze/raze.env
WorkingDirectory=/var/lib/raze-router
ExecStart=/usr/local/bin/raze-router
Restart=on-failure
RestartSec=5
LimitNOFILE=65535
# The router keeps the live pool state in RAM. Size for your venue coverage;
# 8-16 GB is a realistic working set on mainnet.
MemoryHigh=12G
MemoryMax=16G

[Install]
WantedBy=multi-user.target
```

> **Set `RAZE_CACHE_DIR` to an absolute path.** Under systemd the default working
> directory is `/`, and without it the process tries to create `/cache` — which
> either fails with `EACCES` for an unprivileged `User=`, or (worse, because it
> goes unnoticed) succeeds and plants a root-owned `/cache`. If you use
> `ProtectSystem=`/`ReadOnlyPaths=`, add the directory to `ReadWritePaths=`.

---

## 5. The API

Everything is served on `BIND_ADDR` (default `0.0.0.0:8082`).

| Method | Path | Needs license | Needs credential |
|---|---|---|---|
| GET | `/swap/sol/quote/{mint}` | yes | **yes** |
| GET | `/swap/sol/quote/stream` (WebSocket) | yes | **yes** (also `?apiKey=`) |
| POST | `/swap/sol/instructions` | yes | **yes** |
| POST | `/swap/sol/buy-sell`, `/swap/sol/sell-buy` | yes | **yes** |
| POST | `/swap/sol/buy` | yes | **no** — falls back to the public fee tier ⚠️ |
| POST | `/swap/sol/sell` | yes | **no** — falls back to the public fee tier ⚠️ |
| GET | `/health`, `/ready` (+ `/swap/sol/` aliases) | no | no |

Not mounted in self-host mode: `/perp/*`, `/internal/*`, `/public/alt/known`,
and the product feed (WebSocket tape, execution lanes) — the latter is not even
compiled into this binary.

### Authentication

Your license key is the credential. Three transports, tried in this order:

```bash
# 1) X-Api-Key — checked first. Prefer this.
curl -H 'X-Api-Key: rzl_...' 'http://127.0.0.1:8082/swap/sol/quote/<mint>?amount=1000000000'

# 2) Authorization: Bearer
curl -H 'Authorization: Bearer rzl_...' 'http://127.0.0.1:8082/swap/sol/quote/<mint>?amount=1000000000'

# 3) ?apiKey= — a FALLBACK, read only when no Authorization header is present
curl 'http://127.0.0.1:8082/swap/sol/quote/<mint>?amount=1000000000&apiKey=rzl_...'
```

The query form exists for the WebSocket: browsers cannot set headers on a
WebSocket handshake.

```js
new WebSocket('ws://127.0.0.1:8082/swap/sol/quote/stream?apiKey=rzl_...')
```

Everywhere else use a header — a credential in a query string ends up in the
access log of every proxy in front of you. The value is compared byte-for-byte
and is **not** URL-decoded.

### Reading the response codes

Precedence: CORS/`OPTIONS` → license gate (403) → auth (401) → routing (404).

- `403 SELF_HOST_UNLICENSED` — the **process** is not licensed (not yet verified,
  revoked, or `RAZE_HUB_URL=""`). Every `/swap/*` path answers this, known or not.
- `401` — licensed, but your credential is missing (`AUTH_REQUIRED`) or rejected
  (`INVALID_KEY`).
- `404` — licensed *and* authenticated, path does not exist.

So a 401 or 403 tells you nothing about whether an endpoint exists, and
`OPTIONS` answers `200` on any path. Probe with an authenticated request.

### `GET /swap/sol/quote/{mint}` is exact-in only

It accepts `amount` (raw units; omitted ⇒ 1 SOL), `slippageBps` (default 500),
`inputMint`, `outputMint`, `maxHops` (1–3), `includeDex`, `excludeDex`, `wallet`
— and **silently ignores every other query parameter**, including `swapMode`.
For exact-out, use `POST /swap/sol/instructions` with `"swapMode": "exactOut"`.

### Multi-wallet responses: `success: true` is not per-wallet

`transactions[]` is index-aligned with the wallets you sent, and entries the
router could not build come back as **empty strings** — no route, build failure,
nothing to sell, an invalid pubkey in your list, or a transaction that exceeded
Solana's 1232-byte wire limit. `amountsOut[]` is `null` at exactly those indexes.
Skip empty entries; never base64-decode them. If *every* wallet fails, the
response flips to `success: false`. All of this is HTTP 200 — check the field,
not the status code.

---

## 6. Fees — read this before you go live

> ### ⚠️ An unauthenticated `/swap/sol/buy` or `/swap/sol/sell` builds transactions that pay **Raze**, not you.
>
> Those two routes are the only ones that answer without a credential. When they
> do, they fall back to the compiled **public fee tier**, which pins a hardcoded
> Raze wallet as the recipient of:
>
> - a **0.001 SOL tip** per built transaction (a plain SOL transfer out of the
>   trading wallet). `feeTipLamports` can only *raise* it, never lower it.
> - a **0.5% (50 bps) platform fee**, taken in-kind on-chain by the RAZEX9 router.
>
> Neither the address nor the rate is configurable — the recipient is a
> compile-time constant. On that tier `tipWallet`, `tipLamports`, `feeWallet`,
> `feeBps` and `feeOnInput` are parsed and then **silently ignored**.
>
> **Send your license key on every call — buy and sell included.** An
> authenticated request takes its fee settings from the request body, and a body
> with no `tipWallet`/`feeWallet` produces a transaction with **no tip and no
> percentage fee at all**.

### Charging your own fee

On authenticated calls, `tipWallet` / `tipLamports` / `feeWallet` / `feeBps` /
`feeOnInput` are honoured as given. Three things to know before you price a
business model on it:

1. **The percentage fee is charged on-chain, in kind, by the RAZEX9 router — and
   only on routes the CPI router actually builds.** A route that only the local
   assembler can build carries the tip alone, and your percentage fee is silently
   zero on it. Clamped to 1000 bps (10%).
2. **A SOL-denominated fee arrives as wSOL**, credited to your fee wallet's
   associated token account — nothing in the transaction unwraps it. (The one
   exception: a fee on the native side of a PumpFun bonding-curve trade is paid
   in native lamports.) The trader's wallet pays the ~0.00204 SOL rent to create
   your fee ATA the first time.
3. **`/swap/sol/buy-sell` and `/swap/sol/sell-buy` work differently**: there the
   fee is an off-chain native-SOL transfer sized on the combined notional, the
   10% clamp does not apply, and every transfer is floored up to 100,000 lamports
   per destination — so on small round-trips your wallet receives more than
   `feeBps`.

A malformed pubkey in any of those fields is discarded silently rather than
rejected — which disables that fee leg instead of returning an error. Check your
own input.

### What a built transaction costs regardless

The Solana signature fee, rent for any token account the build has to open
(~0.00204 SOL per new ATA, recoverable when closed), and a **compute budget**:
every transaction is prefixed with `SetComputeUnitLimit` + `SetComputeUnitPrice`
priced to target a 0.0001 SOL priority fee. That goes to the validator, not to
Raze. Override per request with `transactionsFeeLamports`; `0` removes both
instructions.

---

## 7. Operating it

### Network exposure

`BIND_ADDR` defaults to `0.0.0.0:8082` — **every** interface. That default is
fleet-shaped (our boxes are firewalled); on your machine it is a decision.

Anyone who can reach that port can make your node build buy/sell transactions
without knowing anything, because those two routes need no credential. The
transactions come back unsigned, so nobody can move your funds this way — what
is exposed is **your RPC bill, your licensed capacity, and fee revenue going to
Raze instead of you**.

Set `BIND_ADDR=127.0.0.1:8082` and put your own reverse proxy in front, or
firewall the port down to hosts you trust. There is no built-in rate limiter.

Metrics (`/metrics`, port 9093) are bound to `127.0.0.1` in self-host mode and
cannot be moved — they sit outside the license gate, so they are never put on
the network for you.

### What it writes to disk

All state lives under `RAZE_CACHE_DIR` (default `./cache`, **relative to the
working directory** — this binary has no absolute path compiled in).

| file | when | if the write fails |
|---|---|---|
| `pool_snapshot.bin` | every `SNAPSHOT_INTERVAL_SECS` (60 s) | `[persist] save failed` in the log; every restart is a cold start |
| `pool_snapshot.pmm` | every 60 s (dark-AMM templates) | templates lost on restart |
| `managed_alts*.json` | only with ALT peer sync | **silently** — no log at all |

Saves are write-temp-then-rename, so the process needs write permission on the
**directory**, not just the files. A missing or unwritable directory does not
stop the router: it just cold-starts forever, which with an undersized RPC shows
up as missing routes until the state is rebuilt.

### Health and readiness

- `/health` — unconditional 200 while the process lives. It does **not** reflect
  the license state.
- `/ready` — 200 only when snapshot loaded **and** first route index built **and**
  the Yellowstone feed is fresh; 503 otherwise.

Neither passes through the license gate, so **a monitor watching only health
stays green on a router whose trading routes are all 403.** Probe with a real
quote if you want to know that the product works.

### Sizing your RPC

The router does a large cold-hydration sweep at boot and keeps warming CLMM
tick-arrays and DLMM bin-arrays continuously — tens of thousands of accounts per
sweep via `getMultipleAccounts`. Measured against the free public endpoint:
`requested=46726 fetched=376`, i.e. ~99% rate-limited away.

Prices come from your gRPC feed, not from RPC — so an undersized RPC shows up as
**missing routes, not wrong prices**: concentrated-liquidity pools whose satellite
accounts never arrive are declined rather than mispriced. That is the safe
direction, but it costs you coverage. Use an endpoint with real
`getMultipleAccounts` throughput.

### The blockhash inside a returned transaction

Serialized transactions embed a `recentBlockhash` fetched from **your** RPC on a
2-second loop. Nothing validates it before serializing and `/ready` does not look
at it, so a healthy-looking node can hand you an unusable transaction:

- **RPC never answered since boot** → the cache still holds the all-zero hash;
  you get `200 success:true` and your submit fails with `Blockhash not found`.
  Reject any transaction whose `recentBlockhash` is all zeros.
- **RPC answered once then stopped** → the last good hash is served indefinitely;
  transactions land for ~60–90 s, then fail the same way with nothing else
  changing.

Only transport-level failures are logged; HTTP error pages and JSON-RPC error
bodies (401, 429) are discarded silently. Poll `getLatestBlockhash` against your
own RPC as the real signal. Sign and submit promptly — the hash is already up to
2 s old and expires in ~60–90 s.

---

## 8. The license gate

- **It starts closed.** Until the first successful verification, every `/swap/*`
  route answers `403 SELF_HOST_UNLICENSED`. In normal operation you never see
  this: the port does not open until after the index is built, by which time the
  license is verified.
- The binary calls `POST {RAZE_HUB_URL}/api/v1/licenses/verify` at boot, then
  every `RAZE_SELF_HOST_VERIFY_INTERVAL_SECS` (default 300 s), with a 5 s timeout.
- **A revocation** (hub answers 2xx with `valid:false`) closes trading routes
  within one interval.
- **An unreachable hub does not** — timeouts, 502s, malformed bodies are treated
  as transient and keep the previous state through 5 consecutive failures; the
  6th closes the gate anyway. At the default interval that is roughly **30
  minutes of autonomy**. Lowering the interval shortens that window.
- **Fail-closed is not terminal**: the loop keeps polling and routes reopen by
  themselves on the first good answer, no restart needed.
- There is no fast retry. If you start while the hub is unreachable, expect one
  full interval of 403 before trading routes open.
- The gate never gives you access to Raze's ALT authority or payer wallets.

Diagnose with the logs (`target=selfhost`): `hub verify call failed`,
`hub unreachable too long — failing closed`, `license verified — trading routes enabled`.

---

## 9. Optional: reuse Raze's Address Lookup Tables (read-only)

Your builds always use the two static lookup tables — nothing to configure. On
top of that, Raze maintains a rotating pool of tables that make transactions
smaller (more of them fit under the 1232-byte wire limit). You can consume that
pool read-only, without ever creating or paying for a table.

**Only do this if Raze gives you the URL of the box that IS the ALT source of
truth.** Use exactly that URL, never a geo-routed alias: reconciliation is
peer-authoritative, so a URL that lands on a different box between polls discards
everything it learned from the previous one.

```bash
RAZE_ALT_AUTOEXTEND=0                     # REQUIRED — absent means ON, not off
RAZE_ALT_SYNC_PEER_URL=<the exact URL Raze gives you>
RAZE_ALT_SYNC_INTERVAL_SECS=60            # default
```

`RAZE_ALT_AUTOEXTEND=0` is load-bearing, not a restatement of the default. Absent
or empty means **ON**. It is inert today only because you have no ALT signer —
but the moment `ALT_PRIVATE_KEY` or `ALT_KEYPAIR_PATH` is set, an unset
`RAZE_ALT_AUTOEXTEND` means the router starts creating and extending tables with
your SOL, *and* the read-only sync never starts at all.

The poller calls `{peer}/public/alt/known` with your license key as
`x-license-key`, then re-reads every newly learned table from **your own** RPC
before trusting it. It never asks you for a signer: creating, extending or
closing a lookup table needs its authority, but *referencing* one does not.

---

## 10. Verifying what you downloaded

```bash
sha256sum -c checksums.txt
```

The binary is built from a private source tree with:

- no default features (`--no-default-features`) — the product feed and all
  fleet-only paths are not compiled in;
- build paths remapped and the build-id stripped;
- a portability floor of **glibc 2.34**, so it runs on Debian 12+ / Ubuntu 22.04+
  / RHEL 9+ regardless of what it was built on;
- OpenSSL statically linked — no `libssl` needed on your machine;
- automated guards that fail the build if the artifact contains a local build
  path, a fleet IP or hostname, or anything shaped like a credential.

```bash
raze-router --version 2>/dev/null || true   # see VERSION for the release identity
ldd --version | head -1                     # your glibc must be >= 2.34
```

---

## 11. Support

Contact Raze with your license label. When reporting a problem, include:

- the output of `curl -s localhost:8082/health`,
- whether `/swap/sol/quote/<a known mint>` returns 200, 401 or 403,
- the last 50 log lines,
- your RPC and gRPC providers (not the keys).

If PumpSwap routes start failing on-chain with error `6053`, ask us for an
updated build: the live fee-recipient set is refreshed by a fleet-only poller,
and this binary carries a compiled fallback.
