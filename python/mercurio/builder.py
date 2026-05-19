"""Native model-builder API.

This module intentionally hides the PyO3 extension module name from user scripts.
"""

try:
    from mercurio_core_native import ModelBuilder, WriteBackResult
except ImportError as error:  # pragma: no cover - depends on native wheel install
    raise ImportError(
        "Mercurio native builder is not installed. Install the PyO3 wheel or run "
        "`maturin develop -m crates/mercurio-python/Cargo.toml` in this environment."
    ) from error


__all__ = ["ModelBuilder", "WriteBackResult"]
