#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# near-sdk calls the on-chain host functions (panic_utf8 and others) only under
# cfg(near), which cargo-near sets. Without it every failed call is a bare trap
# with no error name. CARGO_ENCODED_RUSTFLAGS would override RUSTFLAGS. With
# --target given, RUSTFLAGS does not reach build scripts or proc-macros.
unset CARGO_ENCODED_RUSTFLAGS
export RUSTFLAGS="--cfg near"
cargo build -p pool-contract -p pool-executor --target wasm32-unknown-unknown --release --locked
mkdir -p artifacts
python3 - <<'PY'
import hashlib,json,os,pathlib,subprocess,sys

def leb(b,i):
    """Unsigned LEB128 at b[i]; returns (value, next index)."""
    r=s=0
    while True:
        x=b[i]; i+=1; r|=(x&0x7f)<<s; s+=7
        if x<0x80: return r,i

def limits(b,i):
    flags=b[i]; _,i=leb(b,i+1)
    if flags&1: _,i=leb(b,i)
    return i

def imports(p):
    """Sorted 'module.name' of every import in the wasm import section (id 2)."""
    b=p.read_bytes()
    if b[:8]!=b'\0asm\x01\0\0\0': sys.exit(f'{p}: not a wasm 1 module')
    i=8
    while i<len(b):
        sid=b[i]; size,i=leb(b,i+1); end=i+size
        if sid==2:
            n,i=leb(b,i); out=[]
            for _ in range(n):
                names=[]
                for _ in range(2):
                    k,i=leb(b,i); names.append(b[i:i+k].decode()); i+=k
                kind=b[i]; i+=1
                if kind==0: _,i=leb(b,i)              # function: type index
                elif kind==1: i=limits(b,i+1)         # table: reftype, limits
                elif kind==2: i=limits(b,i)           # memory: limits
                elif kind==3: i+=2                    # global: valtype, mutability
                elif kind==4: _,i=leb(b,i+1)          # tag: attribute, type index
                else: sys.exit(f'{p}: unknown import kind {kind}')
                out.append('.'.join(names))
            if i!=end: sys.exit(f'{p}: malformed import section')
            return sorted(out)
        i=end
    return []

# NT-F3: both contracts make only function calls, through promise_create and
# promise_then. near-sdk's Promise type links the host functions for every
# other action. Fail if any of them, or any promise_batch_* function, is imported.
FORBIDDEN=['deploy','delete_account','add_key','delete_key','gas_key','transfer','stake','create_account','global_contract']
def forbidden(imp):
    name=imp.split('.',1)[-1]
    return name.startswith('promise_batch_') or any(w in name for w in FORBIDDEN)

paths=[pathlib.Path('target/wasm32-unknown-unknown/release')/(n+'.wasm') for n in ['pool_contract','pool_executor']]
wasm={}
for p in paths:
    found=imports(p)
    # NT-F1: without cfg(near) env::panic_str traps instead of calling panic_utf8.
    if 'env.panic_utf8' not in found: sys.exit(f'{p}: does not import env.panic_utf8; was it built without --cfg near?')
    bad=[i for i in found if forbidden(i)]
    if bad: sys.exit(f'{p}: imports host functions for actions other than function calls: {", ".join(bad)}')
    wasm[str(p)]={'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size,'imports':found}
manifest={'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'dirty':bool(subprocess.check_output(['git','status','--porcelain'],text=True).strip()),'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'rustflags':os.environ['RUSTFLAGS'],'cargo_lock_sha256':hashlib.sha256(pathlib.Path('Cargo.lock').read_bytes()).hexdigest(),'wasm':wasm}
pathlib.Path('artifacts/build.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(manifest,indent=2))
PY
