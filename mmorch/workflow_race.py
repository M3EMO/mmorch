"""workflow_race — corre N VARIANTES de workflow sobre una task congelada del bench y
selecciona por ejecución: pasa (gate duro) > costo > tiempo. El ganador por FIRMA de task
alimenta un bandit propio (mismo ThompsonBandit descontado del sig-bandit) — así /project
puede consultar qué forma de workflow funciona mejor para cada forma de task, y el sistema
EVOLUCIONA su estrategia, no solo su código.

Variantes v1 = configuraciones del project-build engine (el probado E2E en F4). El formato es
data (dict) a propósito: una variante futura puede venir de un mutador (COPRO-lite sobre
specs) sin tocar este runner. Scoring 100% determinista (anti-Goodhart: pass = acceptance
congelado del bench; costo = delta real de metrics; tiempo = reloj) — ningún LLM juzga.

Cota de costo: las variantes corren SECUENCIAL (el delta de metrics por variante requiere no
solapar) y el caller decide cuántas tasks por noche (default 1).
"""
from __future__ import annotations

import time
from pathlib import Path

from .feedback import ThompsonBandit

_ROOT = Path(__file__).resolve().parents[1]
_WF_BANDIT = _ROOT / "logs" / "workflow_bandit.json"

# Variantes v1 del project-build engine. Frozen-ish: renombrar una variante resetea su
# historial en el bandit (el arm es el nombre) — versionar como los bench tasks.
VARIANTS: dict[str, dict] = {
    "pb-quick": {"max_fix": 1, "max_depth": 1},   # ¿alcanza lo barato?
    "pb-base":  {"max_fix": 3, "max_depth": 2},   # el default probado en F4
    "pb-deep":  {"max_fix": 5, "max_depth": 2, "max_gen_calls": 250},
}


def _default_build_fn(task_text: str, repo: str, accept_cmd: str, cfg: dict) -> dict:
    from .project_integrate import build_project
    return build_project(task_text, repo, external_test=accept_cmd, **cfg)


def _default_cost_fn() -> float:
    from .metrics import summary
    return float(summary().get("total_cost_usd", 0.0))


def race(bench_task, variants: dict[str, dict] | None = None, *,
         build_fn=None, cost_fn=None, clock=None, materialize_fn=None,
         bandit: ThompsonBandit | None = None, record: bool = True) -> dict:
    """Corre cada variante sobre SU copia limpia de la task (materialize por variante — sin
    contaminación entre corridas). Score por variante: {passed, cost_usd, wall_s, status}.
    Ganador: pasa > menor costo > menor tiempo. Si `record`, alimenta el workflow-bandit
    (arm = 'variante#firma-de-task', reward = 1.0 pasa / 0.0 no) — Thompson descontado, así
    un cambio de régimen no deja estrategia fósil. Todos los boundaries inyectables."""
    import tempfile

    from .signature import signature
    variants = variants or VARIANTS
    build_fn = build_fn or _default_build_fn
    cost_fn = cost_fn or _default_cost_fn
    clock = clock or time.monotonic
    if materialize_fn is None:
        from .bench import materialize as materialize_fn   # type: ignore[assignment]

    sig = signature(bench_task.task).to_key()
    rows: dict[str, dict] = {}
    for name, cfg in variants.items():
        repo = tempfile.mkdtemp(prefix=f"wfrace-{name}-")
        accept_cmd = materialize_fn(bench_task, repo)
        c0, t0 = cost_fn(), clock()
        try:
            res = build_fn(bench_task.task, repo, accept_cmd, cfg)
            status = res.get("status", "?")
        except Exception as e:
            status = f"crash:{type(e).__name__}"
        rows[name] = {"passed": status == "built", "status": status,
                      "cost_usd": round(cost_fn() - c0, 6),
                      "wall_s": round(clock() - t0, 2)}

    passing = [n for n, r in rows.items() if r["passed"]]
    winner = (min(passing, key=lambda n: (rows[n]["cost_usd"], rows[n]["wall_s"]))
              if passing else None)

    if record:
        b = bandit or ThompsonBandit(path=_WF_BANDIT)
        for name, r in rows.items():
            b.update(f"{name}#{sig}", 1.0 if r["passed"] else 0.0)

    return {"task": bench_task.name, "sig": sig, "rows": rows, "winner": winner,
            "held_out": bench_task.held_out}


def best_variant_for(task_text: str, *, min_n: int = 3,
                     bandit: ThompsonBandit | None = None) -> str | None:
    """Consulta del lado /project: mejor variante conocida para la FIRMA de esta task.
    None si no hay evidencia suficiente (min_n — la misma disciplina que intuition.decide:
    sin datos no se opina)."""
    from .signature import signature
    b = bandit or ThompsonBandit(path=_WF_BANDIT)
    sig = signature(task_text).to_key()
    cands = [(a, s) for a, s in b.stats().items() if a.endswith("#" + sig) and s["n"] >= min_n]
    if not cands:
        return None
    return max(cands, key=lambda x: x[1]["mean"])[0].split("#")[0]


