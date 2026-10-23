"""builder — arma un entregable desde una carpeta vacia, modulo por modulo, por el SDLC (builder-e2e ticket 06).

    python -m mmorch.builder <plan.json> <carpeta_vacia>

El plan es la salida de la descomposicion (Opus, aprobada en el walkthrough). Rutas relativas a la carpeta del plan:
    {"bootstrap": {"destino/en/el/repo": "origen"},                  # contratos, sdlc.toml, Dockerfile.sdlc, config
     "modulos": [{"nombre": "m1", "tarea": "...", "files": ["src/a.ts"], "contrato": "c/a.test.ts",
                  "accept": "tests/a.test.ts"}],                     # en orden: cada uno parte del anterior
     "integracion": [{"imagen": "img", "cmd": "... {repo} {base} ...", "montajes": ["dir"]}],   # exit 0 = verde
     "coders": ["deepseek-v4-pro", "claude:claude-haiku-5-5"], "semilla": 20261015,   # opcionales: sorteo por modulo
     "juez": {"contra": "capturas/otro-entregable", "rubricas": ["seo_texto", "visual"]}}   # opcional: sombra
Bootstrap: commit inicial y, si hay Dockerfile.sdlc y `contenedor` en sdlc.toml, construye la imagen (con red).
`{capturas}` en un comando de integracion = carpeta donde el driver deja evidencia para el juez en sombra.
Paradas del usuario (ticket 01: walkthrough y preview), medidas de abrir a cerrar:
    python -m mmorch.builder parada <entregable> <walkthrough|preview> <abrir|aprobado|rechazado> [nota]
Encadenado: cada modulo construido se fusiona en main antes del siguiente. Corta en el primer modulo que no queda
construido; el resto no corre. Devuelve una fila por modulo y otra por comando de integracion.
"""
from __future__ import annotations

import json
import pathlib
import random
import subprocess
import sys
import time

from mmorch.paths import logs_dir
from mmorch.sandbox import container_argv, engine


CODERS = ["deepseek-v4-pro", "claude:claude-haiku-5-5"]   # ticket 01: Haiku 5.5 o DeepSeek v4 pro por sorteo
SEMILLA = 20261015


def sortear(coders: list[str], n: int, semilla: int) -> list[str]:
    """Coder por modulo, balanceado (mitad y mitad, +-1) y mezclado con semilla: elegir a mano sesgaria el banco."""
    asignados = (coders * (n // len(coders) + 1))[:n]
    random.Random(semilla).shuffle(asignados)
    return asignados


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
    from .pipeline import StageFailed, build_feature
    filas = []
    for m in modulos:
        t0, fase = time.time(), f"builder-{m['nombre']}"
        try:
            st = build_feature(m["nombre"], m["tarea"], str(repo), accept={m["accept"]: m["contrato_texto"]},
                               files=m["files"], wt=wts / m["nombre"], phase=fase, coder=m.get("coder"))
        except StageFailed as e:
            st = {"status": "stage_failed", "failed_stage": e.stage, "failed_note": e.note}
        fila = {"modulo": m["nombre"], "coder": m.get("coder"), "coder_cupo_usd": st.get("claude_coder_usd"), "usd": st.get("usd_run"),
                "status": st.get("status"), "failed_stage": st.get("failed_stage"),
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


def _capturas(base: pathlib.Path, repo: pathlib.Path) -> pathlib.Path:
    """Donde el driver deja texto y capturas para el juez en sombra: fuera del repo, junto al plan."""
    return base / "capturas" / repo.name


def juzgar(repo: pathlib.Path, juez: dict, base: pathlib.Path) -> list[dict]:
    """Ticket 09: opinion de a pares contra un entregable de referencia. Va a sombra y NUNCA cambia el veredicto."""
    from mmorch.juez_sombra import comparar
    nuevo, ref = _capturas(base, repo), (base / juez["contra"]).resolve()
    if not (nuevo / "texto.txt").exists() or not (ref / "texto.txt").exists():
        return [{"juez": "sin capturas", "nuevo": str(nuevo), "contra": str(ref)}]
    filas = []
    for r in juez.get("rubricas", ["seo_texto", "visual"]):
        d = comparar(nuevo, ref, r)
        filas.append({"juez": r, "contra": juez["contra"], "mejor": d["mejor"], "motivo": d["motivo_ab"] or d["error"]})
    return filas


def integrar(repo: pathlib.Path, pasos: list[dict], base: pathlib.Path) -> list[dict]:
    filas = []
    for i, p in enumerate(pasos):
        montajes = [str(repo), str(base)] + [str((base / d).resolve()) for d in p.get("montajes", [])]
        cmd = p["cmd"].replace("{repo}", str(repo)).replace("{base}", str(base)).replace("{capturas}", str(_capturas(base, repo)))
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
    for m, coder in zip(modulos, sortear(plan.get("coders", CODERS), len(modulos), plan.get("semilla", SEMILLA)), strict=True):
        m.setdefault("coder", coder)
    filas = construir(repo, modulos, repo.parent / f".{repo.name}-worktrees")
    if all(f["status"] == "built" for f in filas) and len(filas) == len(modulos):
        filas += integrar(repo, plan.get("integracion", []), base)
        if plan.get("juez"):
            filas += juzgar(repo, plan["juez"], base)
    return filas


def _paradas() -> pathlib.Path:
    return logs_dir() / "builder" / "paradas.jsonl"


def parada(entregable: str, nombre: str, evento: str, nota: str = "", *, ts: float | None = None) -> dict:
    """Anota una parada y devuelve el total del entregable: minutos del usuario y rechazos (destino: < 1 h, 2 paradas).
    shortcut: reloj de pared de abrir a cerrar, cota superior (el usuario puede no estar mirando); upgrade si hace
    falta tiempo activo."""
    if evento not in ("abrir", "aprobado", "rechazado"):
        raise ValueError(f"evento desconocido: {evento}")
    _paradas().parent.mkdir(parents=True, exist_ok=True)
    with _paradas().open("a", encoding="utf-8") as f:
        f.write(json.dumps({"ts": time.time() if ts is None else ts, "entregable": entregable, "parada": nombre, "evento": evento,
                            "nota": nota}, ensure_ascii=False) + "\n")
    abiertas: dict[str, float] = {}
    minutos, rechazos, aprobadas = 0.0, 0, []
    for d in (json.loads(x) for x in _paradas().read_text(encoding="utf-8").splitlines() if x.strip()):
        if d["entregable"] != entregable:
            continue
        if d["evento"] == "abrir":
            abiertas[d["parada"]] = d["ts"]
        elif d["parada"] in abiertas:   # un cierre sin apertura no suma minutos que no se midieron
            minutos += (d["ts"] - abiertas.pop(d["parada"])) / 60
            rechazos += d["evento"] == "rechazado"
            if d["evento"] == "aprobado":
                aprobadas.append(d["parada"])
    return {"entregable": entregable, "minutos": round(minutos, 1), "rechazos": rechazos, "aprobadas": aprobadas}


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    if args[:1] == ["parada"] and len(args) in (4, 5):
        print(json.dumps(parada(*args[1:]), ensure_ascii=False))
        return 0
    if len(args) != 2:
        print(__doc__)
        return 2
    filas = armar(*args)
    for f in filas:
        print(json.dumps(f, ensure_ascii=False))
    verde = bool(filas) and all(f.get("status", "built") == "built" and f.get("ok", True) for f in filas)   # el juez no cuenta
    return 0 if verde else 1


if __name__ == "__main__":
    raise SystemExit(main())
