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
            /TR "<venv>/python.exe <repo>/scripts/nightly.py"
"""
import json
import os
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

    # autoresearch CONFIGURABLE por env (audit 2026-07: el target fijo evolve.py-maintainability
    # no tenía headroom -> baseline==best toda noche. Ahora vos apuntás donde HAYA headroom sin
    # tocar código). Default = fortalecer tests/mut_signature.py contra mutantes (mutation_score).
    #   MMORCH_AR_TASK    enunciado para el coder
    #   MMORCH_AR_TARGET  archivo que edita el loop (relativo al repo)
    #   MMORCH_AR_SCORER  script scorer (recibe el target como argv, imprime 'score: N')
    #   MMORCH_AR_ROUNDS  rounds (default 12) / MMORCH_AR_PATIENCE (default 5)
    #   MMORCH_AR_TARGET_SCORE  corta apenas el best la alcanza (default 1.0), no gasta todos los rounds
    # default = optimizar el system-prompt del coder contra pass-rate de una batería edge-heavy
    # (el ÚNICO target con headroom real medido: baseline ~0.5-0.9 y prompt-sensible; code_quality
    # y mutation_score sobre módulos daban 1.0 fijo — audit 2026-07).
    ar_target = os.getenv("MMORCH_AR_TARGET", "prompts/coder_prompt.txt")
    ar_scorer = os.getenv("MMORCH_AR_SCORER", "scripts/score_coder_prompt.py")
    ar_task = os.getenv("MMORCH_AR_TASK",
                        f"Reescribí el system-prompt de {ar_target} para que un coder Python resuelva "
                        "MÁS de la batería de tasks algorítmicas edge-heavy (casos borde: intervalos "
                        "que tocan, división entera hacia cero, puntuación, matrices no-cuadradas). "
                        "El prompt debe seguir pidiendo SOLO la función en un bloque de código. El "
                        "scorer mide pass-rate por ejecución — no expliques, mejorá el prompt.")
    try:
        from mmorch.autoresearch import run_autoresearch
        from mmorch.worktree_driver import open_worktree
        wt = open_worktree(str(ROOT), prefix="mmorch/ar")
        wt.seed([".venv"])   # el pre-commit hook (ruff gate) usa .venv/Scripts/python.exe relativo
                              # -> sin esto cae a python de sistema sin ruff, la gate falla, el commit
                              # se pierde silencioso (bug medido 2026-07, ver worktree_driver.capture)
        improved = False
        try:
            r = run_autoresearch(
                ar_task, ar_target,
                scorer_cmd=f'"{sys.executable}" {ar_scorer} {ar_target}',
                cwd=wt.path, maximize=True, target=float(os.getenv("MMORCH_AR_TARGET_SCORE", "1.0")),
                max_rounds=int(os.getenv("MMORCH_AR_ROUNDS", "12")),
                patience=int(os.getenv("MMORCH_AR_PATIENCE", "5")), scorer_timeout=700)
            improved = (r.baseline is not None and r.best_score is not None
                        and r.best_score > r.baseline)
            if improved:
                wt.capture(f"autoresearch {ar_target}: {r.baseline} -> {r.best_score}")
            rec["autoresearch"] = {"target": ar_target, "baseline": r.baseline, "best": r.best_score,
                                   "rounds": r.rounds, "improved": improved,
                                   "branch": wt.branch if improved else None}
        finally:
            wt.close(keep_branch=improved)   # branch queda SOLO si mejoró (revisión humana)
    except Exception as e:
        rec["autoresearch_error"] = f"{type(e).__name__}: {str(e)[:200]}"

    # DESTILADO en volumen (audit 2026-07-08: 408 episodios vs 9 notas — el nudge destila de a 5
    # cada 10 closes, el backlog crece más rápido; ritmo de entrada >> ritmo de destilación).
    # La noche es el momento barato para ponerse al día: 50/noche, MISMO watermark que el nudge
    # (nudge.json distill_upto) — un solo estado, dos ritmos (goteo diurno + lote nocturno).
    try:
        from mmorch.memory import _connect, _DB_PATH, distill_backlog
        from mmorch.nudge import _load as _nudge_load, _STATE as _NUDGE_STATE
        st = _nudge_load(_NUDGE_STATE)
        upto = int(st.get("distill_upto", 0))
        # GATE barato-primero (patron dream.rs de grok-build, 2026-07): contar ANTES de
        # destilar — si hay < min_new episodios nuevos, saltear la pasada entera (cada
        # destilado son llamadas gen+verify; una noche sin actividad no debe gastarlas).
        min_new = int(os.getenv("MMORCH_DISTILL_MIN_NEW", "10"))
        _c = _connect(_DB_PATH)
        try:
            n_new = _c.execute("SELECT COUNT(*) FROM episodic WHERE id > ? "
                               "AND kind NOT IN ('consolidation')", [upto]).fetchone()[0]
        finally:
            _c.close()
        if n_new < min_new:
            rec["distill"] = {"skipped": f"solo {n_new} episodios nuevos (< {min_new})"}
        else:
            d = distill_backlog(after_id=upto, limit=50)
            st["distill_upto"] = d["last_id"]
            _NUDGE_STATE.parent.mkdir(parents=True, exist_ok=True)
            _NUDGE_STATE.write_text(json.dumps(st, ensure_ascii=False), encoding="utf-8")
            rec["distill"] = d
            # tier 3 (HCA): con material nuevo destilado, refrescar la vista global
            # comprimida (1 llamada) — solo si el destilado persistio algo.
            if d.get("persisted", 0) > 0:
                from mmorch.memory import refresh_digest
                rec["digest"] = refresh_digest("global")
    except Exception as e:
        rec["distill_error"] = f"{type(e).__name__}: {str(e)[:200]}"

    # WORKFLOW RACE (evolución de estrategia, no solo de código): 1 task del bench por noche
    # (rotando por día — cota de costo), 3 variantes del engine compiten, el ganador por firma
    # alimenta el workflow-bandit. held-out NUNCA entra acá (anti-contaminación).
    try:
        from mmorch.bench import selection_tasks
        from mmorch.workflow_race import race
        sel = selection_tasks()
        # CURRICULUM por frecuencia (2026-07, patron "foco en topics de alta frecuencia"):
        # el compute nocturno se concentra en las FORMAS de task que mas aparecen en el uso
        # real (n del sig-bandit por firma), en vez de rotacion plana. Deterministico: dia %
        # suma-de-pesos camina la distribucion (task frecuente = mas noches). Sin datos del
        # bandit (frio / error) degrada a pesos iguales = la rotacion plana de antes.
        try:
            from mmorch.feedback import ThompsonBandit
            from mmorch.intuition import _SIG_BANDIT
            from mmorch.signature import key as sig_key
            stats = ThompsonBandit(_SIG_BANDIT).stats()

            def _freq(bt):
                sk = sig_key(bt.task)
                return int(sum(s["n"] for a, s in stats.items() if a.endswith("#" + sk)))
            weights = [1 + _freq(bt) for bt in sel]
        except Exception:
            weights = [1] * len(sel)
        r = int(time.time() // 86400) % sum(weights)
        t = sel[0]
        for t, w in zip(sel, weights, strict=True):   # noqa: B007 — t queda con la elegida
            r -= w
            if r < 0:
                break
        # EVOLUTION-SIM (backlog #1 repo-mining 2026-07): la race corre sobre la POBLACION
        # viva (no las 3 hardcodeadas) y el resultado la evoluciona — el ganador se reproduce
        # (hijo mutado/cruzado), el perdedor cronico muere (con evidencia del bandit).
        # Fitness emergente de la distribucion de bench tasks, nunca un escalar frozen.
        from mmorch.workflow_evolve import evolve_population, load_population
        rec["workflow_race"] = race(t, variants=load_population())
        rec["workflow_race"]["curriculum_weights"] = dict(zip([b.name for b in sel], weights, strict=True))
        ev = evolve_population(rec["workflow_race"].get("winner"))
        rec["workflow_race"]["evolution"] = {"born": ev["born"], "died": ev["died"],
                                             "population": sorted(ev["population"])}
    except Exception as e:
        rec["workflow_race_error"] = f"{type(e).__name__}: {str(e)[:200]}"

    # LEARN-FROM-REPOS (read-only): si MMORCH_LEARN_REPOS lista repos PÚBLICOS (coma-separados),
    # cosecha findings de cada uno a logs/external_findings.jsonl. NUNCA abre PR ni toca esos
    # repos — solo material de aprendizaje (repos ajenos = read-only; PRs solo en el tuyo).
    repos = [u.strip() for u in os.getenv("MMORCH_LEARN_REPOS", "").split(",") if u.strip()]
    if repos:
        try:
            from mmorch.evolve_findings import learn_from_repos
            rec["learn"] = learn_from_repos(repos)
        except Exception as e:
            rec["learn_error"] = f"{type(e).__name__}: {str(e)[:200]}"

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
