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
