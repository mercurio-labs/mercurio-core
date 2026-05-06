from __future__ import annotations

import json
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any
from urllib.parse import parse_qs, urlparse

from mercurio import Mercurio, MercurioBackendError


class FakeMercurioHandler(BaseHTTPRequestHandler):
    workspaces: dict[str, dict[str, Any]] = {}
    requests: list[dict[str, Any]] = []
    next_workspace = 1

    def do_GET(self) -> None:
        parsed = urlparse(self.path)
        if parsed.path == "/api/version":
            self.write_json(
                {"service": "mercurio-core", "version": "0.1.0", "apiVersion": 1}
            )
            return
        if parsed.path == "/api/health":
            self.write_json(
                {"service": "mercurio-core", "version": "0.1.0", "status": "ok"}
            )
            return
        if parsed.path == "/api/workspaces":
            self.write_json(list(self.workspaces.values()))
            return
        if parsed.path.endswith("/graph"):
            self.requests.append({"method": "GET", "path": parsed.path})
            self.write_json({"nodes": [], "edges": []})
            return
        if parsed.path.endswith("/editor/file"):
            query = parse_qs(parsed.query)
            self.write_json({"path": query["path"][0], "content": "package Demo {}"})
            return
        self.write_error(404, "not found")

    def do_POST(self) -> None:
        parsed = urlparse(self.path)
        payload = self.read_json()
        self.requests.append({"method": "POST", "path": parsed.path, "json": payload})
        if parsed.path == "/api/workspaces":
            workspace_id = f"ws_{self.next_workspace:016x}"
            type(self).next_workspace += 1
            data = {
                "workspaceId": workspace_id,
                "workspaceRoot": payload["path"],
                "activePath": None,
                "project": {},
            }
            self.workspaces[workspace_id] = data
            self.write_json(data)
            return
        if parsed.path.endswith("/semantic/project/compile"):
            self.write_json(
                {
                    "ok": True,
                    "project_path": payload["project_path"],
                    "file_count": 1,
                    "success_count": 1,
                    "failure_count": 0,
                    "results": [{"path": "model.sysml", "ok": True}],
                }
            )
            return
        if parsed.path.endswith("/editor/parse"):
            self.write_json({"ok": True, "diagnostics": [], "element_count": 1})
            return
        self.write_error(404, "not found")

    def do_PUT(self) -> None:
        parsed = urlparse(self.path)
        self.requests.append(
            {"method": "PUT", "path": parsed.path, "json": self.read_json()}
        )
        self.send_response(204)
        self.end_headers()

    def do_DELETE(self) -> None:
        parsed = urlparse(self.path)
        workspace_id = parsed.path.rsplit("/", 1)[-1]
        self.workspaces.pop(workspace_id, None)
        self.send_response(204)
        self.end_headers()

    def log_message(self, format: str, *args: Any) -> None:
        return

    def read_json(self) -> dict[str, Any]:
        length = int(self.headers.get("content-length", "0"))
        if length == 0:
            return {}
        return json.loads(self.rfile.read(length).decode("utf-8"))

    def write_json(self, payload: Any) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def write_error(self, status: int, message: str) -> None:
        body = message.encode("utf-8")
        self.send_response(status)
        self.send_header("content-type", "text/plain")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


class ClientTests(unittest.TestCase):
    def setUp(self) -> None:
        FakeMercurioHandler.workspaces = {}
        FakeMercurioHandler.requests = []
        FakeMercurioHandler.next_workspace = 1
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), FakeMercurioHandler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        host, port = self.server.server_address
        self.backend = Mercurio.connect(f"http://{host}:{port}")

    def tearDown(self) -> None:
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)

    def test_version_handshake_and_workspace_open(self) -> None:
        version = self.backend.version()
        self.assertEqual(version.api_version, 1)
        workspace = self.backend.open_workspace("C:/models/demo")
        self.assertEqual(workspace.workspace_id, "ws_0000000000000001")

    def test_compile_project_preview_shapes_staged_files(self) -> None:
        workspace = self.backend.open_workspace("C:/models/demo")
        result = workspace.compile_project_preview(
            staged_files={"model.sysml": "package Demo {}"}
        )
        self.assertTrue(result.ok)
        self.assertEqual(result.file_count, 1)
        request = FakeMercurioHandler.requests[-1]
        self.assertEqual(
            request["path"],
            "/api/workspaces/ws_0000000000000001/semantic/project/compile",
        )
        self.assertEqual(
            request["json"]["staged_files"],
            [{"path": "model.sysml", "content": "package Demo {}"}],
        )

    def test_save_file_uses_workspace_scoped_put(self) -> None:
        workspace = self.backend.open_workspace("C:/models/demo")
        workspace.save_file("model.sysml", "package Demo {}")
        request = FakeMercurioHandler.requests[-1]
        self.assertEqual(request["method"], "PUT")
        self.assertEqual(
            request["path"],
            "/api/workspaces/ws_0000000000000001/editor/file",
        )

    def test_backend_errors_are_mapped(self) -> None:
        with self.assertRaises(MercurioBackendError) as context:
            self.backend.client.get("/api/missing")
        self.assertEqual(context.exception.status, 404)


if __name__ == "__main__":
    unittest.main()
