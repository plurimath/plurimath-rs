#!/usr/bin/env python3
"""Package the tested facade web glue and record exact source/tool/file digests."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--bindgen', type=Path, required=True)
parser.add_argument('--wasm', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]

def run(*command):
    return subprocess.check_output(command, cwd=root, text=True).strip()

def digest(path):
    data = path.read_bytes()
    return {'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}

if run('git', 'status', '--porcelain', '--untracked-files=no'):
    raise SystemExit('Refusing to package a dirty source tree')
version = (root / 'Cargo.lock').read_text().split('name = "wasm-bindgen"\nversion = "')[1].split('"')[0]
if run(str(args.bindgen), '--version') != f'wasm-bindgen {version}':
    raise SystemExit('wasm-bindgen executable differs from Cargo.lock')
args.output.mkdir(parents=True, exist_ok=True)
subprocess.run([str(args.bindgen), str(args.wasm), '--target', 'web',
                '--out-name', 'facade', '--out-dir', str(args.output)], check=True)
(args.output / 'package.json').write_text('{"private":true,"type":"module"}\n')
# Keep licenses/source and the lockfile beside the generated runtime.
for source in ['Cargo.lock', 'plurimath-facade/LICENSE', 'plurimath-facade/NOTICE']:
    shutil.copyfile(root / source, args.output / Path(source).name)
files = {path.name: digest(path) for path in sorted(args.output.iterdir()) if path.is_file() and path.name != 'provenance.json'}
provenance = {
    'schema': 'plurimath-facade-wasm-runtime/1',
    'repository': 'https://github.com/plurimath/plurimath-rs',
    'revision': run('git', 'rev-parse', 'HEAD'),
    'cargoLock': digest(root / 'Cargo.lock'),
    'rustc': run('rustc', '+stable', '-vV'),
    'wasmBindgen': {'version': version, **digest(args.bindgen)},
    'rawWasm': digest(args.wasm),
    'target': 'wasm32-unknown-unknown',
    'profile': 'release',
    'stackBytes': 8388608,
    'files': files,
}
(args.output / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
print(json.dumps({'revision': provenance['revision'], 'files': len(files), 'output': str(args.output)}))
