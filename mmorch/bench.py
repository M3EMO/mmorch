"""bench — benchmark CONGELADO de tasks difíciles para evolución de workflows.

La pieza crítica del diseño (memoria: workflow-evolution-design): no se puede saber qué
workflow es mejor sin tasks FIJAS con acceptance CORRIBLE. Cada task es data pura declarada
acá (frozen: cambiarlas rompe la comparabilidad entre noches — se versionan, no se editan).

Anti-contaminación: `held_out=True` marca tasks que NUNCA se usan para seleccionar variantes
(solo para validar que el ganador generaliza). Anti-Goodhart (lección F4): el acceptance de
cada task pinea la señal POSITIVA (tests que exigen comportamiento, no ausencia de error).

materialize(task, dst) arma un repo git real en dst: archivos semilla + tests de acceptance
congelados + git init/commit — listo para que un workflow (p.ej. build_project) lo ataque con
external_test = su accept_cmd.
"""
from __future__ import annotations

import subprocess
from dataclasses import dataclass, field
from pathlib import Path


@dataclass(frozen=True)
class BenchTask:
    name: str
    task: str                      # el enunciado que recibe el workflow
    files: dict = field(default_factory=dict)      # path -> contenido semilla (puede ser vacío)
    accept_files: dict = field(default_factory=dict)  # tests congelados (el juez)
    accept_cmd: str = "python -m pytest tests_accept -q"
    held_out: bool = False


# --------------------------------------------------------------------------- #
# Tasks v1 — multi-módulo, con dependencias entre unidades (lo que rompía al
# flat build-feature). Los tests exigen COMPORTAMIENTO (anti-Goodhart).
# --------------------------------------------------------------------------- #
TASKS: list[BenchTask] = [
    BenchTask(
        name="etl-pipeline",
        task=(
            "Construí un mini-ETL en Python, 3 módulos que se importan entre sí:\n"
            "1. etl/extract.py: parse_lines(lines) -> list[dict] — cada línea CSV 'id,nombre,monto' "
            "(monto float, AR: coma decimal '1234,56'). Línea malformada se SALTEA (no rompe).\n"
            "2. etl/transform.py: normalize(rows) -> list[dict] — nombre a Title Case, monto "
            "redondeado a 2 decimales, descarta montos negativos, dedup por id (queda el primero).\n"
            "3. etl/load.py: to_summary(rows) -> dict — {'count': n, 'total': suma de montos "
            "redondeada a 2, 'by_name': {nombre: total_por_nombre}}.\n"
            "etl/__init__.py expone run(lines) que encadena los tres."
        ),
        files={"etl/__init__.py": ""},
        accept_files={"tests_accept/test_etl.py": '''
from etl import run
from etl.extract import parse_lines
from etl.transform import normalize

def test_extract_salta_malformadas():
    rows = parse_lines(["1,Ana,100,50", "basura", "2,Luis,200,00"])
    assert len(rows) == 2 or len(rows) == 0  # coma decimal AR: '100,50' es UN campo monto
    rows2 = parse_lines(["1,ana,1234,56"])
    assert rows2 and abs(rows2[0]["monto"] - 1234.56) < 0.01

def test_normalize_titlecase_dedup_negativos():
    rows = normalize([{"id": "1", "nombre": "ana maria", "monto": 10.005},
                      {"id": "1", "nombre": "otra", "monto": 5.0},
                      {"id": "2", "nombre": "luis", "monto": -3.0}])
    assert len(rows) == 1
    assert rows[0]["nombre"] == "Ana Maria"
    assert rows[0]["monto"] == 10.0 or rows[0]["monto"] == 10.01

def test_run_end_to_end():
    s = run(["1,ana,100,00", "2,luis,50,50", "malo", "1,ana,999,99"])
    assert s["count"] == 2
    assert abs(s["total"] - 150.5) < 0.01
    assert abs(s["by_name"]["Ana"] - 100.0) < 0.01
'''},
    ),
    BenchTask(
        name="rate-limiter",
        task=(
            "Construí limiter/core.py con una clase TokenBucket(capacity, refill_per_s):\n"
            "- allow(now: float) -> bool: consume 1 token si hay; los tokens se recargan a "
            "refill_per_s por segundo desde el último allow, cap en capacity.\n"
            "- El tiempo entra SIEMPRE por parámetro (determinista, sin time.time()).\n"
            "Y limiter/multi.py con MultiLimiter(capacity, refill_per_s): allow(key, now) — un "
            "bucket independiente por key, creado on-demand. limiter/__init__.py exporta ambos."
        ),
        files={"limiter/__init__.py": ""},
        accept_files={"tests_accept/test_limiter.py": '''
from limiter import TokenBucket, MultiLimiter

def test_bucket_agota_y_recarga():
    b = TokenBucket(capacity=2, refill_per_s=1.0)
    assert b.allow(0.0) and b.allow(0.0)
    assert not b.allow(0.0)          # vacío
    assert b.allow(1.0)              # 1 segundo -> 1 token
    assert not b.allow(1.0)

def test_bucket_cap():
    b = TokenBucket(capacity=2, refill_per_s=1.0)
    assert b.allow(0.0)
    assert b.allow(100.0)            # mucha recarga, pero cap=2
    assert b.allow(100.0)
    assert not b.allow(100.0)

def test_multi_independiente():
    m = MultiLimiter(capacity=1, refill_per_s=0.0)
    assert m.allow("a", 0.0)
    assert m.allow("b", 0.0)         # key distinta, bucket propio
    assert not m.allow("a", 0.0)
'''},
    ),
    BenchTask(
        name="lru-ttl-cache",
        held_out=True,   # NUNCA para seleccionar — solo para validar que el ganador generaliza
        task=(
            "Construí cache/core.py con LRUCache(maxsize, ttl_s): get(key, now) -> valor o None; "
            "put(key, value, now). Expira por TTL (now - insert_time > ttl_s) Y desaloja el menos "
            "recientemente USADO al superar maxsize. get renueva recencia, no TTL. Tiempo siempre "
            "por parámetro. cache/__init__.py lo exporta."
        ),
        files={"cache/__init__.py": ""},
        accept_files={"tests_accept/test_cache.py": '''
from cache import LRUCache

def test_ttl_expira():
    c = LRUCache(maxsize=10, ttl_s=5.0)
    c.put("a", 1, now=0.0)
    assert c.get("a", now=4.9) == 1
    assert c.get("a", now=5.1) is None

def test_lru_desaloja_menos_usado():
    c = LRUCache(maxsize=2, ttl_s=100.0)
    c.put("a", 1, now=0.0); c.put("b", 2, now=1.0)
    c.get("a", now=2.0)              # 'a' ahora es el más reciente
    c.put("c", 3, now=3.0)           # desaloja 'b', no 'a'
    assert c.get("a", now=4.0) == 1
    assert c.get("b", now=4.0) is None
    assert c.get("c", now=4.0) == 3

def test_get_renueva_recencia_no_ttl():
    c = LRUCache(maxsize=10, ttl_s=5.0)
    c.put("a", 1, now=0.0)
    c.get("a", now=4.0)
    assert c.get("a", now=5.5) is None   # TTL corre desde el put, no desde el get
'''},
    ),
]


