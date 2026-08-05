from __future__ import annotations

import asyncio
import time
from typing import Any

import httpx

from .errors import AuthError, RateLimitError, RazeError, ServerError, ValidationError

_USER_AGENT = "raze-trading-py/0.3.0"


def _auth_headers(api_key: str) -> dict[str, str]:
    """Route a credential to the header the router reads for its kind.

    - ``sk_…`` / ``eyJ…`` -> ``Authorization: Bearer …``
    - anything else, a license key included -> ``X-Api-Key``

    ``X-Api-Key`` is the transport the router checks first.
    """
    if not api_key:
        return {}
    if api_key.startswith("sk_") or api_key.startswith("eyJ"):
        return {"authorization": f"Bearer {api_key}"}
    return {"x-api-key": api_key}


def _extract_error(body: Any) -> str | None:
    """Pull an error message from a parsed response body."""
    if isinstance(body, dict) and "error" in body:
        return str(body["error"])
    if isinstance(body, str) and len(body) < 500:
        return body
    return None


def _parse_body(resp: httpx.Response) -> Any:
    """Try to decode JSON; fall back to raw text."""
    try:
        return resp.json()
    except Exception:
        return resp.text


def _handle_error_status(resp: httpx.Response) -> None:
    """Raise immediately for auth and validation errors (no retry)."""
    if resp.status_code == 401:
        body = _parse_body(resp)
        raise AuthError(_extract_error(body) or "Unauthorized", body)
    if resp.status_code in (400, 422):
        body = _parse_body(resp)
        msg = _extract_error(body) or f"Validation error ({resp.status_code})"
        raise ValidationError(msg, resp.status_code, body)


def _should_retry(status: int) -> bool:
    """Only retry on server errors (5xx)."""
    return status >= 500


def _backoff_seconds(attempt: int) -> float:
    """Exponential back-off: 0.2s, 0.4s, 0.8s, ..."""
    return 0.2 * (2 ** attempt)


class SyncHttpClient:
    """Synchronous HTTP transport with retry and back-off."""

    def __init__(self, base_url: str, api_key: str, timeout: float, max_retries: int):
        self._base = base_url.rstrip("/")
        self._max_retries = max_retries
        headers: dict[str, str] = {
            "content-type": "application/json",
            "user-agent": _USER_AGENT,
            **_auth_headers(api_key),
        }
        self._client = httpx.Client(timeout=timeout, headers=headers)

    def get(self, path: str, params: dict[str, Any] | None = None) -> Any:
        cleaned = {k: v for k, v in (params or {}).items() if v is not None}
        resp = self._request("GET", path, params=cleaned or None)
        return resp.json()

    def post(self, path: str, body: Any = None) -> Any:
        resp = self._request("POST", path, json=body)
        return resp.json()

    def _request(self, method: str, path: str, **kwargs: Any) -> httpx.Response:
        url = f"{self._base}{path}"
        last_err: Exception | None = None

        for attempt in range(self._max_retries + 1):
            if attempt > 0:
                time.sleep(_backoff_seconds(attempt - 1))

            try:
                resp = self._client.request(method, url, **kwargs)
            except httpx.HTTPError as exc:
                last_err = exc
                continue

            _handle_error_status(resp)

            if resp.status_code == 429:
                raw = resp.headers.get("retry-after", "")
                try:
                    wait = float(raw) if float(raw) > 0 else 1.0 * (2 ** attempt)
                except (ValueError, TypeError):
                    wait = 1.0 * (2 ** attempt)
                raise RateLimitError("Rate limit exceeded", retry_after=wait)

            if _should_retry(resp.status_code):
                body = _parse_body(resp)
                last_err = ServerError(
                    _extract_error(body) or f"Server error ({resp.status_code})",
                    resp.status_code,
                    body,
                )
                continue

            return resp

        if last_err:
            raise last_err
        raise RazeError("Request failed")

    def close(self) -> None:
        self._client.close()


class AsyncHttpClient:
    """Asynchronous HTTP transport with retry and back-off."""

    def __init__(self, base_url: str, api_key: str, timeout: float, max_retries: int):
        self._base = base_url.rstrip("/")
        self._max_retries = max_retries
        headers: dict[str, str] = {
            "content-type": "application/json",
            "user-agent": _USER_AGENT,
            **_auth_headers(api_key),
        }
        self._client = httpx.AsyncClient(timeout=timeout, headers=headers)

    async def get(self, path: str, params: dict[str, Any] | None = None) -> Any:
        cleaned = {k: v for k, v in (params or {}).items() if v is not None}
        resp = await self._request("GET", path, params=cleaned or None)
        return resp.json()

    async def post(self, path: str, body: Any = None) -> Any:
        resp = await self._request("POST", path, json=body)
        return resp.json()

    async def _request(self, method: str, path: str, **kwargs: Any) -> httpx.Response:
        url = f"{self._base}{path}"
        last_err: Exception | None = None

        for attempt in range(self._max_retries + 1):
            if attempt > 0:
                await asyncio.sleep(_backoff_seconds(attempt - 1))

            try:
                resp = await self._client.request(method, url, **kwargs)
            except httpx.HTTPError as exc:
                last_err = exc
                continue

            _handle_error_status(resp)

            if resp.status_code == 429:
                raw = resp.headers.get("retry-after", "")
                try:
                    wait = float(raw) if float(raw) > 0 else 1.0 * (2 ** attempt)
                except (ValueError, TypeError):
                    wait = 1.0 * (2 ** attempt)
                raise RateLimitError("Rate limit exceeded", retry_after=wait)

            if _should_retry(resp.status_code):
                body = _parse_body(resp)
                last_err = ServerError(
                    _extract_error(body) or f"Server error ({resp.status_code})",
                    resp.status_code,
                    body,
                )
                continue

            return resp

        if last_err:
            raise last_err
        raise RazeError("Request failed")

    async def close(self) -> None:
        await self._client.aclose()
