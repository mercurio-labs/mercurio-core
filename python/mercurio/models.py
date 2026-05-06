from __future__ import annotations

from dataclasses import dataclass
from typing import Any


JsonObject = dict[str, Any]


@dataclass(frozen=True)
class VersionInfo:
    service: str
    version: str
    api_version: int

    @classmethod
    def from_json(cls, data: JsonObject) -> "VersionInfo":
        return cls(
            service=str(data["service"]),
            version=str(data["version"]),
            api_version=int(data["apiVersion"]),
        )


@dataclass(frozen=True)
class BackendStartupInfo:
    url: str
    pid: int
    version: str
    api_version: int

    @classmethod
    def from_json(cls, data: JsonObject) -> "BackendStartupInfo":
        return cls(
            url=str(data["url"]),
            pid=int(data["pid"]),
            version=str(data["version"]),
            api_version=int(data["apiVersion"]),
        )


@dataclass(frozen=True)
class WorkspaceInfo:
    workspace_id: str
    workspace_root: str | None
    active_path: str | None
    project: JsonObject | None

    @classmethod
    def from_open_json(cls, data: JsonObject) -> "WorkspaceInfo":
        return cls(
            workspace_id=str(data["workspaceId"]),
            workspace_root=data.get("workspaceRoot"),
            active_path=data.get("activePath"),
            project=data.get("project"),
        )

    @classmethod
    def from_summary_json(cls, data: JsonObject) -> "WorkspaceInfo":
        return cls(
            workspace_id=str(data["workspaceId"]),
            workspace_root=data.get("workspaceRoot"),
            active_path=data.get("activePath"),
            project=data.get("project"),
        )


@dataclass(frozen=True)
class SemanticProjectResult:
    data: JsonObject

    @property
    def ok(self) -> bool:
        return bool(self.data.get("ok"))

    @property
    def file_count(self) -> int:
        return int(self.data.get("file_count", self.data.get("fileCount", 0)))

    @property
    def success_count(self) -> int:
        return int(self.data.get("success_count", self.data.get("successCount", 0)))

    @property
    def failure_count(self) -> int:
        return int(self.data.get("failure_count", self.data.get("failureCount", 0)))

    @property
    def results(self) -> list[JsonObject]:
        value = self.data.get("results", [])
        return value if isinstance(value, list) else []

    def __repr__(self) -> str:
        return (
            "SemanticProjectResult("
            f"ok={self.ok}, files={self.file_count}, "
            f"success={self.success_count}, failure={self.failure_count})"
        )
