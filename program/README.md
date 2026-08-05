# raze-router — the on-chain half of the swap router

```
RAZEX9pxDuRCrtwR5wxUPAX3pWwAkBzvM8hF2fKaRE9
```

This is the program the `raze-router` binary calls when it builds a swap. The
binary quotes and assembles routes off-chain; this program executes them on
chain, in one transaction, under one uniform envelope.

It holds **no per-venue knowledge**. Pool layouts and instruction encodings live
in the client, which hands over each hop as a fully-formed CPI
`(program_id, accounts, data)`. Adding or updating a venue is therefore an
off-chain change plus an `add_venue` allowlist entry — **never a redeploy**.

---

## 1. If you run the router binary, there is nothing to do

The program is already deployed at the address above and the router already
uses it. You do not deploy it, fund it, or name it in your config — its id is a
compile-time constant in the binary, not an environment variable.

One flag is worth knowing:

| variable | default | effect |
|---|---|---|
| `CPI_PROGRAM` | **on** | route swaps through this program. Absent means on — only `0`/`false`/`no`/`off` disables it. |

With `CPI_PROGRAM=0` the router falls back to its local assembler for every
swap: routes still build, but each hop settles independently instead of
chaining on measured output, so multi-hop quotes get looser.

### How a swap picks its path

The router tries every candidate plan through this program first. Only if no
plan can be expressed or fit in a transaction does it retry the same plans with
the local assembler. If neither pass produces a transaction the swap is a miss —
there is no third-party aggregator behind it.

A route falls out of the CPI pass mainly on **size**: concentrated-liquidity
venues (CLMM, DLMM, Whirlpool) carry tick/bin arrays that move with price, so
they cannot live in a lookup table and are inlined at 32 bytes each. Two such
legs in one route overflow the 1232-byte transaction limit.

---

## 2. Where your fee settings end up

The `feeWallet` / `feeBps` / `feeOnInput` you send to the router become
`RouteArgsV2.fee_bps` + `fee_on_input` and the `platform_fee_account` account on
this instruction. The program charges the fee in kind, on chain, and:

- caps it at **1000 bps (10%)** — `FeeTooHigh` above that;
- takes it from the input or the output according to `fee_on_input`;
- pays it to whatever account you named — there is no pinned recipient;
- enforces `min_return` on the **net**, after the fee.

A fee only exists on hops this program builds. A route that fell back to the
local assembler carries no percentage fee at all.

---

## 3. Reading a failed transaction

Failures surface as `custom program error: 0x…`. Anchor numbers them from
**6000** (`0x1770`), in declaration order. The ones you are most likely to meet:

| code | hex | meaning |
|---|---|---|
| 6000 | `0x1770` | `Paused` — swaps are halted by the admin; admin instructions still work |
| 6005 | `0x1775` | `MinReturnNotReached` — the net output missed your floor. Slippage, or a quote built on stale state |
| 6008 | `0x1778` | `VenueNotAllowed` — that venue program is not in the allowlist |
| 6013 | `0x177d` | `InvalidHopAccounts` — a hop's source and destination are the same account |
| 6014 | `0x177e` | `UnexpectedSaTokenAccount` — the venue CPI listed a protected account that is not this hop's source or destination |
| 6015 | `0x177f` | `InvalidActualAmountIn` — the venue consumed less than 90%, or more than it was given |
| 6018 | `0x1782` | `FeeTooHigh` — `fee_bps` over the 10% cap |
| 6031 | `0x178f` | `MaxAmountInExceeded` — exact-out spent more than `max_amount_in` |
| 6032 | `0x1790` | `ExactOutNotReached` — exact-out produced less than `amount_out` |
| 6033 | `0x1791` | `InvalidSwapAuthority` — the passed authority is not `SA_POOL[sa_pool_index(payer)]` |

The full list is `src/error.rs`, in order. Note that a **custom error `101`** is
not from this program: it is Anchor's "instruction not found", meaning the
caller sent a discriminator this deployment does not implement.

---

## 4. Instructions

