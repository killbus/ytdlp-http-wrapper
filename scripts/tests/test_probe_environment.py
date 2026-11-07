"""Path-only checks of the embedded probe; no interpreter/venv provisioning."""

import ast
import unittest
from pathlib import Path, PurePosixPath, PureWindowsPath
from types import SimpleNamespace


PROBE = Path(__file__).resolve().parents[2] / "worker" / "probe_environment.py"
TREE = ast.parse(PROBE.read_text(encoding="utf-8"))
# Execute the actual argument and containment checks, stopping before imports
# that require an installed yt-dlp environment. Pure paths model both OSes.
START = next(i for i, node in enumerate(TREE.body) if isinstance(node, ast.Assign))
END = next(
    i for i, node in enumerate(TREE.body)
    if isinstance(node, ast.Import) and node.names[0].name == "yt_dlp"
)
CHECKS = compile(ast.Module(body=TREE.body[START:END], type_ignores=[]), str(PROBE), "exec")


class ProbePathTests(unittest.TestCase):
    def cases(self):
        for path_type, parent in ((PurePosixPath, "/downloads"), (PureWindowsPath, "C:/libs")):
            for name in ("ordinary", "staging"):
                install_root = path_type(parent) / name
                for relative in ("staging/runtime-candidate", "runtime"):
                    yield path_type, install_root, install_root / relative

    def probe(self, path_type, install_root, directory, base, base_prefix):
        scope = {
            "resolved": path_type,
            "sys": SimpleNamespace(
                argv=["-c", str(directory), "fixture-version", str(install_root)],
                prefix=str(directory / "python"),
                _base_executable=str(base),
                base_prefix=str(base_prefix),
            ),
            "platform": SimpleNamespace(python_implementation=lambda: "CPython"),
        }
        exec(CHECKS, scope)
        self.assertEqual(scope["install_root"], install_root)

    def test_candidate_and_published_accept_stable_private_or_external_base(self):
        for path_type, install_root, directory in self.cases():
            for prefix in (install_root / "uv-python" / "cpython", install_root.parent / "host"):
                with self.subTest(directory=str(directory), prefix=str(prefix)):
                    self.probe(path_type, install_root, directory, prefix / "python", prefix)

    def test_base_executable_in_replaceable_storage_is_rejected(self):
        for path_type, install_root, directory in self.cases():
            for name in ("runtime", "staging"):
                forbidden = install_root / name
                for base in (forbidden, forbidden / "python"):
                    with self.subTest(directory=str(directory), base=str(base)):
                        with self.assertRaisesRegex(
                            AssertionError, "base interpreter is inside replaceable storage"
                        ):
                            self.probe(
                                path_type, install_root, directory, base, install_root / "uv-python"
                            )

    def test_base_prefix_in_replaceable_storage_is_rejected(self):
        for path_type, install_root, directory in self.cases():
            for name in ("runtime", "staging"):
                forbidden = install_root / name
                for prefix in (forbidden, forbidden / "cpython"):
                    with self.subTest(directory=str(directory), prefix=str(prefix)):
                        with self.assertRaisesRegex(
                            AssertionError, "base prefix is inside replaceable storage"
                        ):
                            self.probe(
                                path_type, install_root, directory,
                                install_root / "uv-python/python", prefix,
                            )


if __name__ == "__main__":
    unittest.main()
