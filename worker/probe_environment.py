"""Read-only boot probe, embedded by Rust; not part of the worker protocol."""

import hashlib
import importlib.metadata
import json
import platform
import runpy
import site
import sys
from pathlib import Path


def resolved(path):
    # Rust canonical paths can use Windows' extended prefix while sys.prefix
    # uses a DOS path. Compare the same spelling without changing the target.
    text = str(Path(path).resolve())
    if text.startswith("\\\\?\\UNC\\"):
        text = "\\\\" + text[8:]
    elif text.startswith("\\\\?\\"):
        text = text[4:]
    return Path(text)


root = resolved(sys.argv[1])
expected = sys.argv[2]
install_root = resolved(sys.argv[3])
venv = root / "python"
assert platform.python_implementation() == "CPython", "worker requires CPython"
assert resolved(sys.prefix) == venv, "venv prefix did not survive publication"
base = resolved(sys._base_executable)
base_prefix = resolved(sys.base_prefix)
# The base must survive retirement of runtime/ and all staging candidates.
for forbidden in (install_root / "runtime", install_root / "staging"):
    assert not base.is_relative_to(forbidden), "base interpreter is inside replaceable storage"
    assert not base_prefix.is_relative_to(forbidden), "base prefix is inside replaceable storage"

import yt_dlp
from yt_dlp.version import __version__

assert __version__ == expected, "yt_dlp module version does not match explicit pin"
assert resolved(yt_dlp.__file__).is_relative_to(venv), "yt_dlp escapes venv"
# PEP 440 normalization belongs to uv; distribution versions can omit date zeros.
distribution = importlib.metadata.distribution("yt-dlp")
assert distribution.version, "missing yt-dlp distribution version"

import yt_dlp_ejs
from yt_dlp_ejs.yt import solver

assert importlib.metadata.version("yt-dlp-ejs"), "missing EJS distribution version"
for module in (yt_dlp_ejs, solver):
    assert resolved(module.__file__).is_relative_to(venv), "EJS escapes venv"
for name in ("lib", "core"):
    bundle = getattr(solver, name)()
    assert isinstance(bundle, str) and bundle.strip(), f"missing or empty EJS {name} bundle"

runpy.run_path(str(root / "worker" / "ytdlp_worker.py"), run_name="runtime_probe")

packages = sorted(
    ({"name": item.metadata["Name"], "version": item.version}
     for item in importlib.metadata.distributions()),
    key=lambda item: (item["name"].lower(), item["version"]),
)
site_packages = sorted(
    resolved(path).relative_to(root).as_posix()
    for path in site.getsitepackages()
    if Path(path).is_dir()
)
print(json.dumps({
    "implementation": platform.python_implementation(),
    "python_version": platform.python_version(),
    "base_executable": str(base),
    "base_prefix": str(base_prefix),
    "interpreter_sha256": hashlib.sha256(base.read_bytes()).hexdigest(),
    "yt_dlp_module_version": __version__,
    "yt_dlp_distribution_version": distribution.version,
    "packages": packages,
    "site_packages": site_packages,
}, sort_keys=True))
