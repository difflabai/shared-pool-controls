#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p pool-contract -p pool-executor --target wasm32-unknown-unknown --release --locked
mkdir -p artifacts
python3 - <<'PY'
import hashlib,json,pathlib,subprocess
paths=[pathlib.Path('target/wasm32-unknown-unknown/release')/(n+'.wasm') for n in ['pool_contract','pool_executor']]
manifest={'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'dirty':bool(subprocess.check_output(['git','status','--porcelain'],text=True).strip()),'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'cargo_lock_sha256':hashlib.sha256(pathlib.Path('Cargo.lock').read_bytes()).hexdigest(),'wasm':{str(p):{'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size} for p in paths}}
pathlib.Path('artifacts/build.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(manifest,indent=2))
PY
