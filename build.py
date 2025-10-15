#!/usr/bin/env python3

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Dict, Any, List, Optional, Tuple


def run(
    cmd: List[str], *, check=True, env=None, capture=False, cwd=None
) -> subprocess.CompletedProcess:
    print("> " + " ".join(cmd))
    return subprocess.run(
        cmd, check=check, env=env, cwd=cwd, text=True, capture_output=capture
    )


def have(cmd: str) -> bool:
    return shutil.which(cmd) is not None


def ensure_dir(p: Path):
    p.mkdir(parents=True, exist_ok=True)


def host_os() -> str:
    s = platform.system().lower()
    if "windows" in s:
        return "windows"
    if "linux" in s:
        return "linux"
    raise RuntimeError(f"unsupported host OS: {s}")


def target_triple(os_: str, arch: int, windows_gnu: bool = False) -> str:
    if os_ == "windows":
        return "i686-pc-windows-msvc" if arch == 32 else "x86_64-pc-windows-msvc"
    if windows_gnu:
        return "i686-pc-windows-gnu" if arch == 32 else "x86_64-pc-windows-gnu"
    return "i686-unknown-linux-gnu" if arch == 32 else "x86_64-unknown-linux-gnu"


def load_toml(path: Path) -> Dict[str, Any]:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def normalize_crate_name(name: str) -> str:
    return name.replace("-", "_")


def is_workspace_root(manifest: Dict[str, Any]) -> bool:
    return "workspace" in manifest and "package" not in manifest


def infer_lib_name_from_manifest(manifest: Dict[str, Any]) -> Optional[str]:
    if isinstance(manifest.get("lib"), dict):
        nm = manifest["lib"].get("name")
        if nm:
            return normalize_crate_name(nm)
    if isinstance(manifest.get("package"), dict):
        nm = manifest["package"].get("name")
        if nm:
            return normalize_crate_name(nm)
    return None


def cargo_metadata(cwd: Path) -> Dict[str, Any]:
    if not have("cargo"):
        raise RuntimeError("cargo not found in PATH.")
    res = run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        capture=True,
        cwd=str(cwd),
    )
    return json.loads(res.stdout)


def pick_workspace_cdylib(metadata: Dict[str, Any], cwd: Path) -> Tuple[Path, str]:
    candidates: List[Tuple[Path, str]] = []
    for pkg in metadata.get("packages", []):
        for tgt in pkg.get("targets", []):
            if "cdylib" in tgt.get("kind", []):
                lib = normalize_crate_name(tgt.get("name") or pkg.get("name"))
                pkg_dir = Path(pkg["manifest_path"]).parent.resolve()
                candidates.append((pkg_dir, lib))
                break
    if not candidates:
        raise RuntimeError(
            'No cdylib targets found. Add [lib] crate-type = ["cdylib"].'
        )

    if len(candidates) == 1:
        return candidates[0]

    for d, lib in candidates:
        if d == cwd.resolve():
            return d, lib

    msg = "Multiple cdylib targets in workspace:\n" + "\n".join(
        f"- {d} (lib: {lib})" for d, lib in candidates
    )
    msg += "\nRun this script from the desired package directory to disambiguate."
    raise RuntimeError(msg)


def infer_pkg_and_lib(cwd: Path) -> Tuple[Path, str]:
    manifest_path = cwd / "Cargo.toml"
    if not manifest_path.is_file():
        raise RuntimeError(f"Cargo.toml not found in {cwd}")
    manifest = load_toml(manifest_path)
    if not is_workspace_root(manifest):
        lib = infer_lib_name_from_manifest(manifest)
        if not lib:
            raise RuntimeError(
                "Could not infer lib name. Ensure [lib].name or [package].name is set."
            )
        return cwd, lib
    meta = cargo_metadata(cwd)
    return pick_workspace_cdylib(meta, cwd)


def artifact_path_for(
    target_dir: Path,
    target: str,
    lib_name: str,
    os_: str,
    profile: str,
    windows_gnu: bool = False,
) -> Path:
    base = target_dir / target / profile
    if os_ == "windows" or windows_gnu:
        return base / f"{lib_name}.dll"
    return base / (
        f"{lib_name}.so" if lib_name.startswith("lib") else f"lib{lib_name}.so"
    )


def gmod_suffix(os_: str, arch: int, windows_gnu: bool = False) -> str:
    if os_ == "windows" or windows_gnu:
        return "_win32.dll" if arch == 32 else "_win64.dll"
    else:
        return "_linux.dll" if arch == 32 else "_linux64.dll"