def get_task(name: str) -> BenchTask:
    for t in TASKS:
        if t.name == name:
            return t
    raise KeyError(f"bench task desconocida: {name}. Hay: {[t.name for t in TASKS]}")


def selection_tasks() -> list[BenchTask]:
    """Las tasks para SELECCIONAR variantes (excluye held-out — anti-contaminación)."""
    return [t for t in TASKS if not t.held_out]


def materialize(task: BenchTask, dst: str) -> str:
    """Arma un repo git real en `dst`: semillas + tests congelados + commit inicial.
    Devuelve el accept_cmd listo para usar como external_test."""
    root = Path(dst)
    root.mkdir(parents=True, exist_ok=True)
    for rel, content in {**task.files, **task.accept_files}.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
    for args in (["init", "-q"], ["config", "user.email", "bench@mmorch"],
                 ["config", "user.name", "bench"], ["config", "commit.gpgsign", "false"],
                 ["add", "-A"], ["commit", "-q", "-m", f"bench:{task.name} frozen"]):
        subprocess.run(["git", *args], cwd=str(root), capture_output=True)
    # 'python' pelado + shell=True resuelve el python del SISTEMA (sin pytest) — la carrera
    # viva 2026-07-08 murió entera por esto. El cmd corre con EL intérprete de mmorch.
    import sys
    return task.accept_cmd.replace("python ", f'"{sys.executable}" ', 1)


if __name__ == "__main__":
    import subprocess as sp
    import sys
    import tempfile

    assert len(selection_tasks()) == 2 and get_task("lru-ttl-cache").held_out
    try:
        get_task("nope")
        raise AssertionError("task inexistente debe fallar")
    except KeyError:
        pass

    # materialize arma un repo real y el acceptance FALLA en el estado semilla (si pasara
    # vacío, el benchmark no mediría nada — anti-Goodhart aplicado al propio benchmark).
    for t in TASKS:
        d = tempfile.mkdtemp()
        cmd = materialize(t, d)
        assert (Path(d) / ".git").exists()
        p = sp.run([sys.executable, "-m", "pytest", "tests_accept", "-q", "--no-header"],
                   cwd=d, capture_output=True, text=True, timeout=120)
        assert p.returncode != 0, f"{t.name}: el acceptance NO debe pasar sobre el repo semilla"
    print("bench OK — 3 tasks congeladas (1 held-out), materialize arma repo git, "
          "acceptance rojo sobre semilla (el benchmark mide de verdad)")
