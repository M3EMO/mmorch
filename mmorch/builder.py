"""builder — arma un entregable desde una carpeta vacia, modulo por modulo, por el SDLC (builder-e2e ticket 06).

    python -m mmorch.builder <plan.json> <carpeta_vacia>

El plan es la salida de la descomposicion (Opus, aprobada en el walkthrough). Rutas relativas a la carpeta del plan:
    {"bootstrap": {"destino/en/el/repo": "origen"},                  # contratos, sdlc.toml, Dockerfile.sdlc, config
     "modulos": [{"nombre": "m1", "tarea": "...", "files": ["src/a.ts"], "contrato": "c/a.test.ts",
                  "accept": "tests/a.test.ts"}],                     # en orden: cada uno parte del anterior
     "integracion": [{"imagen": "img", "cmd": "... {repo} {base} ...", "montajes": ["dir"]}]}   # exit 0 = verde
Bootstrap: commit inicial y, si hay Dockerfile.sdlc y `contenedor` en sdlc.toml, construye la imagen (con red).
Encadenado: cada modulo construido se fusiona en main antes del siguiente. Corta en el primer modulo que no queda
construido; el resto no corre. Devuelve una fila por modulo y otra por comando de integracion.
"""
from __future__ import annotations

import json
import pathlib
import subprocess
import sys
import time

from .sandbox import container_argv, engine


def _git(repo: pathlib.Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)


def bootstrap(repo: pathlib.Path, archivos: dict[str, pathlib.Path]) -> str:
    """Repo nuevo con los archivos del bootstrap en su commit inicial. Nunca pisa una carpeta con contenido."""
    if repo.exists() and any(repo.iterdir()):
        raise SystemExit(f"{repo} no esta vacia: el builder solo arranca desde una carpeta vacia")
    repo.mkdir(parents=True, exist_ok=True)
    _git(repo, "init", "-q", "-b", "main")
    for destino, origen in archivos.items():
        (repo / destino).parent.mkdir(parents=True, exist_ok=True)
        (repo / destino).write_bytes(origen.read_bytes())
    _git(repo, "add", "-A")
    c = _git(repo, "commit", "-qm", "bootstrap del builder: contratos y configuracion")
    if c.returncode:
        raise SystemExit(f"commit inicial fallo: {c.stdout}{c.stderr}")
    import tomllib
    toml = repo / "sdlc.toml"
    imagen = tomllib.loads(toml.read_text(encoding="utf-8")).get("contenedor") if toml.exists() else None
    if imagen and (repo / "Dockerfile.sdlc").exists():
        b = subprocess.run([engine() or "docker", "build", "-q", "-f", "Dockerfile.sdlc", "-t", imagen, "."], cwd=repo,
                           capture_output=True, text=True)
        if b.returncode:
            raise SystemExit(f"la imagen {imagen} no se construyo: {b.stderr[-800:]}")
    return _git(repo, "rev-parse", "--short", "HEAD").stdout.strip()


def construir(repo: pathlib.Path, modulos: list[dict], wts: pathlib.Path) -> list[dict]:
    """Cada modulo por el SDLC; el construido se fusiona en main y el siguiente parte de ahi (hueco 3 del ticket 02)."""
    from .sdlc import StageFailed, build_feature
    filas = []
    for m in modulos:
        t0, fase = time.time(), f"builder-{m['nombre']}"
        try:
            st = build_feature(m["nombre"], m["tarea"], str(repo), accept={m["accept"]: m["contrato_texto"]},
                               files=m["files"], wt=wts / m["nombre"], phase=fase)
        except StageFailed as e:
            st = {"status": "stage_failed", "failed_stage": e.stage, "failed_note": e.note}
        fila = {"modulo": m["nombre"], "status": st.get("status"), "failed_stage": st.get("failed_stage"),
                "minutos": round((time.time() - t0) / 60, 1), "claude_calls": st.get("claude_calls"),
                "rechazos": [g["gate"] for g in st.get("gate_rejects", [])]}
        filas.append(fila)
        if st.get("status") != "built":
            fila["nota"] = (st.get("failed_note") or "")[:400]
            break
        mg = _git(repo, "merge", "--no-ff", "-m", f"builder: integra {m['nombre']}", f"sdlc/{fase}")
        _git(repo, "worktree", "remove", "--force", str(wts / m["nombre"]))
        if mg.returncode:
            fila.update(status="merge_failed", nota=(mg.stdout + mg.stderr)[-400:])
            _git(repo, "merge", "--abort")
            break
    return filas


def integrar(repo: pathlib.Path, pasos: list[dict], base: pathlib.Path) -> list[dict]:
    filas = []
    for i, p in enumerate(pasos):
        montajes = [str(repo), str(base)] + [str((base / d).resolve()) for d in p.get("montajes", [])]
        cmd = p["cmd"].replace("{repo}", str(repo)).replace("{base}", str(base))   # rutas que el plan no conoce
        argv = container_argv(cmd, image=p["imagen"], cwd=str(repo), mounts=montajes,
                              name=f"builder-int-{i}-{time.time_ns()}", eng=engine() or "docker")
        r = subprocess.run(argv, capture_output=True, text=True, timeout=p.get("timeout", 900))
        filas.append({"integracion": p["cmd"][:80], "ok": r.returncode == 0, "salida": (r.stdout + r.stderr)[-600:]})
        if r.returncode:
            break
    return filas


def armar(plan_path: str, repo_dir: str) -> list[dict]:
    plan_file = pathlib.Path(plan_path).resolve()
    base, repo = plan_file.parent, pathlib.Path(repo_dir).resolve()
    plan = json.loads(plan_file.read_text(encoding="utf-8"))
    bootstrap(repo, {d: base / o for d, o in plan["bootstrap"].items()})
    modulos = [m | {"contrato_texto": (base / m["contrato"]).read_text(encoding="utf-8")} for m in plan["modulos"]]
    filas = construir(repo, modulos, repo.parent / f".{repo.name}-worktrees")
    if all(f["status"] == "built" for f in filas) and len(filas) == len(modulos):
        filas += integrar(repo, plan.get("integracion", []), base)
    return filas


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    if len(args) != 2:
        print(__doc__)
        return 2
    filas = armar(*args)
    for f in filas:
        print(json.dumps(f, ensure_ascii=False))
    verde = bool(filas) and all(f.get("status", "built") == "built" and f.get("ok", True) for f in filas)
    return 0 if verde else 1


if __name__ == "__main__":
    raise SystemExit(main())