def final_out_name(
    outdir: Path, base_name: str, os_: str, arch: int, windows_gnu: bool = False
) -> Path:
    return outdir / (base_name + gmod_suffix(os_, arch, windows_gnu))


def ensure_rust_toolchain_and_target(toolchain: str, target: str):
    if not have("rustup"):
        raise RuntimeError("rustup not found. Install Rust (rustup) first.")
    run(["rustup", "target", "add", "--toolchain", toolchain, target])


def main():
    parser = argparse.ArgumentParser(description="Build Rust cdylib for Garry's Mod.")
    parser.add_argument(
        "-C",
        "--directory",
        default=".",
        help="Crate dir or workspace root (default: .)",
    )
    parser.add_argument(
        "-a",
        "--arch",
        type=int,
        choices=[32, 64],
        default=32,
        help="Target arch: 32 or 64 (default 32)",
    )
    parser.add_argument(
        "-o", "--outdir", default="bin", help="Output directory (default: bin)"
    )
    parser.add_argument(
        "-n",
        "--name",
        default=None,
        help="Optional override for final base name (defaults to [lib].name)",
    )
    parser.add_argument(
        "--rustflags", default="", help="Extra RUSTFLAGS to append (space separated)"
    )
    parser.add_argument(
        "--link-args",
        default="",
        help="Extra linker args; appended as -C link-arg=<arg> each",
    )
    parser.add_argument(
        "-d", "--dev", action="store_true", help="Build in dev (non-release) mode"
    )
    parser.add_argument(
        "-t",
        "--toolchain",
        default="stable",
        help="Rust toolchain (default: stable, e.g. nightly, 1.81.0, stable-YYYY-MM-DD)",
    )
    parser.add_argument(
        "--windows-gnu", action="store_true", help="Build for Windows GNU toolchain"
    )

    args = parser.parse_args()

    workdir = Path(args.directory).resolve()
    if not workdir.is_dir():
        print(f"error: directory not found: {workdir}")
        sys.exit(1)

    os_ = host_os()
    target = target_triple(os_, args.arch, windows_gnu=args.windows_gnu)

    pkg_dir, lib_name = infer_pkg_and_lib(workdir)
    base_name = args.name or lib_name

    ensure_rust_toolchain_and_target(args.toolchain, target)

    env = os.environ.copy()

    rustflags = []
    if args.link_args.strip():
        for tok in args.link_args.strip().split():
            rustflags.extend(["-C", f"link-arg={tok}"])
    if args.rustflags.strip():
        rustflags.extend(args.rustflags.strip().split())
    env["RUSTFLAGS"] = " ".join(rustflags)

    target_dir = Path(env.get("CARGO_TARGET_DIR") or (pkg_dir / "target")).resolve()
    env["CARGO_TARGET_DIR"] = str(target_dir)
    profile = "debug" if args.dev else "release"

    print("=" * 50)
    print("GMod Rust Build")
    print("-" * 50)
    print(f"Toolchain       : {args.toolchain}")
    print(f"Host OS         : {os_}")
    print(f"Arch            : {args.arch}-bit")
    print(f"Target          : {target}")
    print(f"Crate dir       : {pkg_dir}")
    print(f"Inferred lib    : {lib_name}")
    print(f"Final base      : {base_name}")
    print(f"Out dir         : {args.outdir}")
    print(f"Profile         : {profile}")
    print(f"RUSTFLAGS       : {env['RUSTFLAGS']}")
    print("=" * 50)
    print()

    ensure_dir(Path(args.outdir))
    ensure_dir(target_dir)

    cargo_cmd = ["cargo", f"+{args.toolchain}", "build", "--target", target]
    if not args.dev:
        cargo_cmd.append("--release")

    run(cargo_cmd, env=env, cwd=str(pkg_dir))

    built_abs = artifact_path_for(
        target_dir, target, lib_name, os_, profile, windows_gnu=args.windows_gnu
    )
    if not built_abs.is_file():
        print(f"error: built artifact not found: {built_abs}")
        sys.exit(1)

    dest = final_out_name(
        Path(args.outdir).resolve(),
        base_name,
        os_,
        args.arch,
        windows_gnu=args.windows_gnu,
    )
    shutil.copy2(built_abs, dest)


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as e:
        print(
            f"command failed ({e.returncode}): {' '.join(e.cmd) if isinstance(e.cmd, list) else e.cmd}"
        )
        sys.exit(e.returncode)
    except Exception as e:
        print(f"error: {e}")
        sys.exit(1)
