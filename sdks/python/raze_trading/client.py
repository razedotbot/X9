from __future__ import annotations

import warnings
from typing import Any

from .http import AsyncHttpClient, SyncHttpClient
from .types import (
    BuyOpts,
    HealthResponse,
    InstructionsOpts,
    InstructionsResponse,
    QuoteResponse,
    SellOpts,
    SwapResponse,
)


class RazeTrading:
    """Client for a self-hosted raze-router.

    Operates in either sync or async mode depending on the ``sync`` flag.
    In async mode (the default) every public method returns an awaitable.

    The credential is your license key. It is sent as ``X-Api-Key``, which is
    the transport the router checks first.

    Usage (sync)::

        with RazeTrading("rzl_xxx", sync=True) as raze:
            health = raze.health()
            quote = raze.quote("So11...mint", slippage_bps=50, amount=1000000)

    Usage (async)::

        async with RazeTrading("rzl_xxx") as raze:
            health = await raze.health()
            txns = await raze.buy({
                "walletAddresses": ["addr1"],
                "tokenAddress": "mint...",
                "solAmount": 0.1,
            })
    """

    def __init__(
        self,
        api_key: str = "",
        *,
        base_url: str = "http://localhost:8082",
        timeout: float = 30,
        max_retries: int = 2,
        sync: bool = False,
    ):
        self._sync = sync

        if sync:
            self._http: SyncHttpClient | AsyncHttpClient = SyncHttpClient(
                base_url, api_key, timeout, max_retries,
            )
        else:
            self._http = AsyncHttpClient(base_url, api_key, timeout, max_retries)

    def health(self) -> HealthResponse | Any:
        """GET /health (unauthenticated)."""
        return self._http.get("/health")

    def quote(
        self,
        mint: str,
        *,
        input_mint: str | None = None,
        slippage_bps: int | None = None,
        amount: int | None = None,
    ) -> QuoteResponse | Any:
        """GET /swap/sol/quote/:mint (requires auth)."""
        params: dict[str, Any] = {}
        if input_mint is not None:
            params["inputMint"] = input_mint
        if slippage_bps is not None:
            params["slippageBps"] = slippage_bps
        if amount is not None:
            params["amount"] = amount
        return self._http.get(f"/swap/sol/quote/{mint}", params or None)

    def buy(self, opts: BuyOpts) -> SwapResponse | Any:
        """POST /swap/sol/buy.

        Answers without a credential, but an unauthenticated call falls back to
        the compiled public fee tier — send your key.
        """
        return self._http.post("/swap/sol/buy", dict(opts))

    def sell(self, opts: SellOpts) -> SwapResponse | Any:
        """POST /swap/sol/sell.

        Same fee-tier caveat as :meth:`buy` — send your key.
        """
        return self._http.post("/swap/sol/sell", dict(opts))

    def instructions(self, opts: InstructionsOpts) -> InstructionsResponse | Any:
        """POST /swap/sol/instructions (requires auth)."""
        return self._http.post("/swap/sol/instructions", dict(opts))

    def close(self) -> None:
        """Close the underlying HTTP transport.

        In async mode this returns the ``aclose()`` coroutine — ``await`` it,
        or prefer ``async with``.
        """
        if self._sync:
            self._http.close()  # type: ignore[union-attr]
        else:
            return self._http.close()  # type: ignore[union-attr,return-value]

    async def __aenter__(self) -> RazeTrading:
        return self

    async def __aexit__(self, *exc: object) -> None:
        await self._http.close()  # type: ignore[misc]

    def __enter__(self) -> RazeTrading:
        return self

    def __exit__(self, *exc: object) -> None:
        # Sync `with` on a sync client: close normally. On an async client the
        # close() is a coroutine — calling it synchronously here would leak the
        # client and emit "coroutine was never awaited". Warn instead of
        # silently leaking; the caller should use `async with`.
        if self._sync:
            self._http.close()  # type: ignore[union-attr]
        else:
            warnings.warn(
                "RazeTrading was created in async mode but used with a sync "
                "`with` block; the HTTP client was not closed. Use "
                "`async with RazeTrading(...)` instead.",
                RuntimeWarning,
                stacklevel=2,
            )
