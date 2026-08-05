from __future__ import annotations


class RazeError(Exception):
    """Base exception for all Raze SDK errors."""

    def __init__(self, message: str, status: int = 0, code: str | None = None, body: object = None):
        super().__init__(message)
        self.status = status
        self.code = code
        self.body = body


class AuthError(RazeError):
    """Raised on 401 Unauthorized responses."""

    def __init__(self, message: str = "Unauthorized", body: object = None):
        super().__init__(message, 401, "AUTH_ERROR", body)


class ValidationError(RazeError):
    """Raised on 400/422 validation failures."""

    def __init__(self, message: str = "Validation failed", status: int = 422, body: object = None):
        super().__init__(message, status, "VALIDATION_ERROR", body)


class RateLimitError(RazeError):
    """Raised on 429 Too Many Requests."""

    def __init__(self, message: str = "Rate limited", retry_after: float | None = None):
        super().__init__(message, 429, "RATE_LIMIT")
        self.retry_after = retry_after


class ServerError(RazeError):
    """Raised on 5xx server errors after retries are exhausted."""

    def __init__(self, message: str, status: int, body: object = None):
        super().__init__(message, status, "SERVER_ERROR", body)