def race_live(task_text: str, repo: str, accept_cmd: str, *,
              variants: dict[str, dict] | None = None, build_fn=None,
              max_workers: int | None = None) -> dict:
    """Racing EN VIVO para una task urgente (patron ADW hotfix, 2026-07): N variantes
    compiten EN PARALELO sobre copias del repo; la PRIMERA que pasa gana y las demas se
    descartan (sus threads terminan solos; no hay kill -- costo acotado por la variante
    mas cara, aceptado para el caso urgente). Distinto de race(): aquel corre SECUENCIAL
    sobre bench tasks para evolucion de estrategia (aislar el delta de costo por variante);
    este optimiza LATENCIA sobre una task real -- el costo por variante se difumina (se
    reporta wall_s por variante pero no cost_usd, honesto en vez de un numero mentiroso).
    El resultado ganador queda en el dir devuelto (`repo_dir`) -- el caller decide merge
    (invariante: auto-run si, auto-merge jamas). NO alimenta el workflow-bandit (la task
    viva no es un bench congelado; contaminar el bandit con tasks no comparables = ruido).
    Requiere variantes DIVERSAS, no clones: contra fallas correlacionadas (medido 2026-07:
    rate-limiter 0/3 por el MISMO planner) la redundancia no ayuda, la diversidad si."""
    import concurrent.futures as _cf
    import shutil
    import tempfile

    variants = variants or VARIANTS
    build_fn = build_fn or _default_build_fn

    def _one(name_cfg):
        name, cfg = name_cfg
        wdir = tempfile.mkdtemp(prefix=f"wflive-{name}-")
        # copia del repo (task viva = repo real, no bench materializable)
        dst = wdir + "/repo"
        shutil.copytree(repo, dst,
                        ignore=shutil.ignore_patterns(".git", ".venv", "venv", "node_modules",
                                                      "__pycache__"))
        t0 = time.monotonic()
        try:
            res = build_fn(task_text, dst, accept_cmd, cfg)
            status = res.get("status", "?")
        except Exception as e:
            status = f"crash:{type(e).__name__}"
        return {"variant": name, "passed": status == "built", "status": status,
                "wall_s": round(time.monotonic() - t0, 2), "repo_dir": dst}

    rows: list[dict] = []
    winner = None
    ex = _cf.ThreadPoolExecutor(max_workers=max_workers or len(variants))
    try:
        futs = [ex.submit(_one, nc) for nc in variants.items()]
        for f in _cf.as_completed(futs):
            r = f.result()
            rows.append(r)
            if r["passed"] and winner is None:
                winner = r
                break               # primera verde gana; NO esperar a las lentas
    finally:
        # wait=False: retornar YA con el ganador. Los threads en vuelo terminan solos en
        # background (un `with` joinearia todo y mataria la ganancia de latencia).
        ex.shutdown(wait=False, cancel_futures=True)
    return {"winner": winner, "rows": rows,
            "note": None if winner else "ninguna variante paso -- escalar a humano/Opus"}




if __name__ == "__main__":
    # cero-API/cero-git: build/cost/clock/materialize inyectados; bandit en tmp.
    import tempfile as _tf

    from .bench import get_task
    t = get_task("etl-pipeline")

    ticks = iter(range(100))
    costs = iter([0.0, 0.01, 0.01, 0.05, 0.05, 0.06])   # quick gasta 0.01, base 0.04, deep 0.01

    def _build(task_text, repo, accept, cfg):
        # quick falla (max_fix=1 no alcanza), base y deep pasan
        return {"status": "escalate" if cfg["max_fix"] == 1 else "built"}

    b = ThompsonBandit(path=Path(_tf.mkdtemp()) / "wb.json")
    r = race(t, build_fn=_build, cost_fn=lambda: next(costs), clock=lambda: next(ticks),
             materialize_fn=lambda task, dst: "pytest -q", bandit=b)
    assert r["rows"]["pb-quick"]["passed"] is False
    assert r["rows"]["pb-base"]["passed"] and r["rows"]["pb-deep"]["passed"]
    assert r["winner"] == "pb-deep", r   # ambos pasan; deep costó 0.01 < base 0.04
    # el bandit aprendió por firma: quick 0.0, base/deep 1.0
    st = b.stats()
    assert any(a.startswith("pb-quick#") and s["mean"] < 0.5 for a, s in st.items()), st
    assert any(a.startswith("pb-deep#") and s["mean"] > 0.5 for a, s in st.items()), st

    # best_variant_for: con n=1 no opina (min_n=3); con evidencia, elige el mejor
    assert best_variant_for(t.task, bandit=b) is None
    for _ in range(3):
        b.update("pb-base#" + r["sig"], 1.0)
    assert best_variant_for(t.task, bandit=b) == "pb-base"

    # crash de una variante no rompe la carrera de las demás
    def _build_crashy(task_text, repo, accept, cfg):
        if cfg["max_fix"] == 1:
            raise RuntimeError("boom")
        return {"status": "built"}
    r2 = race(t, build_fn=_build_crashy, cost_fn=lambda: 0.0, clock=lambda: 0.0,
              materialize_fn=lambda task, dst: "x", bandit=b, record=False)
    assert r2["rows"]["pb-quick"]["status"].startswith("crash:") and r2["winner"] in ("pb-base", "pb-deep")

    print("workflow_race OK — carrera por variante, ganador pasa>costo>tiempo, bandit por "
          "firma, consulta con min_n, crash aislado")

    # race_live: primera verde gana SIN esperar a la lenta (fake builds, cero API)
    import time as _t

    def _fake_build(task, repo, accept, cfg):
        _t.sleep(cfg["delay"])
        return {"status": "built" if cfg["ok"] else "escalate"}
    _lv = {"fail-fast": {"delay": 0.01, "ok": False},
           "pass-mid":  {"delay": 0.15, "ok": True},
           "pass-slow": {"delay": 8.0,  "ok": True}}
    _d = _tf.mkdtemp()
    (Path(_d) / "x.txt").write_text("seed")
    _t0 = _t.monotonic()
    lv = race_live("task viva", _d, "true", variants=_lv, build_fn=_fake_build)
    _el = _t.monotonic() - _t0
    assert lv["winner"] and lv["winner"]["variant"] == "pass-mid", lv
    assert _el < 4.0, f"no debe esperar a pass-slow (8s): tardo {_el:.1f}s"
    print("workflow_race OK + race_live (first-green-wins, no espera a las lentas)")
