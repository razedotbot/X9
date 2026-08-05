from __future__ import annotations

from typing import Any, Literal, TypedDict


Encoding = Literal["base58", "base64"]
SwapMode = Literal["exactIn", "exactOut"]


# ── Responses ───────────────────────────────────────────────────────────────


class HealthData(TypedDict):
    status: str
    slot: int
    accounts: int


class HealthResponse(TypedDict):
    success: bool
    data: HealthData


class QuoteData(TypedDict, total=False):
    mint: str
    avgPrice: float
    # Server emits integers (u64) for both of these.
    amountIn: int
    amountOut: int
    platform: str
    pool: str
    timeTaken: float


class QuoteResponse(TypedDict, total=False):
    success: bool
    data: QuoteData
    error: str


class SwapResponse(TypedDict, total=False):
    success: bool
    transactions: list[str]
    error: str


class InstructionsResponse(TypedDict, total=False):
    success: bool
    instructions: list[dict[str, Any]]
    addressLookupTableAddresses: list[str]
    error: str


# ── Request opts (spot swap) ────────────────────────────────────────────────


class BuyOpts(TypedDict, total=False):
    walletAddresses: list[str]
    tokenAddress: str
    solAmount: float
    # Per-wallet SOL amounts (alternative to solAmount). Length must match
    # walletAddresses. Server field `amounts`.
    amounts: list[float]
    slippageBps: int
    encoding: Encoding
    inputMint: str
    # Raw input amount in smallest units — REQUIRED when inputMint is set
    # (non-SOL input buys). Server field `inputAmountRaw`.
    inputAmountRaw: int
    tipWallet: str
    tipLamports: int
    feeWallet: str
    feeBps: int
    feeTipLamports: int
    # Priority fee per transaction in lamports (compute budget).
    transactionsFeeLamports: int


class SellOpts(TypedDict, total=False):
    walletAddresses: list[str]
    tokenAddress: str
    percentage: float
    tokensAmount: Any
    slippageBps: int
    encoding: Encoding
    outputMint: str
    tipWallet: str
    tipLamports: int
    feeWallet: str
    feeBps: int
    feeTipLamports: int
    transactionsFeeLamports: int


class InstructionsOpts(TypedDict, total=False):
    wallet: str
    inputMint: str
    outputMint: str
    amount: int
    slippageBps: int
    swapMode: SwapMode
    tipWallet: str
    tipLamports: int
    feeWallet: str
    feeBps: int
    feeTipLamports: int
    transactionsFeeLamports: int
