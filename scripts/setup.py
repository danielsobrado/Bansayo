#!/usr/bin/env python3
"""Sets up a Bansayo development machine: downloads, verifies and installs everything local.

  python scripts/setup.py                 every step below, skipping what is already in place
  python scripts/setup.py check           report what is present, change nothing
  python scripts/setup.py models onnxruntime   only the named steps
  python scripts/setup.py cameras         write config/local-cameras.yaml from the attached cameras

Steps, in order:
  prerequisites  report Rust, Node.js, npm, Python and Godot (installs nothing system-wide)
  models         YOLOX-m (person) and RTMPose-m (body) ONNX models into models/
  onnxruntime    ONNX Runtime 1.24.4 shared library into models/onnxruntime/
                 (DirectML build on Windows, CPU build on Linux x64 and macOS arm64)
  python         opencv-python and numpy for scripts/calibrate_cameras.py (pip --user)
  desktop        npm ci in apps/desktop
  engine         cargo build --release -p bansayo-engine
  godot          headless import of apps/vr, if Godot is on PATH
  verify         engine configuration check of config/cameras-two.yaml (model paths) and the
                 calibration selftest

Every download is checked against a pinned SHA-256 before anything is extracted. Model weights
and the runtime are git-ignored local files: see models/README.md for sources and licences.
Re-run with --force to replace files that are already present.
"""
import argparse
import hashlib
import io
import json
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODELS = ROOT / "models"
ORT_VERSION = "1.24.4"

# (zip url, zip sha256, {member suffix: installed name}, installed onnx sha256)
MODEL_ARCHIVES = [
    (
        "https://download.openmmlab.com/mmpose/v1/projects/rtmposev1/onnx_sdk/"
        "rtmpose-m_simcc-body7_pt-body7_420e-256x192-e48f03d0_20230504.zip",
        "f7fbb6c5c11a1bb70f3d445e4ddec5d144ea89ad5649c081ef976f9b24a0b741",
        {"end2end.onnx": "rtmpose-m.onnx", "deploy.json": "rtmpose-m.deploy.json",
         "pipeline.json": "rtmpose-m.pipeline.json", "detail.json": "rtmpose-m.detail.json"},
        "5c0a4bf67953e6d2ac43ce15e77dc9d5d354ae18430a47d2c5963a7bc5683e3c",
    ),
    (
        "https://download.openmmlab.com/mmpose/v1/projects/rtmposev1/onnx_sdk/"
        "yolox_m_8xb8-300e_humanart-c2c7a14a.zip",
        "a000224fd8ba283202bc62d4a5fcdfe353adb9f468777dbac1ea2ada2093adde",
        {"end2end.onnx": "yolox-m-humanart.onnx", "deploy.json": "yolox-m-humanart.deploy.json",
         "pipeline.json": "yolox-m-humanart.pipeline.json", "detail.json": "yolox-m-humanart.detail.json"},
        "3dea6513388889f0fff4b77bf7a26013600321b9eb9ceb0e9a400a82572f5f23",
    ),
]

# platform key -> (url, sha256, {archive member suffix: installed name})
ORT_BUILDS = {
    "windows-x86_64": (
        "https://files.pythonhosted.org/packages/88/ea/33814eb0ec96775eda4c1d30b0d86e91d7d2cd0d84c66d3915aef0e06fa3/"
        f"onnxruntime_directml-{ORT_VERSION}-cp312-cp312-win_amd64.whl",
        "f2ecb68b7b7b259d2ef3112ae760149f9b5a1e7c0fbb73d539da6250a648a614",
        {"onnxruntime/capi/onnxruntime.dll": "onnxruntime.dll",
         "onnxruntime/capi/DirectML.dll": "DirectML.dll",
         "onnxruntime/capi/onnxruntime_providers_shared.dll": "onnxruntime_providers_shared.dll"},
    ),
    "linux-x86_64": (
        f"https://github.com/microsoft/onnxruntime/releases/download/v{ORT_VERSION}/"
        f"onnxruntime-linux-x64-{ORT_VERSION}.tgz",
        "3a211fbea252c1e66290658f1b735b772056149f28321e71c308942cdb54b747",
        {f"lib/libonnxruntime.so.{ORT_VERSION}": "libonnxruntime.so",
         "lib/libonnxruntime_providers_shared.so": "libonnxruntime_providers_shared.so"},
    ),
    "macos-arm64": (
        f"https://github.com/microsoft/onnxruntime/releases/download/v{ORT_VERSION}/"
        f"onnxruntime-osx-arm64-{ORT_VERSION}.tgz",
        "93787795f47e1eee369182e43ed51b9e5da0878ab0346aecf4258979b8bba989",
        {f"lib/libonnxruntime.{ORT_VERSION}.dylib": "libonnxruntime.dylib"},
    ),
}


