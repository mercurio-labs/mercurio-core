from __future__ import annotations

import atexit
import json
import os
import queue
import shutil
import subprocess
import threading
import time
from pathlib import Path

from .errors import MercurioLaunchError
from .models import BackendStartupInfo


def discover_executable(explicit: str | None = None) -> str:
    if explicit:
        return explicit

    env_value = os.environ.get("MERCURIO_EXE")
    if env_value:
        return env_value

    path_value = shutil.which("mercurio")
    if path_value:
        return path_value

    bundled = _bundled_executable()
    if bundled and bundled.exists():
        return str(bundled)

    raise MercurioLaunchError(
        "Could not find Mercurio executable. Pass executable=..., set "
        "MERCURIO_EXE, or put mercurio on PATH."
    )


class BackendProcess:
    def __init__(
        self,
        process: subprocess.Popen[str],
        startup: BackendStartupInfo,
    ):
        self.process = process
        self.startup = startup
        self._closed = False
        atexit.register(self.close)

    @property
    def url(self) -> str:
        return self.startup.url

    def close(self) -> None:
        if self._closed:
            return
        self._closed = True
        if self.process.poll() is not None:
            return
        self.process.terminate()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=5)


def launch_backend(
    *,
    executable: str | None = None,
    workspace: str | os.PathLike[str] | None = None,
    host: str = "127.0.0.1",
    port: int = 0,
    timeout: float = 15.0,
) -> BackendProcess:
    exe = discover_executable(executable)
    command = [exe, "server", "--host", host, "--port", str(port)]
    if workspace is not None:
        command.extend(["--workspace", str(workspace)])

    process = subprocess.Popen(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )

    output: queue.Queue[str] = queue.Queue(maxsize=1)

    def read_startup_line() -> None:
        if process.stdout is None:
            return
        line = process.stdout.readline()
        if line:
            output.put(line)

    reader = threading.Thread(target=read_startup_line, daemon=True)
    reader.start()

    line = ""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            stderr = process.stderr.read() if process.stderr else ""
            raise MercurioLaunchError(
                f"Mercurio backend exited before startup: {stderr.strip()}"
            )
        try:
            line = output.get(timeout=0.05)
            break
        except queue.Empty:
            continue

    if not line:
        process.terminate()
        raise MercurioLaunchError("Mercurio backend did not print startup JSON.")

    try:
        startup = BackendStartupInfo.from_json(json.loads(line))
    except (json.JSONDecodeError, KeyError, TypeError, ValueError) as error:
        process.terminate()
        raise MercurioLaunchError(
            f"Mercurio backend printed invalid startup JSON: {line.strip()}"
        ) from error

    return BackendProcess(process, startup)


def _bundled_executable() -> Path | None:
    suffix = ".exe" if os.name == "nt" else ""
    return Path(__file__).resolve().parent / "bin" / f"mercurio{suffix}"
