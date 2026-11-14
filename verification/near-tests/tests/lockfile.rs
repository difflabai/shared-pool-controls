//! Lockfile rule (near-approval strategy.md section 9), adapted to a harness
//! with one test-only dependency (wasmi).
//!
//! 1. The dependency closure of the code under test (`pool-contract`,
//!    `pool-executor`, `pool-core`, and `near-sdk` with `unit-testing`) in this
//!    crate's `Cargo.lock` is a subset of `../source-mirror/Cargo.lock`, with
//!    equal versions and checksums: the tests run against the TCB the deployed
//!    wasm is built from.
//! 2. Every other package is reachable only from `wasmi` (the interpreter
//!    that runs the pinned wasm in `tests/common`), and is listed.
//!
//! `cargo test --locked --test lockfile`.
//!
//! No catalogue ID names the lockfile rule. The test is a precondition of
//! every property this layer checks, not a check of what they say: it makes
//! the host path run the same near-sdk, borsh, serde_json and pool crates
//! the wasm is built from. The tags below record that scope.
//!
// PROP: RD-PREVIEW SCN-1 SH-1 SH-2 SH-3 SH-4 SH-5 SH-6 SH-7 SH-8 SH-9 SH-10 EX-1 EX-2 EX-3 EX-4 EX-5 EX-6 EX-7 EX-8

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Clone, Debug)]
struct Pkg {
    name: String,
    version: String,
    checksum: String,
    deps: Vec<String>,
}

fn packages(path: &Path) -> Vec<Pkg> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.split("[[package]]")
        .skip(1)
        .map(|block| {
            let field = |k: &str| {
                block
                    .lines()
                    .find_map(|l| l.strip_prefix(&format!("{k} = \"")))
                    .map(|v| v.trim_end_matches('"').to_string())
            };
            let mut deps = Vec::new();
            if let Some(i) = block.find("dependencies = [") {
                for l in block[i..].lines().skip(1) {
                    let l = l.trim();
                    if l.starts_with(']') {
                        break;
                    }
                    deps.push(l.trim_matches(|c| c == '"' || c == ',').to_string());
                }
            }
            Pkg {
                name: field("name").expect("name"),
                version: field("version").expect("version"),
                checksum: field("checksum").unwrap_or_else(|| "<path>".into()),
                deps,
            }
        })
        .collect()
}

/// Resolves a `dependencies` entry ("name" or "name version [(source)]").
fn resolve<'a>(pkgs: &'a [Pkg], dep: &str) -> &'a Pkg {
    let mut it = dep.split(' ');
    let name = it.next().unwrap();
    let version = it.next();
    let found: Vec<&Pkg> = pkgs
        .iter()
        .filter(|p| p.name == name && version.is_none_or(|v| p.version == v))
        .collect();
    assert_eq!(found.len(), 1, "ambiguous or missing dependency {dep}");
    found[0]
}

fn closure(pkgs: &[Pkg], roots: &[&str]) -> BTreeSet<(String, String)> {
    let mut seen = BTreeSet::new();
    let mut todo: Vec<&Pkg> = roots.iter().map(|r| resolve(pkgs, r)).collect();
    while let Some(p) = todo.pop() {
        if seen.insert((p.name.clone(), p.version.clone())) {
            todo.extend(p.deps.iter().map(|d| resolve(pkgs, d)));
        }
    }
    seen
}

// PROP: RD-PREVIEW SCN-1 SH-1 SH-2 SH-3 SH-4 SH-5 SH-6 SH-7 SH-8 SH-9 SH-10 EX-1 EX-2 EX-3 EX-4 EX-5 EX-6 EX-7 EX-8
#[test]
fn code_under_test_resolves_exactly_as_in_the_mirror_lockfile() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ours = packages(&here.join("Cargo.lock"));
    let mirror = packages(&here.join("../source-mirror/Cargo.lock"));
    let mirror_set: BTreeMap<(String, String), String> = mirror
        .iter()
        .map(|p| ((p.name.clone(), p.version.clone()), p.checksum.clone()))
        .collect();
    let tcb = closure(&ours, &["pool-contract", "pool-executor", "pool-core", "near-sdk"]);
    let mut bad = Vec::new();
    for (name, version) in &tcb {
        let ck = &ours.iter().find(|p| &p.name == name && &p.version == version).unwrap().checksum;
        match mirror_set.get(&(name.clone(), version.clone())) {
            Some(m) if m == ck => {}
            other => bad.push(format!("{name} {version}: ours {ck}, mirror {other:?}")),
        }
    }
    assert!(bad.is_empty(), "TCB drift from the mirror lockfile:\n{}", bad.join("\n"));
    for must in [
        "near-sdk",
        "near-sdk-macros",
        "near-sdk-core",
        "near-sdk-env",
        "near-sys",
        "near-account-id",
        "near-vm-runner",
        "borsh",
        "serde_json",
        "pool-core",
        "pool-contract",
        "pool-executor",
    ] {
        assert!(tcb.iter().any(|(n, _)| n == must), "{must} not in the TCB closure");
    }
    // Everything else comes from the wasm interpreter only.
    let harness = closure(&ours, &["wasmi"]);
    let rest: Vec<String> = ours
        .iter()
        .filter(|p| p.name != "shared-pool-near-tests")
        .filter(|p| !tcb.contains(&(p.name.clone(), p.version.clone())))
        .map(|p| format!("{} {}", p.name, p.version))
        .collect();
    for r in &rest {
        let mut it = r.split(' ');
        let k = (it.next().unwrap().to_string(), it.next().unwrap().to_string());
        assert!(harness.contains(&k), "{r} is neither in the TCB closure nor wasmi's");
    }
    eprintln!(
        "lockfile: {} TCB packages equal to the mirror; harness-only (wasmi): {}",
        tcb.len(),
        rest.join(", ")
    );
}