def say(message):
    print(f"[setup] {message}", flush=True)


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def download(url, expected_sha256):
    """Downloads into memory and returns the bytes only if the SHA-256 matches."""
    say(f"downloading {url}")
    with urllib.request.urlopen(url, timeout=120) as response:
        data = response.read()
    actual = hashlib.sha256(data).hexdigest()
    if actual != expected_sha256:
        sys.exit(f"[setup] checksum mismatch for {url}\n  expected {expected_sha256}\n  got      {actual}")
    return data


def members(data, url):
    """Archive members as {name: bytes} for zip/whl and tgz archives."""
    if url.endswith((".zip", ".whl")):
        with zipfile.ZipFile(io.BytesIO(data)) as z:
            return {n: z.read(n) for n in z.namelist() if not n.endswith("/")}
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as t:
        return {m.name: t.extractfile(m).read() for m in t.getmembers() if m.isfile()}


def install(files, wanted, target):
    """Writes the archive members whose names end with each wanted suffix; refuses path tricks."""
    target.mkdir(parents=True, exist_ok=True)
    for suffix, name in wanted.items():
        matches = [n for n in files if n.endswith(suffix)]
        if len(matches) != 1:
            sys.exit(f"[setup] expected one archive member ending in {suffix}, found {matches}")
        assert "/" not in name and "\\" not in name
        (target / name).write_bytes(files[matches[0]])
        say(f"installed {(target / name).relative_to(ROOT)}")


def platform_key():
    machine = platform.machine().lower()
    arch = "x86_64" if machine in ("amd64", "x86_64") else "arm64" if machine in ("arm64", "aarch64") else machine
    system = {"win32": "windows", "darwin": "macos"}.get(sys.platform, sys.platform)
    return f"{system}-{arch}"


def which(tool):
    return shutil.which(tool)


def run(cmd, cwd=ROOT, check=True):
    say("$ " + " ".join(str(c) for c in cmd) + (f"   (in {Path(cwd).relative_to(ROOT) or '.'})" if cwd != ROOT else ""))
    # On Windows, npm and npx are .cmd shims that need the shell to resolve.
    return subprocess.run([str(c) for c in cmd], cwd=cwd, check=check, shell=sys.platform == "win32" and cmd[0] in ("npm", "npx"))


# ---------------------------------------------------------------- steps

def step_prerequisites(a):
    ok = True
    for tool, why, required in [
        ("cargo", "engine (Rust 1.92+, rustup reads rust-toolchain.toml)", True),
        ("node", "desktop app and gate checks (Node.js 22+)", True),
        ("npm", "desktop app dependencies", True),
        ("godot", "VR training room (Godot 4.5)", False),
    ]:
        path = which(tool)
        if path:
            version = subprocess.run([path, "--version"], capture_output=True, text=True).stdout.strip().splitlines()
            say(f"{tool}: {version[0] if version else 'found'}")
        else:
            say(f"{tool}: MISSING - needed for the {why}")
            ok = ok and not required
    say(f"python: {platform.python_version()} ({sys.executable})")
    if not ok:
        say("install the missing tools (https://rustup.rs, https://nodejs.org), then re-run")
    return ok


def step_models(a):
    for url, zip_sha, wanted, onnx_sha in MODEL_ARCHIVES:
        onnx = MODELS / wanted["end2end.onnx"]
        if onnx.exists() and not a.force and sha256(onnx) == onnx_sha:
            say(f"{onnx.relative_to(ROOT)} present and verified")
            continue
        install(members(download(url, zip_sha), url), wanted, MODELS)
        if sha256(onnx) != onnx_sha:
            sys.exit(f"[setup] {onnx} does not match its pinned checksum")
    return True


def step_onnxruntime(a):
    key = platform_key()
    if key not in ORT_BUILDS:
        say(f"no pinned ONNX Runtime build for {key}; set ORT_DYLIB_PATH to an onnxruntime {ORT_VERSION} library")
        return False
    url, digest, wanted = ORT_BUILDS[key]
    target = MODELS / "onnxruntime"
    main = target / next(iter(wanted.values()))
    stamp = target / "VERSION"
    if main.exists() and not a.force and stamp.exists() and stamp.read_text().strip() == f"{ORT_VERSION} {digest}":
        say(f"{main.relative_to(ROOT)} present ({ORT_VERSION})")
        return True
    install(members(download(url, digest), url), wanted, target)
    stamp.write_text(f"{ORT_VERSION} {digest}\n")
    return True


def step_python(a):
    missing = [pkg for module, pkg in [("cv2", "opencv-python"), ("numpy", "numpy")]
               if subprocess.run([sys.executable, "-c", f"import {module}"], capture_output=True).returncode]
    if not missing:
        say("opencv-python and numpy present")
        return True
    run([sys.executable, "-m", "pip", "install", "--user", *missing])
    return True


