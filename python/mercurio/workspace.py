from __future__ import annotations

from .client import MercurioClient
from .models import JsonObject, SemanticProjectResult, WorkspaceInfo


class MercurioWorkspace:
    """Workspace-scoped convenience API."""

    def __init__(self, client: MercurioClient, info: WorkspaceInfo):
        self.client = client
        self.info = info

    @property
    def workspace_id(self) -> str:
        return self.info.workspace_id

    def model(self) -> JsonObject:
        return self.client.get(self._path("/model"))

    def graph(self, *, scope: str | None = None) -> JsonObject:
        query = {"scope": scope} if scope else None
        return self.client.get(self._path("/graph"), query=query)

    def element(self, element_id: str) -> JsonObject:
        return self.client.get(self._path(f"/elements/{element_id}"))

    def search(self, query_text: str) -> list[JsonObject]:
        return self.client.get(self._path("/search"), query={"q": query_text})

    def files(self) -> JsonObject:
        return self.client.get(self._path("/editor/files"))

    def read_file(self, path: str) -> JsonObject:
        return self.client.get(self._path("/editor/file"), query={"path": path})

    def save_file(self, path: str, content: str) -> None:
        self.client.put(
            self._path("/editor/file"),
            {"content": content},
            query={"path": path},
        )

    def parse_preview(self, path: str, content: str) -> JsonObject:
        return self.client.post(
            self._path("/editor/parse"),
            {"path": path, "content": content},
        )

    def compile_file_preview(self, path: str, content: str) -> JsonObject:
        return self.client.post(
            self._path("/editor/semantic-compile"),
            {"path": path, "content": content},
        )

    def lint_preview(self, path: str, content: str) -> JsonObject:
        return self.client.post(
            self._path("/editor/lint"),
            {"path": path, "content": content},
        )

    def format_preview(self, path: str, content: str) -> JsonObject:
        return self.client.post(
            self._path("/editor/format"),
            {"path": path, "content": content},
        )

    def refresh(self, path: str) -> JsonObject:
        return self.client.post(self._path("/editor/refresh"), {"path": path})

    def workspace_session(self) -> JsonObject:
        return self.client.get(self._path("/semantic/workspace-session"))

    def compile_project(
        self,
        project_path: str = ".",
        *,
        staged_files: dict[str, str] | None = None,
    ) -> SemanticProjectResult:
        return self.compile_project_preview(project_path, staged_files=staged_files)

    def compile_project_preview(
        self,
        project_path: str = ".",
        *,
        staged_files: dict[str, str] | None = None,
    ) -> SemanticProjectResult:
        payload = {
            "project_path": project_path,
            "staged_files": self._staged_files(staged_files),
        }
        return SemanticProjectResult(
            self.client.post(self._path("/semantic/project/compile"), payload)
        )

    def lint_project_preview(
        self,
        project_path: str = ".",
        *,
        staged_files: dict[str, str] | None = None,
    ) -> JsonObject:
        payload = {
            "project_path": project_path,
            "staged_files": self._staged_files(staged_files),
        }
        return self.client.post(self._path("/semantic/project/lint"), payload)

    def close(self) -> None:
        self.client.delete_workspace(self.workspace_id)

    def _path(self, suffix: str) -> str:
        return f"/api/workspaces/{self.workspace_id}{suffix}"

    @staticmethod
    def _staged_files(staged_files: dict[str, str] | None) -> list[JsonObject]:
        if not staged_files:
            return []
        return [
            {"path": path, "content": content}
            for path, content in staged_files.items()
        ]
