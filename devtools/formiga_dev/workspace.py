"""Where Formiga lives on this machine, and how to run cargo against it.

Desktop is the repository this folder is in. Hill, Home and Farm are found beside it, under
either spelling a checkout gets ("Formiga-Hill" from git, "Formiga Hill" from GitHub Desktop), or
wherever FORMIGA_HILL_REPO / FORMIGA_HOME_REPO / FORMIGA_FARM_REPO point. When this folder is a git worktree
(under .claude/worktrees/, say), they are also looked for beside the main checkout it belongs to.

Hill, Home and Farm take Desktop's crates from a release tag. To try them against the Desktop in this
checkout, cargo is told to patch those crates to local paths for one run (`patched`); that rewrites
the expansion's Cargo.lock, so `lockfile_kept` puts the file back afterwards, and the patched build
gets a target folder of its own under .dev/ so the expansion's usual build is not thrown away.
"""

from __future__ import annotations

import contextlib
import os
import platform
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterator, List, Optional, Sequence, Tuple

DESKTOP = Path(__file__).resolve().parents[2]
DEV = DESKTOP / ".dev"
DESKTOP_GIT = "https://github.com/Von-Van/Formiga-Desktop"
# The Desktop crate that only builds on macOS and Windows.
APP_CRATE = "formiga-desktop"


def app_builds_here() -> bool:
    return sys.platform in ("darwin", "win32")


def desktop_version() -> str:
    text = (DESKTOP / "Cargo.toml").read_text(encoding="utf-8")
    section = text.split("[workspace.package]", 1)[-1]
    match = re.search(r'^version\s*=\s*"([^"]+)"', section, re.MULTILINE)
    return match.group(1) if match else "unknown"


@dataclass
class Expansion:
    """An expansion app that builds on Desktop's crates."""

    key: str  # "hill" | "home" | "farm"
    title: str  # "Formiga Hill"
    package: str  # its cargo package and binary
    data_env: str  # the variable that moves its data folder
    path: Optional[Path] = None

    def pins(self) -> Dict[str, str]:
        """Each Desktop crate it uses, and the tag it takes it from."""
        if not self.path:
            return {}
        pins = {}
        pattern = re.compile(
            r'^(formiga-[a-z-]+)\s*=\s*\{[^}]*git\s*=\s*"' + re.escape(DESKTOP_GIT)
            + r'"[^}]*tag\s*=\s*"([^"]+)"',
            re.MULTILINE,
        )
        for match in pattern.finditer((self.path / "Cargo.toml").read_text(encoding="utf-8")):
            pins[match.group(1)] = match.group(2)
        return pins


EXPANSIONS = {
    "hill": Expansion("hill", "Formiga Hill", "formiga-hill", "FORMIGA_HILL_DATA_DIR"),
    "home": Expansion("home", "Formiga Home", "formiga-home", "FORMIGA_HOME_DATA_DIR"),
    "farm": Expansion("farm", "Formiga Farm", "formiga-farm", "FORMIGA_FARM_DATA_DIR"),
}


def main_checkout() -> Optional[Path]:
    """The checkout this one is a git worktree of, or None when this is the main checkout."""
    try:
        common = subprocess.run(
            ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
            capture_output=True, text=True, cwd=DESKTOP,
        ).stdout.strip()
    except OSError:
        return None
    if not common:
        return None
    main = Path(common).resolve().parent
    return None if main == DESKTOP.resolve() else main


def beside() -> List[Path]:
    """The folders a sibling checkout is looked for in: beside this one, then beside the main
    checkout when this is a worktree of it."""
    folders = [DESKTOP.parent]
    main = main_checkout()
    if main and main.parent not in folders:
        folders.append(main.parent)
    return folders


def find_expansion(key: str) -> Expansion:
    expansion = EXPANSIONS[key]
    if expansion.path:
        return expansion
    override = os.environ.get(f"FORMIGA_{key.upper()}_REPO")
    title = expansion.title.split()[-1]
    candidates = (
        [Path(override)]
        if override
        else [folder / f"Formiga{gap}{title}" for folder in beside() for gap in ("-", " ")]
    )
    for candidate in candidates:
        manifest = candidate / "Cargo.toml"
        if manifest.is_file() and expansion.package in manifest.read_text(encoding="utf-8"):
            expansion.path = candidate.resolve()
            break
    return expansion


def patched(expansion: Expansion) -> List[str]:
    """cargo arguments that build `expansion` on this checkout's Desktop crates."""
    args = []
    for crate in expansion.pins():
        local = DESKTOP / "crates" / crate
        if local.is_dir():
            args += [
                "--config",
                f"patch.'{DESKTOP_GIT}'.{crate}.path='{local.as_posix()}'",
            ]
    return args


@contextlib.contextmanager
def lockfile_kept(repo: Path) -> Iterator[None]:
    lock = repo / "Cargo.lock"
    before = lock.read_bytes() if lock.exists() else None
    try:
        yield
    finally:
        if before is not None and (not lock.exists() or lock.read_bytes() != before):
            lock.write_bytes(before)


def patched_target(expansion: Expansion) -> Path:
    return DEV / "target" / f"{expansion.key}-on-local-desktop"


def windows_cross_check_available() -> Tuple[bool, str]:
    """Whether this non-Windows, non-Mac machine can lint the app as a Windows build."""
    if app_builds_here():
        return False, "the app builds natively here"
    rustup = shutil.which("rustup")
    if not rustup:
        return False, "rustup is not installed"
    installed = subprocess.run(
        [rustup, "target", "list", "--installed"], capture_output=True, text=True, cwd=DESKTOP
    ).stdout
    if "x86_64-pc-windows-gnu" not in installed:
        return False, "the x86_64-pc-windows-gnu target is not installed"
    if not shutil.which("x86_64-w64-mingw32-gcc"):
        return False, "the MinGW C compiler (x86_64-w64-mingw32-gcc) is not installed"
    return True, ""


@dataclass
class Run:
    code: int
    output: str
    seconds: float


def run(
    command: Sequence[str],
    cwd: Path = DESKTOP,
    env: Optional[Dict[str, str]] = None,
    quiet: bool = False,
) -> Run:
    """Run a command with its output and errors as one stream, in order, and keep all of it."""
    if not quiet:
        print(f"  … {' '.join(command[:4])}{' …' if len(command) > 4 else ''}", file=sys.stderr)
    started = time.monotonic()
    process = subprocess.run(
        list(command),
        cwd=cwd,
        env={**os.environ, **(env or {})},
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    return Run(process.returncode, process.stdout, time.monotonic() - started)


def relative(path: str, root: Path = DESKTOP) -> str:
    """A path as the repository names it, when it is inside `root`."""
    try:
        return Path(path).resolve().relative_to(root.resolve()).as_posix()
    except (ValueError, OSError):
        return path


def host() -> str:
    return f"{platform.system()} {platform.machine()}"
