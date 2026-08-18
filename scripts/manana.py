"""Cockpit matutino de mmorch — TODO lo que depende del humano, en un comando.

Uso diario:  .venv/Scripts/python.exe scripts/manana.py

Secuencia: digest de anoche -> salud -> merges pendientes (tren + amarillas,
con semáforo, diffstat e interacción m/enter/q) -> veredictos pendientes
(d/n/enter/q) -> resumen. Cinco minutos y el sistema queda servido.
"""

import io
import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

G, R, Y, D, B, C, X = ("\x1b[92m", "\x1b[91m", "\x1b[93m", "\x1b[2m",
                       "\x1b[1m", "\x1b[96m", "\x1b[0m")
ZC = {"green": G + "🟢", "yellow": Y + "🟡", "red": R + "🔴", "paused": D + "⏸"}


def _git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True,
                          text=True, encoding="utf-8", errors="replace")


def _hdr(title):
    print(f"\n{B}{C}══ {title} {'═' * max(4, 60 - len(title))}{X}")


def seccion_digest():
    _hdr("DIGEST DE ANOCHE")
    p = ROOT / "logs" / "digest_last.md"
    print(p.read_text(encoding="utf-8") if p.exists()
          else f"{D}(sin digest — ¿corrió el nightly?){X}")


def seccion_salud():
    _hdr("SALUD")
    from mmorch.health import report
    r = report(logs_dir=str(ROOT / "logs"))
    if r["healthy"]:
        print(f"  {G}✓ todo late{X}")
    for d in r["check"]["dead"]:
        print(f"  {R}✗ {d['component']}: vencido hace {d['overdue_s']/3600:.1f}h{X}")
    for n in r["check"]["never"]:
        print(f"  {D}· {n}: sin latidos aún{X}")
    for k, v in r["errors"]["nightly_errors"].items():
        print(f"  {Y}⚠ {k}: {str(v)[:90]}{X}")
    try:
        smoke = json.loads((ROOT / "logs" / "smoke.jsonl")
                           .read_text(encoding="utf-8").strip().splitlines()[-1])
        col = G if not smoke["fails"] else R
        print(f"  {col}🧪 smoke {smoke['ok']}/{smoke['total']}"
              f"{' — ' + ','.join(smoke['fails']) if smoke['fails'] else ''}{X}")
    except (OSError, IndexError, json.JSONDecodeError):
        pass


def seccion_merges():
    _hdr("MERGES PENDIENTES")
    from mmorch.automerge import classify_branch
    base = _git("rev-parse", "--abbrev-ref", "HEAD").stdout.strip()
    out = _git("branch", "--list", "--no-merged", base, "mmorch/*",
               "mmorch-sbx-*", "--format=%(refname:short)")
    branches = [b.strip() for b in out.stdout.splitlines() if b.strip()]
    if not branches:
        print(f"  {D}(nada pendiente de merge){X}")
        return 0
    # tren primero (un click resuelve N)
    branches.sort(key=lambda b: (0 if "tren" in b else 1, b))
    merged = 0
    for b in branches:
        z = classify_branch(str(ROOT), b, base=base)
        zone = z.get("zone", "?")
        stat = _git("diff", "--shortstat", f"{base}..{b}").stdout.strip()
        print(f"\n  {ZC.get(zone, zone)}{X} {B}{b}{X}")
        print(f"    {D}{stat or '(sin diff)'}"
              f"{' · ' + z.get('reason', '') if zone == 'red' else ''}{X}")
        if zone == "red":
            print(f"    {R}rojo: solo merge manual tuyo fuera de este tool{X}")
            continue
        r = input(f"    {B}[m]erge / [d]iff / enter=saltar / q=salir >{X} ").strip().lower()
        if r == "q":
            break
        if r == "d":
            print(_git("diff", "--stat", f"{base}..{b}").stdout[:2000])
            r = input(f"    {B}[m]erge / enter=saltar >{X} ").strip().lower()
        if r == "m":
            m = _git("merge", "--no-edit", b)
            if m.returncode == 0:
                print(f"    {G}✓ mergeada{X}")
                merged += 1
            else:
                _git("merge", "--abort")
                print(f"    {R}✗ conflicto — apartada: {m.stderr[:100]}{X}")
    return merged


def seccion_veredictos():
    _hdr("VEREDICTOS PENDIENTES")
    from mmorch.curation import pending
    p = pending()
    n = len(p["candidatas"]) + len(p["cards"])
    if not n:
        print(f"  {D}(nada pendiente de veredicto){X}")
        return
    print(f"  {n} pendientes — entrando al modo interactivo (d/n/enter/q)")
    import importlib
    veredicto = importlib.import_module("veredicto")
    veredicto.interactivo()


def main() -> None:
    os.system("")
    print(f"{B}☀ BUENOS DÍAS — cockpit mmorch{X}")
    seccion_digest()
    seccion_salud()
    merged = seccion_merges()
    seccion_veredictos()
    _hdr("LISTO")
    print(f"  merges hechos: {merged} · el sistema queda servido hasta mañana.\n")
    if merged:
        print(f"  {D}tip: git push cuando quieras subir lo mergeado{X}\n")


if __name__ == "__main__":
    main()
