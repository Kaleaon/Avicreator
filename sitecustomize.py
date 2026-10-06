import sys
import os

# Ensure conftest mocks are loaded at interpreter startup so imports like
# `import bpy` work seamlessly during pytest collection or app initialization.
tests_dir = os.path.dirname(os.path.abspath(__file__))
conftest_path = os.path.join(tests_dir, "tests", "conftest.py")
if os.path.exists(conftest_path):
    import importlib.util
    spec = importlib.util.spec_from_file_location("conftest_bootstrap", conftest_path)
    if spec and spec.loader:
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
