#!/usr/bin/env python3
"""Lockfile rule (near-approval strategy.md section 9) for the shared-pool sandbox.

Every registry package present in both sandbox/Cargo.lock and
source-mirror/Cargo.lock must have the same version set, source and checksum.
The sandbox lock was seeded from the mirror's; today it is a strict subset of
it plus the two local crates (shared-pool-sandbox, sandbox-relay). No host-TLS
backport exceptions are needed. Exit 1 on any mismatch, 2 on input errors.
"""
import json
import pathlib
import sys
import tomllib


def index(packages):
    out = {}
    for p in packages:
        if "source" not in p:  # path / workspace members
            continue
        out.setdefault(p["name"], {})[p["version"]] = (p["source"], p.get("checksum"))
    return out


def main():
    here = pathlib.Path(__file__).resolve().parent.parent
    mine = index(tomllib.loads((here / "Cargo.lock").read_text())["package"])
    theirs = index(tomllib.loads((here.parent / "source-mirror/Cargo.lock").read_text())["package"])
    bad, extra = [], []
    for name in sorted(mine):
        if name not in theirs:
            extra.append(name)
            continue
        for version, (source, checksum) in mine[name].items():
            if version not in theirs[name]:
                bad.append(f"{name} {version}: mirror has {sorted(theirs[name])}")
            elif theirs[name][version] != (source, checksum):
                bad.append(f"{name} {version}: source or checksum differs")
    shared = len(mine.keys() & theirs.keys())
    report = {"shared_packages": shared, "sandbox_only": extra, "mismatches": bad}
    if len(sys.argv) > 1:
        pathlib.Path(sys.argv[1]).write_text(json.dumps(report, indent=2) + "\n")
    for line in bad:
        print("lockfile mismatch:", line)
    print(f"check-lock: {shared} shared registry packages, {len(extra)} sandbox-only, {len(bad)} mismatches")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
