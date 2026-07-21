#!/usr/bin/env python3
"""gdio local installer builds debug and installs.

Usage:
    python local_install.py
"""

import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path

APP = "gdio"


def get_install_dir() -> Path:
    if "GDIO_INSTALL_DIR" in os.environ:
        return Path(os.environ["GDIO_INSTALL_DIR"])
    if platform.system() == "Windows":
        return Path(os.environ.get("LOCALAPPDATA", "")) / "gdio" / "bin"
    return Path.home() / ".gdio" / "bin"


def main() -> None:
    install_dir = get_install_dir()
    binary_name = f"{APP}.exe" if platform.system() == "Windows" else APP

    # Check for cargo
    if shutil.which("cargo") is None:
        print("[X] cargo is required but not found. Install Rust: https://rustup.rs", file=sys.stderr)
        sys.exit(1)

    # Build
    print(f"[OK] Building {APP} (debug)...")
    result = subprocess.run(["cargo", "build"])
    if result.returncode != 0:
        print("[X] Build failed", file=sys.stderr)
        sys.exit(1)

    # Find binary
    binary = Path("target") / "debug" / binary_name
    if not binary.exists():
        print(f"[X] Binary not found at {binary}", file=sys.stderr)
        sys.exit(1)

    # Install
    install_dir.mkdir(parents=True, exist_ok=True)
    dest = install_dir / binary_name
    shutil.copy2(binary, dest)

    print(f"[OK] Installed {APP} to {dest}")

    # Add to PATH
    path_str = os.environ.get("PATH", "")
    if str(install_dir) not in path_str.split(os.pathsep):
        print(f"  Manually add to your PATH:")
        print(f"  export PATH=\"{install_dir}:$PATH\"")
    else:
        print("[OK] Already in PATH")

    print(f"\n[OK] Done! Run '{APP} --help' to get started.")


if __name__ == "__main__":
    main()
