"""nightly — driver ALWAYS-ON del loop nocturno (Windows Task Scheduler, no Claude).

El scheduled-task de Claude solo corre con la app abierta; este script corre con la PC
prendida, sin Claude — invoca directo la librería (cero cupo total). Dos patas:

  1. nightly_evolve(): cosecha findings (code_review sobre archivos cambiados) -> propone
     -> sandbox+tests -> PR. Lock por archivo, nunca mergea (evolve.py, ya probado).
  2. autoresearch code_quality sobre mmorch/evolve.py, AISLADO en worktree (run_autoresearch
     edita in-place — sobre el repo vivo sería inaceptable desatendido; el worktree deja una
     branch mmorch/ar-quality-* SOLO si mejoró, para revisión humana, igual que los PRs).

Resultado a logs/nightly.jsonl — la capa de notificación (el task de Claude, 09:00) solo LEE
ese log y resume; no ejecuta nada.

Registrar:  schtasks /Create /TN mmorch-nightly /SC DAILY /ST 02:10 /F
            /TR "<venv>\python.exe <repo>\scripts\nightly.py"
"""
import json
import pathlib
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from dotenv import load_dotenv  # noqa: E402

load_dotenv(ROOT / ".env")

LOG = ROOT / "logs" / "nightly.jsonl"


def _log(rec: dict) -> None:
    LOG.parent.mkdir(parents=True, exist_ok=True)
    with open(LOG, "a", encoding="utf-8") as f:
        f.write(json.dumps(rec, ensure_ascii=False, default=str) + "\n")


def main() -> None:
    rec: dict = {"ts": time.time()}

    try:
        from mmorch.evolve import nightly_evolve
        rec["evolve"] = nightly_evolve()
    except Exception as e:
        rec["evolve_error"] = f"{type(e).__name__}: {str(e)[:200]}"

    try:
        from mmorch.autoresearch import run_autoresearch
        from mmorch.worktree_driver import open_worktree
        wt = open_worktree(str(ROOT), prefix="mmorch/ar-quality")
        improved = False
        try:
            r = run_autoresearch(
                "Mejorá la mantenibilidad de mmorch/evolve.py (menos complejidad ciclomática, "
                "menos anidamiento, funciones más cortas) SIN cambiar comportamiento — los tests "
                "de evolve son el gate y el scorer los corre.",
                "mmorch/evolve.py",
                scorer_cmd=f'"{sys.executable}" scripts/score_quality.py mmorch/evolve.py',
                cwd=wt.path, maximize=True, max_rounds=6, patience=3, scorer_timeout=700)
            improved = (r.baseline is not None and r.best_score is not None
                        and r.best_score > r.baseline)
            if improved:
                wt.capture(f"autoresearch quality evolve.py: {r.baseline} -> {r.best_score}")
            rec["autoresearch"] = {"baseline": r.baseline, "best": r.best_score,
                                   "rounds": r.rounds, "improved": improved,
                                   "branch": wt.branch if improved else None}
        finally:
            wt.close(keep_branch=improved)   # branch queda SOLO si mejoró (revisión humana)
    except Exception as e:
        rec["autoresearch_error"] = f"{type(e).__name__}: {str(e)[:200]}"

    # cola de re-check de arbitrajes (blind-spot #2: descartes del árbitro nunca auditados).
    # nightly solo SURFACEA la cola — el re-juicio es del orquestador (Opus), no de un cron.
    try:
        from mmorch.arbitration import pending_recheck, stats as arb_stats
        rec["arbitration"] = {"pending_recheck": len(pending_recheck()),
                              **{k: v for k, v in arb_stats().items()
                                 if k in ("dismissed_without_evidence_rate", "total")}}
    except Exception as e:
        rec["arbitration_error"] = f"{type(e).__name__}: {str(e)[:120]}"

    _log(rec)
    print(json.dumps(rec, ensure_ascii=False, default=str))


if __name__ == "__main__":
    main()
