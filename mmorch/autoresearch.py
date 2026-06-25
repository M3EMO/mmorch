"""autoresearch (r4a) — hillclimb como JOB declarativo + resumable.

Optimiza una METRICA ESCALAR editando un archivo con un modelo, contra un scorer
DETERMINISTA frozen. El loop es mmorch.hillclimb (NO se reimplementa). Extraido de
karpathy/autoresearch: search-space (archivo editable) vs frozen oracle (scorer_cmd),
keep/discard (best gana), journal append-only (qrf), resume desde el journal.

anti-reward-hacking (invariante mmorch): el score sale del scorer CORRIBLE, jamas de un
LLM-judge. El modelo solo PROPONE; la verdad la da la ejecucion.

Declarativo a proposito: propose/score se arman adentro a partir de config JSON-able, asi
un tool MCP (mmorch_autoresearch) puede lanzarlo (los callables no cruzan MCP). gen_fn/
run_fn son inyectables para test (cero API).
"""
from __future__ import annotations

import json
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path

from .hillclimb import hillclimb, ClimbCtx, ClimbResult
from .config import DEFAULT_GENERATOR

_FENCE = re.compile(r"```(?:[a-zA-Z]*)?\s*(.*?)```", re.DOTALL)


def _extract(text: str) -> str:
    m = _FENCE.search(text or "")
    return (m.group(1) if m else (text or "")).strip()


def parse_metric(text: str, regex: str) -> float:
    """Extrae el numero del output del scorer. Lanza si no aparece (=> ronda fallida)."""
    m = re.search(regex, text or "")
    if not m:
        raise ValueError("scorer no emitio la metrica esperada")
    return float(m.group(1))


def resume_from_journal(journal_path: Path) -> tuple[int, float | None]:
    """Lee un journal de hillclimb (qrf) y devuelve (rondas_hechas, mejor_score visto).
    Permite continuar una corrida overnight cortada. (0, None) si no hay journal."""
    p = Path(journal_path)
    if not p.exists():
        return 0, None
    rounds, best = 0, None
    for ln in p.read_text(encoding="utf-8").splitlines():
        if not ln.strip():
            continue
        rec = json.loads(ln)
        rounds += 1
        bs = rec.get("best_score")
        if bs is not None:
            best = bs if best is None else best
    # best_score del ULTIMO registro es el mejor acumulado al cortar
    last = [json.loads(l) for l in p.read_text(encoding="utf-8").splitlines() if l.strip()]
    if last and last[-1].get("best_score") is not None:
        best = last[-1]["best_score"]
    return rounds, best


@dataclass
class AutoResearchResult:
    best_score: float | None
    baseline: float | None
    rounds: int
    stopped: str
    best_content: str


def run_autoresearch(
    task: str,
    target_file: str,
    scorer_cmd: str,
    *,
    cwd: str = ".",
    models: list[str] | None = None,
    maximize: bool = False,
    max_rounds: int = 20,
    patience: int = 5,
    min_delta: float = 0.0,
    metric_regex: str = r"score[:=]\s*([-\d.]+)",
    scorer_timeout: float = 120.0,
    journal_path: str | None = None,
    resume: bool = False,
    gen_fn=None,
    run_fn=None,
) -> AutoResearchResult:
    """Corre hillclimb sobre `target_file` optimizando la metrica que emite `scorer_cmd`.

    propose: lee el MEJOR contenido actual, pide al modelo mejorarlo dado `task` + el ultimo
      feedback del journal, escribe el candidato al archivo, lo devuelve.
    score: corre `scorer_cmd` en `cwd`, parsea la metrica (frozen oracle). Excepcion/no-metrica
      => ronda fallida (el loop sigue).
    keep/discard: hillclimb se queda con el best; al final el best vuelve a escribirse al archivo.
    journal + resume: ledger por ronda; con resume=True arranca del best del journal previo.
    models: arms del bandit (rota generador por ronda); default = DEFAULT_GENERATOR.
    """
    fpath = Path(cwd) / target_file
    models = models or [DEFAULT_GENERATOR]

    def _call_model(model, prompt):
        if gen_fn is not None:
            return gen_fn(model, prompt)              # inyectable: (model, prompt) -> text
        from .providers import call
        return call(model, [{"role": "user", "content": prompt}],
                    pattern="autoresearch", node=model).text

    def _default_run(cmd):
        p = subprocess.run(cmd, cwd=cwd, shell=True, capture_output=True, text=True,
                           encoding="utf-8", errors="replace", timeout=scorer_timeout)
        return (p.stdout + p.stderr)

    run = run_fn or _default_run                      # inyectable: (cmd) -> output text

    base_content = fpath.read_text(encoding="utf-8") if fpath.exists() else ""

    def propose(ctx: ClimbCtx) -> str:
        cur = ctx.best if ctx.best is not None else base_content
        fpath.write_text(cur, encoding="utf-8")
        last = ctx.history[-1].detail if ctx.history else ""
        prompt = (f"TAREA (optimizar una metrica, {'mayor' if maximize else 'menor'} es mejor):\n{task}\n\n"
                  f"ARCHIVO `{target_file}` actual:\n```\n{cur[:6000]}\n```\n"
                  + (f"\nFeedback de la ronda previa: {last[:500]}\n" if last else "")
                  + "Devolve SOLO el contenido COMPLETO nuevo del archivo en un bloque ```.")
        model = ctx.arm or models[0]
        new = _extract(_call_model(model, prompt))
        if not new:
            return cur
        fpath.write_text(new, encoding="utf-8")
        return new

    def score(content: str) -> float:
        out = run(scorer_cmd)
        return parse_metric(out, metric_regex)

    jp = Path(journal_path) if journal_path else None
    initial = base_content
    rounds_done = 0
    if resume and jp:
        rounds_done, _ = resume_from_journal(jp)

    res: ClimbResult = hillclimb(
        propose, score, initial=initial, maximize=maximize,
        max_rounds=max(1, max_rounds - rounds_done), patience=patience,
        min_delta=min_delta, arms=models if len(models) > 1 else None,
        arm=models[0] if len(models) == 1 else None,
        journal_path=jp, pattern="autoresearch", context=task[:2000],
    )
    if res.best is not None:
        fpath.write_text(res.best, encoding="utf-8")   # best gana (keep)
    return AutoResearchResult(res.best_score, res.baseline, res.rounds + rounds_done,
                              res.stopped, res.best if res.best is not None else base_content)
