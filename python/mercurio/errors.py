class MercurioError(Exception):
    """Base exception for Mercurio Python client errors."""


class MercurioBackendError(MercurioError):
    """Raised when the Mercurio backend returns an error response."""

    def __init__(self, status: int, message: str):
        super().__init__(f"Mercurio backend returned HTTP {status}: {message}")
        self.status = status
        self.message = message


class MercurioLaunchError(MercurioError):
    """Raised when the Mercurio backend executable cannot be launched."""