| instruction | what it does |
|---|---|
| `route_universal(RouteArgsV2)` | exact-in swap: deposit → hops → fee → `min_return` → withdraw |
| `route_exact_out(RouteArgsExactOut)` | single-hop exact-out: the venue produces `amount_out`, input capped at `max_amount_in`, remainder refunded |
| `route_unified(RouteArgsUnified)` | both directions behind one discriminator, chosen by `mode` (0 = exact-in, 1 = exact-out) |
| `initialize_config(fee_authority, fee_bps)` | create the singleton config; the caller becomes admin |
| `add_venue` / `remove_venue(program_id)` | edit the venue allowlist (admin) |
| `set_fee` / `set_paused` / `set_admin` / `set_fee_authority` | admin |

`set_fee` and `set_fee_authority` write config fields that routes do not read —
live routes take the fee and its destination per call.

---

## 5. Writing your own client

The program is half a route; the client owns everything venue-specific.

1. **Encode the venue instruction yourself** and pass it as `HopMeta.data`. Set
   `amount_in_offset` to the byte offset of the `u64` input amount inside it, or
   `-1` to pass the data verbatim. The program splices the *measured* output of
   the previous hop there, which is what makes chaining exact.
2. **List every account the venue needs** in `remaining_accounts`, in the
   venue's own order, and index into it from `HopMetaV2.accounts`.
3. **Derive the swap authority the way the program does**:
   `SA_POOL[sa_pool_index(payer)]`. Anything else is rejected with 6033. The
   shared test vector is in `src/constants.rs`.
4. **Create the token accounts first**, one per distinct mint, in the same
   transaction — the program does not create them. Routes whose boundary is the
   user's own account skip this.
5. **Budget the transaction before building.** The args envelope is **31 bytes**
   (so an encoded instruction is `39 + Σ hops` with the discriminator) and each
   hop is `23 + 2·A + D`, where `A` is the number of account metas and `D` the
   venue data length. If you mirror the wire types by hand, mirror
   `wire_envelope_and_hop_payload_are_pinned` too — a hand-written mirror shares
   no compiler with this crate, so only matching assertions catch a drift.

The wire types live in `src/shared.rs`; their borsh layout is the contract.

---

## 6. Deploying your own instance

Possible, with one caveat worth reading first: **the distributed `raze-router`
binary will not use it.** The program id is compiled in, so your deployment is
only reachable from a client you write yourself.

1. **Generate a program keypair and point `declare_id!` at it.** The
   `SA_POOL`, `SA_WSOL_ATA` and `CONFIG_ADDRESS` tables in `src/constants.rs`
   are PDAs derived under that id, so they must be regenerated too. The
   `pda_tables_match_derivation` test fails loudly if you forget.
2. **Build and deploy:**
   ```
   cargo-build-sbf --manifest-path Cargo.toml
   solana program deploy target/deploy/raze_router.so --program-id <your-keypair.json>
   ```
3. **Call `initialize_config` in the same slot as the deploy.** It has static
   seeds and the first caller becomes admin. Prefer `fee_bps = 0`: routes take
   their fee per call, so the config value is inert.
4. **Allowlist your venues** with `add_venue`, one program id at a time. An
   allowlisted program is invoked with the authority's signature, so list only
   programs you have vetted. The token, system and ATA programs and the router
   itself are refused outright.

---

## 7. Build and test

A flat crate — no Anchor `programs/` nesting. Anchor 0.32.1 is a dependency but
the build is plain `cargo-build-sbf`; `Anchor.toml` carries only the program ids
and the forked-validator clone list.

```
cargo-build-sbf --manifest-path Cargo.toml   # build the .so FIRST
cargo test                                   # unit + in-SVM
```

⚠️ Build before you test. The in-SVM tests load the prebuilt `.so` from
`target/deploy`, so `cargo test` alone will happily exercise a stale binary and
still pass.

`keys/` is gitignored except `PROGRAM_ID.txt`; no keypair is, or has ever been,
committed here.

## Status

This program has not been reviewed by a third-party firm. It moves real funds on
mainnet — form your own view before pointing anything you care about at it.

## License

[Apache-2.0](LICENSE). See [NOTICE](NOTICE). Contributions are accepted under the
same license (Apache-2.0 §5) — no separate CLA.
