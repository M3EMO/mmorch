"""portability — export/import mmorch state across devices (grafts G2 + G4).

Ported from paperclip's company-portability: a versioned manifest where every
value is tagged portable | system_dependent | secret (G2). Cross-device sync
(your 2 PCs + phone via Lotus/Tailscale) works because system_dependent values
(absolute paths) and secrets (tokens) are NOT trusted across machines — they are
re-provided on import (G4). Import reconciles by name (skip existing, override
paths, never silently apply a stale absolute path).

ponytail: pure export/reconcile (unit-tested with a temp store) + a thin
import that applies the plan via projects.register.
"""
from __future__ import annotations

import os

SCHEMA = 1


def tag(value, kind: str) -> dict:
    """kind: 'portable' (travels) | 'system_dependent' (machine-local) | 'secret' (re-provide)."""
    return {"value": (None if kind == "secret" else value), "portability": kind}


def export_bundle(projects: dict, hosts: dict, exec_policy: str, ts: float) -> dict:
    return {
        "schemaVersion": SCHEMA,
        "generatedAt": ts,
        "exec_policy": exec_policy,
        "projects": [{"name": n, "path": tag(p, "system_dependent")} for n, p in projects.items()],
        "fleet": [{"name": n, "url": tag((h or {}).get("url", ""), "portable"),
                   "token": tag(None, "secret")} for n, h in hosts.items()],
    }


def reconcile(manifest: dict, existing: dict, overrides: dict) -> dict:
    """Plan an import without applying it. overrides: {project_name: local_path}."""
    plan: dict = {"register": [], "skipped": [], "needs_path": [], "warnings": []}
    if manifest.get("schemaVersion") != SCHEMA:
        plan["warnings"].append(f"schemaVersion {manifest.get('schemaVersion')} != {SCHEMA}")
    for proj in manifest.get("projects", []):
        name = proj.get("name")
        if not name:
            continue
        if name in existing:
            plan["skipped"].append(name)            # collision: keep local
            continue
        src = (proj.get("path") or {}).get("value")
        path = overrides.get(name) or (src if src and os.path.isdir(src) else None)
        if path and os.path.isdir(path):
            plan["register"].append({"name": name, "path": path})
        else:
            plan["needs_path"].append(name)         # system_dependent: must be re-provided here
    return plan


def import_bundle(manifest: dict, overrides=None, store=None) -> dict:
    from .projects import list_projects, register
    existing = list_projects(store=store)
    plan = reconcile(manifest, existing, overrides or {})
    applied = []
    for item in plan["register"]:
        try:
            register(item["name"], item["path"], store=store)
            applied.append(item["name"])
        except Exception as e:
            plan["warnings"].append(f"{item['name']}: {str(e)[:120]}")
    plan["applied"] = applied
    return plan


if __name__ == "__main__":
    import tempfile
    real = tempfile.mkdtemp()                       # a dir that exists on THIS machine
    m = export_bundle({"A": real, "B": "/no/such/path"}, {}, "any", 123.0)
    assert m["schemaVersion"] == SCHEMA
    assert m["projects"][0]["path"]["portability"] == "system_dependent"
    # cross-machine: A's path happens to exist -> register; B's doesn't -> needs_path
    p = reconcile(m, existing={}, overrides={})
    assert any(r["name"] == "A" for r in p["register"]), p
    assert "B" in p["needs_path"], p
    # override supplies B's local path
    p2 = reconcile(m, existing={}, overrides={"B": real})
    assert sorted(r["name"] for r in p2["register"]) == ["A", "B"], p2
    # collision: A already registered locally -> skipped
    p3 = reconcile(m, existing={"A": real}, overrides={})
    assert p3["skipped"] == ["A"] and "B" in p3["needs_path"], p3
    print("portability OK")
