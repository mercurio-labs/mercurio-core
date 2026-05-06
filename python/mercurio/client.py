from __future__ import annotations

import json
from typing import Any
from urllib.error import HTTPError
from urllib.parse import urlencode
from urllib.request import Request, urlopen

from .errors import MercurioBackendError
from .models import JsonObject, VersionInfo, WorkspaceInfo


class MercurioClient:
    """Low-level HTTP client for a Mercurio backend."""

    def __init__(self, base_url: str, *, timeout: float = 30.0):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout

    def health(self) -> JsonObject:
        return self.get("/api/health")

    def version(self) -> VersionInfo:
        return VersionInfo.from_json(self.get("/api/version"))

    def open_workspace(self, path: str, *, mode: str = "lazy") -> WorkspaceInfo:
        return WorkspaceInfo.from_open_json(
            self.post("/api/workspaces", {"path": path, "mode": mode})
        )

    def list_workspaces(self) -> list[WorkspaceInfo]:
        return [
            WorkspaceInfo.from_summary_json(item)
            for item in self.get("/api/workspaces")
        ]

    def delete_workspace(self, workspace_id: str) -> None:
        self.delete(f"/api/workspaces/{workspace_id}")

    def get(self, path: str, query: dict[str, Any] | None = None) -> Any:
        return self._request("GET", path, query=query)

    def post(
        self,
        path: str,
        payload: JsonObject | None = None,
        query: dict[str, Any] | None = None,
    ) -> Any:
        return self._request("POST", path, payload=payload, query=query)

    def put(
        self,
        path: str,
        payload: JsonObject | None = None,
        query: dict[str, Any] | None = None,
    ) -> Any:
        return self._request("PUT", path, payload=payload, query=query)

    def delete(self, path: str, query: dict[str, Any] | None = None) -> Any:
        return self._request("DELETE", path, query=query)

    def _request(
        self,
        method: str,
        path: str,
        *,
        payload: JsonObject | None = None,
        query: dict[str, Any] | None = None,
    ) -> Any:
        url = f"{self.base_url}{path}"
        if query:
            url = f"{url}?{urlencode(query)}"

        body = None
        headers = {"accept": "application/json"}
        if payload is not None:
            body = json.dumps(payload).encode("utf-8")
            headers["content-type"] = "application/json"

        request = Request(url, data=body, headers=headers, method=method)
        try:
            with urlopen(request, timeout=self.timeout) as response:
                raw = response.read()
                if not raw:
                    return None
                return json.loads(raw.decode("utf-8"))
        except HTTPError as error:
            message = error.read().decode("utf-8", errors="replace")
            raise MercurioBackendError(error.code, message) from error
