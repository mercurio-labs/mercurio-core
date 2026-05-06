from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path

from mercurio import Mercurio


class BackendIntegrationTests(unittest.TestCase):
    def test_launch_open_workspace_and_compile_preview(self) -> None:
        executable = os.environ.get("MERCURIO_EXE")
        if not executable:
            self.skipTest("MERCURIO_EXE is not set")

        with tempfile.TemporaryDirectory(prefix="mercurio_python_") as root:
            root_path = Path(root)
            model_path = root_path / "model.sysml"
            model_path.write_text("package Demo {\n}\n", encoding="utf-8")

            with Mercurio.launch(executable=executable, workspace=str(root_path)) as backend:
                workspace = backend.open_workspace(str(root_path))
                result = workspace.compile_project_preview(
                    staged_files={
                        "model.sysml": "package Demo {\n  part def Vehicle;\n}\n"
                    }
                )

            self.assertTrue(result.ok)
            self.assertEqual(
                model_path.read_text(encoding="utf-8"),
                "package Demo {\n}\n",
            )


if __name__ == "__main__":
    unittest.main()
