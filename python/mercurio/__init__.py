"""Python client for the Mercurio local HTTP backend."""

from .backend import Mercurio
from .client import MercurioClient
from .errors import MercurioBackendError, MercurioError, MercurioLaunchError
from .workspace import MercurioWorkspace

__all__ = [
    "Mercurio",
    "MercurioBackendError",
    "MercurioClient",
    "MercurioError",
    "MercurioLaunchError",
    "MercurioWorkspace",
]

__version__ = "0.1.0"
