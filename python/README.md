# Mercurio Python SDK

This package is a thin Python client for the Mercurio local HTTP backend.

```python
from mercurio import Mercurio

with Mercurio.launch() as backend:
    workspace = backend.open_workspace("C:/models/demo")
    result = workspace.compile_project()
    graph = workspace.graph()
```

Attach to an already-running backend:

```python
from mercurio import Mercurio

backend = Mercurio.connect("http://127.0.0.1:49152")
workspace = backend.open_workspace("C:/models/demo")
```

The first release expects a Mercurio executable installed separately. Discovery order:

1. Explicit `executable=` argument.
2. `MERCURIO_EXE` environment variable.
3. `mercurio` on `PATH`.
4. Future bundled executable in the Python wheel.
