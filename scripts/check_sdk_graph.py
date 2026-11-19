"""Reject incompatible multi-SDK C symbol providers before compilation."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

EXPECTED_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
EXPECTED_VERSION = "1.5.9"
parser = argparse.ArgumentParser()
parser.add_argument("--metadata", type=Path, help="Check an existing complete Cargo metadata file")
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
metadata = args.metadata
if metadata is None:
    target = Path(os.environ.get("CARGO_TARGET_DIR", root / "target"))
    target.mkdir(parents=True, exist_ok=True)
    metadata = target / "dependency-metadata.json"
    with metadata.open("w", encoding="utf-8") as output:
        subprocess.run(
            ["cargo", "metadata", "--locked", "--all-features", "--format-version", "1"],
            cwd=root, stdout=output, check=True, timeout=300,
        )
packages = json.loads(metadata.read_text(encoding="utf-8"))["packages"]
sdks = [package for package in packages if package["name"] == "parsanol"]
if len(sdks) != 1:
    print("Expected one Parsanol package; multiple SDKs export duplicate C ABI symbols.", file=sys.stderr)
    for package in sdks:
        print(package["id"], file=sys.stderr)
    raise SystemExit(1)
source = sdks[0].get("source") or ""
version = sdks[0].get("version") or ""
if source != EXPECTED_SOURCE or version != EXPECTED_VERSION:
    print("Parsanol must be the published crate " + EXPECTED_VERSION + ": " + version + " " + source, file=sys.stderr)
    raise SystemExit(1)
print("Single Parsanol SDK verified: parsanol " + version + " from " + source)