def step_desktop(a):
    desktop = ROOT / "apps" / "desktop"
    if (desktop / "node_modules").exists() and not a.force:
        say("apps/desktop/node_modules present (use --force to reinstall with npm ci)")
        return True
    run(["npm", "ci"], cwd=desktop)
    return True


def step_engine(a):
    run(["cargo", "build", "--release", "-p", "bansayo-engine"])
    return True


def step_godot(a):
    if not which("godot"):
        say("godot not on PATH; skipping the VR project import")
        return True
    run(["godot", "--headless", "--path", ROOT / "apps" / "vr", "--xr-mode", "off", "--editor", "--quit"])
    run(["godot", "--headless", "--path", ROOT / "apps" / "vr", "--xr-mode", "off", "--script", "tests/contract_smoke.gd"])
    run(["godot", "--headless", "--path", ROOT / "apps" / "vr", "--xr-mode", "off", "--script", "tests/spatial_ui_smoke.gd"])
    return True


def step_verify(a):
    run(["cargo", "run", "--release", "-q", "-p", "bansayo-engine", "--", "--config", "config/cameras-two.yaml", "--check"])
    run([sys.executable, ROOT / "scripts" / "calibrate_cameras.py", "selftest"])
    return True


def step_cameras(a):
    """Writes config/local-cameras.yaml with every attached camera (git-ignored)."""
    listed = subprocess.run(["cargo", "run", "--release", "-q", "-p", "bansayo-engine", "--", "--list-cameras"],
                            cwd=ROOT, capture_output=True, text=True)
    # Camera lines are "<index>\t<name>"; the indented lines under each list its formats.
    rows = [line.split("\t", 1) for line in listed.stdout.splitlines()
            if "\t" in line and not line.startswith("\t")]
    if listed.returncode or not rows:
        say((listed.stderr or listed.stdout).strip() or "no cameras found")
        return False
    # Manual exposure for kicks (08 §3.2); a camera that cannot take it reports which setting failed.
    cameras = "\n".join(
        f"      - {{ id: CAM{i + 1}, device: {index}, resolution_px: [1280, 720], rate_hz: 30, exposure_s: 0.004 }}   # {name}"
        for i, (index, name) in enumerate(rows[:8]))
    template_name = "one-webcam.yaml" if len(rows) == 1 else "cameras-two.yaml"
    template = (ROOT / "config" / template_name).read_text(encoding="utf-8")
    head = template[:template.index("    cameras:")]
    out = ROOT / "config" / "local-cameras.yaml"
    out.write_text(head + "    cameras:\n" + cameras + "\n", encoding="utf-8")
    say(f"wrote {out.relative_to(ROOT)} with {len(rows[:8])} camera(s); check resolution_px, then run:")
    say(f"  cargo run --release -p bansayo-engine -- --config {out.relative_to(ROOT)}")
    return True


STEPS = {
    "prerequisites": step_prerequisites,
    "models": step_models,
    "onnxruntime": step_onnxruntime,
    "python": step_python,
    "desktop": step_desktop,
    "engine": step_engine,
    "godot": step_godot,
    "verify": step_verify,
}


def check(a):
    """Reports status without changing anything."""
    step_prerequisites(a)
    for url, _, wanted, onnx_sha in MODEL_ARCHIVES:
        onnx = MODELS / wanted["end2end.onnx"]
        state = "verified" if onnx.exists() and sha256(onnx) == onnx_sha else "present, checksum differs" if onnx.exists() else "MISSING"
        say(f"{onnx.relative_to(ROOT)}: {state}")
    stamp = MODELS / "onnxruntime" / "VERSION"
    say(f"models/onnxruntime: {stamp.read_text().split()[0] if stamp.exists() else 'MISSING (or installed by hand)'}")
    for module in ("cv2", "numpy"):
        found = not subprocess.run([sys.executable, "-c", f"import {module}"], capture_output=True).returncode
        say(f"python {module}: {'present' if found else 'MISSING'}")
    say(f"apps/desktop/node_modules: {'present' if (ROOT / 'apps/desktop/node_modules').exists() else 'MISSING'}")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("steps", nargs="*", help="steps to run (default: all); or `check`, `cameras`")
    p.add_argument("--force", action="store_true", help="replace files that are already present")
    a = p.parse_args()
    if a.steps == ["check"]:
        return check(a)
    if a.steps == ["cameras"]:
        return sys.exit(0 if step_cameras(a) else 1)
    unknown = [s for s in a.steps if s not in STEPS]
    if unknown:
        sys.exit(f"unknown step(s) {unknown}; choose from {', '.join(STEPS)}, check, cameras")
    failed = [name for name in (a.steps or STEPS) if not STEPS[name](a)]
    if failed:
        sys.exit(f"[setup] finished with problems in: {', '.join(failed)}")
    say("done. Next: README.md, 'Attach physical cameras'")


if __name__ == "__main__":
    main()
